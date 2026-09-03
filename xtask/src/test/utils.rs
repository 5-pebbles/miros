use std::{
    fs, io,
    os::{fd::AsRawFd, unix::process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc,
};

use crate::test::{DIRECTIVE_TIMEOUT, directive::TestCase, pty};

pub fn prepare_scratch(stem: &str) -> PathBuf {
    let scratch = std::env::temp_dir().join(format!("miros-test-{stem}-{}", std::process::id()));
    if scratch.exists() {
        fs::remove_dir_all(&scratch).expect("clear stale scratch dir");
    }
    fs::create_dir_all(&scratch).expect("create scratch dir");
    scratch
}

pub fn copy_recursive(source: &Path, destination: &Path) -> io::Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            fs::create_dir_all(&target)?;
            copy_recursive(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

pub fn kill(pid: i32, signal: i32) -> io::Result<()> {
    // SAFETY: kill has no memory-safety preconditions.
    if unsafe { libc::kill(pid, signal) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn reap(exit_rx: &mpsc::Receiver<io::Result<ExitStatus>>) -> Option<ExitStatus> {
    exit_rx
        .recv_timeout(DIRECTIVE_TIMEOUT)
        .ok()
        .and_then(Result::ok)
}

pub fn spawn_child(
    binary: &Path,
    case: &TestCase,
    scratch: &Path,
    inout: &pty::Pty,
    err: &pty::Pty,
) -> io::Result<Child> {
    let mut command = Command::new(binary);
    command
        .current_dir(scratch)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(arguments) = &case.args {
        command.args(arguments.split_whitespace());
    }

    let inout_slave = inout.slave.as_raw_fd();
    let err_slave = err.slave.as_raw_fd();
    // SAFETY: dup2 clears FD_CLOEXEC on the targets,
    // so 0/1/2 survive exec while the CLOEXEC originals do not.
    unsafe {
        command.pre_exec(move || {
            for (target, source) in [inout_slave, inout_slave, err_slave]
                .into_iter()
                .enumerate()
            {
                if libc::dup2(source, target as i32) == -1 {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    command.spawn()
}
