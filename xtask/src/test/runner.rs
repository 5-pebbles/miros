use std::{
    fmt,
    fs::{self, File},
    io::{self, Read, Write},
    ops::Range,
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
    process::ExitStatus,
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
    time::Instant,
};

use super::{
    diagnostic::Diagnostic,
    directive::{Directive, ExitExpectation, Stream, TestCase},
    pty,
};
use crate::test::{DIRECTIVE_TIMEOUT, parser::Parser, utils};

#[derive(Debug)]
pub enum Failure {
    Pty(io::Error),
    Spawn(io::Error),
    Fixture(io::Error),
    Wait(io::Error),
    WaitTimeout { text: String, stream: Stream },
    WaitEof { text: String, stream: Stream },
    WriteFailed(io::Error),
    SignalFailed { signal: i32, error: io::Error },
    ExitMismatch { expected: String, actual: String },
    ExitTimeout,
    ExpectMiss { text: String, stream: Stream },
    UnclaimedOutput { stream: Stream, text: String },
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pty(error) => write!(formatter, "pty allocation failed: {error}"),
            Self::Spawn(error) => write!(formatter, "spawn failed: {error}"),
            Self::Fixture(error) => write!(formatter, "fixture staging failed: {error}"),
            Self::Wait(error) => write!(formatter, "wait failed: {error}"),
            Self::WaitTimeout { text, stream } => write!(
                formatter,
                "timed out waiting for {text:?} on {}",
                stream.label()
            ),
            Self::WaitEof { text, stream } => write!(
                formatter,
                "{} closed before {text:?} appeared",
                stream.label()
            ),
            Self::WriteFailed(error) => write!(formatter, "writing to stdin failed: {error}"),
            Self::SignalFailed { signal, error } => {
                write!(formatter, "kill({signal}) failed: {error}")
            }
            Self::ExitMismatch { expected, actual } => {
                write!(formatter, "expected {expected}, got {actual}")
            }
            Self::ExitTimeout => write!(formatter, "timed out waiting for process exit"),
            Self::ExpectMiss { text, stream } => write!(
                formatter,
                "{text:?} not found in unclaimed {} output",
                stream.label()
            ),
            Self::UnclaimedOutput { stream, text } => {
                write!(formatter, "unclaimed {} output: {text:?}", stream.label())
            }
        }
    }
}

#[derive(Default)]
struct StreamState {
    bytes: Vec<u8>,
    eof: bool,
}

#[derive(Default)]
struct StreamBuffer {
    state: Mutex<StreamState>,
    condvar: Condvar,
}

enum WaitOutcome {
    Matched(Range<usize>),
    Eof,
    Timeout,
}

impl StreamBuffer {
    fn wait_for(&self, needle: &[u8], cursor: usize, deadline: Instant) -> WaitOutcome {
        let mut state = self.state.lock().unwrap();
        loop {
            if let Some(offset) = find(state.bytes.get(cursor..).unwrap(), needle) {
                let start = cursor + offset;
                return WaitOutcome::Matched(start..start + needle.len());
            }
            if state.eof {
                return WaitOutcome::Eof;
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return WaitOutcome::Timeout;
            };
            let (guard, _) = self.condvar.wait_timeout(state, remaining).unwrap();
            state = guard;
        }
    }

    fn wait_eof(&self, deadline: Instant) {
        let mut state = self.state.lock().unwrap();
        while !state.eof {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return;
            };
            let (guard, _) = self.condvar.wait_timeout(state, remaining).unwrap();
            state = guard;
        }
    }
}

fn spawn_reader(source: File, buffer: Arc<StreamBuffer>) {
    thread::spawn(move || {
        let mut source = source;
        let mut chunk = [0u8; 4096];
        loop {
            // Read without the lock held: a silent child must not block wait_for's deadline.
            let result = source.read(&mut chunk);
            let mut state = buffer.state.lock().unwrap();
            match result {
                // 0 bytes: the last slave end closed.
                Ok(0) => state.eof = true,
                Ok(count) => state.bytes.extend_from_slice(chunk.get(..count).unwrap()),
                Err(_) => state.eof = true,
            }
            buffer.condvar.notify_all();
            if state.eof {
                break;
            }
        }
    });
}

#[derive(Default)]
struct Claims {
    intervals: Vec<Range<usize>>,
}

impl Claims {
    fn overlaps(&self, range: &Range<usize>) -> bool {
        self.intervals
            .iter()
            .any(|claimed| claimed.start < range.end && range.start < claimed.end)
    }

    fn claim_first_unclaimed(&mut self, bytes: &[u8], needle: &[u8]) -> Option<Range<usize>> {
        let mut search_from = 0;
        while let Some(offset) = find(bytes.get(search_from..).unwrap(), needle) {
            let range = search_from + offset..search_from + offset + needle.len();
            if !self.overlaps(&range) {
                self.intervals.push(range.clone());
                return Some(range);
            }
            search_from = range.start + 1;
        }
        None
    }

    fn residue(&self, bytes: &[u8]) -> Vec<u8> {
        let mut sorted = self.intervals.clone();
        sorted.sort_by_key(|claimed| claimed.start);
        let (mut residue, mut position) = (Vec::new(), 0);
        for claimed in sorted {
            if claimed.start > position {
                residue.extend_from_slice(bytes.get(position..claimed.start).unwrap());
            }
            position = position.max(claimed.end);
        }
        residue.extend_from_slice(bytes.get(position..).unwrap());
        residue
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[derive(Default)]
struct Side {
    buffer: Arc<StreamBuffer>,
    claims: Claims,
    cursor: usize,
}

fn matches_expectation(expectation: &ExitExpectation, status: &ExitStatus) -> bool {
    match expectation {
        ExitExpectation::Code(code) => status.code() == Some(*code),
        ExitExpectation::Signal(signal) => status.signal() == Some(*signal),
    }
}

fn describe_expectation(expectation: &ExitExpectation) -> String {
    match expectation {
        ExitExpectation::Code(code) => format!("exit code {code}"),
        ExitExpectation::Signal(signal) => format!("death by signal {signal}"),
    }
}

fn describe_status(status: &ExitStatus) -> String {
    match (status.code(), status.signal()) {
        (Some(code), _) => describe_expectation(&ExitExpectation::Code(code)),
        (None, Some(signal)) => describe_expectation(&ExitExpectation::Signal(signal)),
        (None, None) => "unknown termination".to_string(),
    }
}

struct Session {
    pid: i32,
    writer: File,
    sides: [Side; 2],
    exit_rx: mpsc::Receiver<io::Result<ExitStatus>>,
    status: Option<ExitStatus>,
    failures: Vec<Failure>,
    expects: Vec<(String, Stream)>,
}

impl Session {
    fn launch(binary: &Path, case: &TestCase, scratch: &Path) -> Result<Self, Failure> {
        // stdin+stdout share one pty; stderr gets its own so streams stay attributable.
        let inout = pty::open().map_err(Failure::Pty)?;
        let err = pty::open().map_err(Failure::Pty)?;
        let mut child =
            utils::spawn_child(binary, case, scratch, &inout, &err).map_err(Failure::Spawn)?;
        drop(inout.slave);
        drop(err.slave);

        let sides: [Side; 2] = std::array::from_fn(|_| Side::default());
        let (inout_reader, writer) = pty::split(inout.master).map_err(Failure::Pty)?;
        spawn_reader(
            inout_reader,
            Arc::clone(&sides.get(Stream::Stdout.index()).unwrap().buffer),
        );
        spawn_reader(
            File::from(err.master),
            Arc::clone(&sides.get(Stream::Stderr.index()).unwrap().buffer),
        );

        let pid = child.id() as i32;
        let (exit_tx, exit_rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = exit_tx.send(child.wait());
        });

        Ok(Self {
            pid,
            writer,
            sides,
            exit_rx,
            status: None,
            failures: Vec::new(),
            expects: Vec::new(),
        })
    }

    fn run_directives(&mut self, directives: &[Directive]) -> Result<(), Vec<Failure>> {
        for directive in directives {
            match directive {
                Directive::WaitFor { text, stream } => self.await_text(text, *stream)?,
                Directive::Expect { text, stream } => self.expects.push((text.clone(), *stream)),
                Directive::Input(text) => self.send_input(text.as_bytes()),
                // ICANON stays on, so VEOF delivers EOF to the child's reads.
                Directive::Eof => self.send_input(b"\x04"),
                Directive::Signal(signal) => self.signal(*signal),
            }
        }
        Ok(())
    }

    fn await_text(&mut self, text: &str, stream: Stream) -> Result<(), Vec<Failure>> {
        let side = self.sides.get_mut(stream.index()).unwrap();
        let needle = format!("{text}\n");
        let deadline = Instant::now() + DIRECTIVE_TIMEOUT;
        match side
            .buffer
            .wait_for(needle.as_bytes(), side.cursor, deadline)
        {
            WaitOutcome::Matched(range) => {
                side.cursor = range.end;
                side.claims.intervals.push(range);
                Ok(())
            }
            outcome => {
                let (text, stream) = (text.to_string(), stream);
                self.failures.push(match outcome {
                    WaitOutcome::Eof => Failure::WaitEof { text, stream },
                    _ => Failure::WaitTimeout { text, stream },
                });
                Err(self.abort())
            }
        }
    }

    fn send_input(&mut self, bytes: &[u8]) {
        if let Err(error) = self.writer.write_all(bytes) {
            self.failures.push(Failure::WriteFailed(error));
        }
    }

    fn signal(&mut self, signal: i32) {
        if let Err(error) = utils::kill(self.pid, signal) {
            self.failures.push(Failure::SignalFailed { signal, error });
        }
    }

    fn abort(&mut self) -> Vec<Failure> {
        let _ = utils::kill(self.pid, libc::SIGKILL);
        // A zombie reaps with its real status even after SIGKILL, so this
        // records exit 0 for a child that already finished on its own.
        self.status = utils::reap(&self.exit_rx);
        std::mem::take(&mut self.failures)
    }

    fn verify_exit(&mut self, exit: Option<ExitExpectation>) {
        let expectation = exit.unwrap_or(ExitExpectation::Code(0));
        match self.exit_rx.recv_timeout(DIRECTIVE_TIMEOUT) {
            Ok(Ok(status)) => {
                self.status = Some(status);
                if !matches_expectation(&expectation, &status) {
                    self.failures.push(Failure::ExitMismatch {
                        expected: describe_expectation(&expectation),
                        actual: describe_status(&status),
                    });
                }
            }
            Ok(Err(error)) => self.failures.push(Failure::Wait(error)),
            Err(_) => {
                let _ = utils::kill(self.pid, libc::SIGKILL);
                self.failures.push(Failure::ExitTimeout);
                self.status = utils::reap(&self.exit_rx);
            }
        }
    }

    fn verify_output(&mut self) {
        let drain_deadline = Instant::now() + DIRECTIVE_TIMEOUT;
        for side in &self.sides {
            side.buffer.wait_eof(drain_deadline);
        }

        for (text, stream) in std::mem::take(&mut self.expects) {
            let side = self.sides.get_mut(stream.index()).unwrap();
            let state = side.buffer.state.lock().unwrap();
            let needle = format!("{text}\n");
            if side
                .claims
                .claim_first_unclaimed(&state.bytes, needle.as_bytes())
                .is_none()
            {
                self.failures.push(Failure::ExpectMiss { text, stream });
            }
        }

        for (stream, side) in [Stream::Stdout, Stream::Stderr]
            .into_iter()
            .zip(self.sides.iter())
        {
            let state = side.buffer.state.lock().unwrap();
            let residue = side.claims.residue(&state.bytes);
            if residue.is_empty() {
                continue;
            }
            self.failures.push(Failure::UnclaimedOutput {
                stream,
                text: String::from_utf8_lossy(&residue).into_owned(),
            });
        }
    }

    fn finish(&mut self) -> Result<(), Vec<Failure>> {
        if self.failures.is_empty() {
            Ok(())
        } else {
            Err(std::mem::take(&mut self.failures))
        }
    }
}

const CAPTURE_LIMIT: usize = 1024;

pub struct RunError {
    failures: Vec<Failure>,
    status: Option<String>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl RunError {
    fn without_capture(failures: Vec<Failure>) -> Self {
        Self {
            failures,
            status: None,
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    }

    fn new(failures: Vec<Failure>, session: &Session) -> Self {
        // Drain so the snapshot includes bytes still in flight to the reader threads.
        let drain_deadline = Instant::now() + DIRECTIVE_TIMEOUT;
        for side in &session.sides {
            side.buffer.wait_eof(drain_deadline);
        }
        let snapshot = |stream: Stream| {
            let side = session.sides.get(stream.index()).unwrap();
            side.buffer.state.lock().unwrap().bytes.clone()
        };
        Self {
            failures,
            status: session.status.as_ref().map(describe_status),
            stdout: snapshot(Stream::Stdout),
            stderr: snapshot(Stream::Stderr),
        }
    }

    pub fn emit(&self) {
        for failure in &self.failures {
            println!("  {failure}");
        }
        if let Some(status) = &self.status {
            println!("  status: {status}");
        }
        println!("  captured stdout: {}", escaped(&self.stdout));
        println!("  captured stderr: {}", escaped(&self.stderr));
    }
}

fn escaped(bytes: &[u8]) -> String {
    let truncated = bytes.len() > CAPTURE_LIMIT;
    let shown = bytes.get(..CAPTURE_LIMIT).unwrap_or(bytes);
    let mut text = format!("{:?}", String::from_utf8_lossy(shown));
    if truncated {
        text.push_str(&format!(" … ({} bytes total)", bytes.len()));
    }
    text
}

pub enum LoadError {
    Read(io::Error),
    Parse {
        source_code: String,
        diagnostics: Vec<Diagnostic>,
    },
}

pub struct TestRunner {
    source: PathBuf,
    stem: String,
    case: TestCase,
}

impl TestRunner {
    pub fn new(source: PathBuf) -> Result<Self, LoadError> {
        let source_code = fs::read_to_string(&source).map_err(LoadError::Read)?;
        let case = Parser::new(&source_code)
            .parse()
            .map_err(|diagnostics| LoadError::Parse {
                source_code,
                diagnostics,
            })?;
        let stem = source
            .file_stem()
            .expect("c file has a stem")
            .to_string_lossy()
            .into_owned();
        Ok(Self { source, stem, case })
    }

    pub fn run(&self) -> Result<(), RunError> {
        let scratch = utils::prepare_scratch(&self.stem);
        let result = self.run_in(&scratch);
        let _ = fs::remove_dir_all(&scratch);
        result
    }

    fn run_in(&self, scratch: &Path) -> Result<(), RunError> {
        // The source lives in examples/, so bin/ and fixtures/ sit beside it.
        let examples = self.source.parent().unwrap();
        let fixtures = examples.join("fixtures").join(&self.stem);
        if fixtures.exists() {
            utils::copy_recursive(&fixtures, scratch)
                .map_err(|error| RunError::without_capture(vec![Failure::Fixture(error)]))?;
        }

        let binary = examples.join("bin").join(&self.stem);
        let mut session = Session::launch(&binary, &self.case, scratch)
            .map_err(|failure| RunError::without_capture(vec![failure]))?;
        if let Err(failures) = session.run_directives(&self.case.directives) {
            return Err(RunError::new(failures, &session));
        }
        session.verify_exit(self.case.exit);
        session.verify_output();
        session
            .finish()
            .map_err(|failures| RunError::new(failures, &session))
    }
}
