use super::{
    diagnostic::{DiagKind, Diagnostic},
    directive::{Directive, ExitExpectation, Stream, TestCase},
    span::Span,
};

macro_rules! signal_table {
    ($($name:ident),+ $(,)?) => {
        fn signal_number(name: &str) -> Option<i32> {
            Some(match name {
                $(stringify!($name) => libc::$name,)+
                _ => return None,
            })
        }
    };
}

signal_table! {
    SIGHUP, SIGINT, SIGQUIT, SIGILL, SIGTRAP, SIGABRT, SIGBUS, SIGFPE, SIGKILL,
    SIGUSR1, SIGSEGV, SIGUSR2, SIGPIPE, SIGALRM, SIGTERM, SIGCHLD, SIGCONT,
    SIGSTOP, SIGTSTP, SIGTTIN, SIGTTOU, SIGURG, SIGXCPU, SIGXFSZ, SIGVTALRM,
    SIGPROF, SIGWINCH, SIGIO, SIGPWR, SIGSYS
}

fn looks_like_integer(word: &str) -> bool {
    word.starts_with(|character: char| character.is_ascii_digit() || character == '-')
}

/// A `//` comment opens a directive only when it is the first token on its line; inline `//` and block comments are ignored.
/// A stand-alone `//` that does not parse as a directive is an error.
pub struct Parser<'a> {
    source: &'a str,
    position: usize,
    current_stream: Stream,
    test_case: TestCase,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
            current_stream: Stream::Stdout,

            test_case: Default::default(),
            diagnostics: Default::default(),
        }
    }

    pub fn parse(mut self) -> Result<TestCase, Vec<Diagnostic>> {
        let mut line_offset = 0;
        for line in self.source.split_inclusive('\n') {
            self.position = line_offset;
            line_offset += line.len();
            if let Err(diagnostic) = self.parse_line() {
                self.diagnostics.push(diagnostic);
            }
        }

        if self.diagnostics.is_empty() {
            Ok(self.test_case)
        } else {
            Err(self.diagnostics)
        }
    }

    fn parse_line(&mut self) -> Result<(), Diagnostic> {
        self.skip_whitespace();
        if !self.rest().starts_with("//") {
            return Ok(());
        }
        self.position += 2;
        self.skip_whitespace();

        let start = self.position;
        let word = self.word();
        let span = self.span_from(start);

        let directive = match word {
            "WAIT-FOR" => Directive::WaitFor {
                text: self.string()?,
                stream: self.current_stream,
            },
            "EXPECT" => Directive::Expect {
                text: self.string()?,
                stream: self.current_stream,
            },
            "INPUT" => Directive::Input(self.string()?),
            "EOF" => Directive::Eof,
            "SIGNAL" => Directive::Signal(self.signal()?),
            "ARGS" => return self.set_args(span),
            "EXIT" => return self.set_exit(span),
            "STDOUT" => return self.select_stream(Stream::Stdout),
            "STDERR" => return self.select_stream(Stream::Stderr),
            "NO-TTY" => return self.set_no_tty(),
            _ => {
                return Err(Diagnostic {
                    span,
                    kind: DiagKind::UnknownDirective,
                });
            }
        };
        self.end_of_line()?;
        self.test_case.directives.push(directive);
        Ok(())
    }

    fn set_args(&mut self, span: Span) -> Result<(), Diagnostic> {
        if self.test_case.args.is_some() {
            return Err(Diagnostic {
                span,
                kind: DiagKind::DuplicateArgs,
            });
        }
        self.test_case.args = Some(self.string()?);
        self.end_of_line()
    }

    fn set_exit(&mut self, span: Span) -> Result<(), Diagnostic> {
        if self.test_case.exit.is_some() {
            return Err(Diagnostic {
                span,
                kind: DiagKind::DuplicateExit,
            });
        }
        self.test_case.exit = Some(self.exit_expectation()?);
        self.end_of_line()
    }

    fn select_stream(&mut self, stream: Stream) -> Result<(), Diagnostic> {
        self.current_stream = stream;
        self.end_of_line()
    }

    fn set_no_tty(&mut self) -> Result<(), Diagnostic> {
        self.skip_whitespace();
        let start = self.position;
        let word = self.word();
        let span = self.span_from(start);
        let stream = match word {
            "STDOUT" => Stream::Stdout,
            "STDERR" => Stream::Stderr,
            _ => {
                return Err(Diagnostic {
                    span,
                    kind: DiagKind::Expected {
                        expected: "STDOUT or STDERR",
                    },
                });
            }
        };
        let pipe = self.test_case.pipes.get_mut(stream.index()).unwrap();
        if *pipe {
            return Err(Diagnostic {
                span,
                kind: DiagKind::DuplicateNoTty(stream),
            });
        }
        *pipe = true;
        self.end_of_line()
    }

    fn string(&mut self) -> Result<String, Diagnostic> {
        self.skip_whitespace();
        let start = self.position;
        if self.peek() != Some('"') {
            return Err(Diagnostic {
                span: self.span_from(start),
                kind: DiagKind::Expected {
                    expected: "a quoted string",
                },
            });
        }
        let _ = self.bump();
        let mut unescaped = String::new();
        loop {
            match self.bump() {
                None => {
                    return Err(Diagnostic {
                        span: self.span_from(start),
                        kind: DiagKind::UnterminatedString,
                    });
                }
                Some('"') => return Ok(unescaped),
                Some('\\') => {
                    let escape_start = self.position - 1;
                    match self.bump() {
                        Some('n') => unescaped.push('\n'),
                        Some('t') => unescaped.push('\t'),
                        Some('r') => unescaped.push('\r'),
                        Some('\\') => unescaped.push('\\'),
                        Some('"') => unescaped.push('"'),
                        Some(other) => {
                            return Err(Diagnostic {
                                span: self.span_from(escape_start),
                                kind: DiagKind::UnknownEscape(other),
                            });
                        }
                        None => {
                            return Err(Diagnostic {
                                span: self.span_from(start),
                                kind: DiagKind::UnterminatedString,
                            });
                        }
                    }
                }
                Some(character) => unescaped.push(character),
            }
        }
    }

    fn signal(&mut self) -> Result<i32, Diagnostic> {
        self.skip_whitespace();
        let start = self.position;
        let word = self.word();
        let span = self.span_from(start);
        if word.is_empty() {
            return Err(Diagnostic {
                span,
                kind: DiagKind::Expected {
                    expected: "a signal name or number",
                },
            });
        }
        if looks_like_integer(word) {
            return self.integer(word, span);
        }
        signal_number(word).ok_or(Diagnostic {
            span,
            kind: DiagKind::UnknownSignal,
        })
    }

    fn exit_expectation(&mut self) -> Result<ExitExpectation, Diagnostic> {
        self.skip_whitespace();
        let start = self.position;
        let word = self.word();
        let span = self.span_from(start);
        if word == "SIGNAL" {
            return self.signal().map(ExitExpectation::Signal);
        }
        if looks_like_integer(word) {
            return self.integer(word, span).map(ExitExpectation::Code);
        }
        Err(Diagnostic {
            span,
            kind: DiagKind::Expected {
                expected: "an exit code or SIGNAL",
            },
        })
    }

    fn integer(&self, word: &str, span: Span) -> Result<i32, Diagnostic> {
        word.parse::<i32>().map_err(|_| Diagnostic {
            span,
            kind: DiagKind::InvalidInteger,
        })
    }

    fn end_of_line(&mut self) -> Result<(), Diagnostic> {
        self.skip_whitespace();
        let line_end = self.line_end();
        if self.position == line_end {
            return Ok(());
        }
        Err(Diagnostic {
            span: Span::new(self.position, line_end),
            kind: DiagKind::Expected {
                expected: "end of line",
            },
        })
    }

    fn line_end(&self) -> usize {
        self.source
            .get(self.position..)
            .unwrap()
            .find('\n')
            .map_or(self.source.len(), |offset| self.position + offset)
    }

    fn word(&mut self) -> &'a str {
        let start = self.position;
        self.bump_while(|character| character.is_ascii_alphanumeric() || character == '-');
        self.source.get(start..self.position).unwrap()
    }

    fn skip_whitespace(&mut self) {
        self.bump_while(|character| character.is_whitespace());
    }

    fn rest(&self) -> &'a str {
        self.source.get(self.position..self.line_end()).unwrap()
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.position += character.len_utf8();
        Some(character)
    }

    fn bump_while(&mut self, mut predicate: impl FnMut(char) -> bool) {
        while self.peek().is_some_and(&mut predicate) {
            self.bump();
        }
    }

    fn span_from(&self, start: usize) -> Span {
        Span::new(start, self.position)
    }
}
