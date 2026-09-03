use std::{
    fs::{File, OpenOptions},
    io,
    os::fd::{AsRawFd, OwnedFd},
};

// ECHO off keeps INPUT bytes out of the output stream; OPOST off keeps `\n` untranslated.
// ICANON stays on so the runner can deliver EOF as the VEOF character.
fn terminal_settings() -> libc::termios {
    let mut settings: libc::termios = unsafe { std::mem::zeroed() };
    settings.c_iflag = 0;
    settings.c_oflag = 0;
    settings.c_cflag = libc::B38400 | libc::CS8 | libc::CREAD;
    settings.c_lflag = libc::ISIG | libc::ICANON | libc::IEXTEN;
    settings.c_cc[libc::VINTR] = 3; // ^C
    settings.c_cc[libc::VQUIT] = 28; // ^\
    settings.c_cc[libc::VERASE] = 127;
    settings.c_cc[libc::VKILL] = 21; // ^U
    settings.c_cc[libc::VEOF] = 4; // ^D
    settings.c_cc[libc::VSTART] = 17; // ^Q
    settings.c_cc[libc::VSTOP] = 19; // ^S
    settings.c_cc[libc::VSUSP] = 26; // ^Z
    settings.c_cc[libc::VREPRINT] = 18; // ^R
    settings.c_cc[libc::VDISCARD] = 15; // ^O
    settings.c_cc[libc::VWERASE] = 23; // ^W
    settings.c_cc[libc::VLNEXT] = 22; // ^V
    settings.c_cc[libc::VMIN] = 1;
    settings
}

pub struct Pty {
    pub master: OwnedFd,
    pub slave: OwnedFd,
}

pub fn open() -> io::Result<Pty> {
    let master = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/ptmx")?;
    let pty = unsafe {
        if libc::grantpt(master.as_raw_fd()) == -1 || libc::unlockpt(master.as_raw_fd()) == -1 {
            return Err(io::Error::last_os_error());
        }
        let name = libc::ptsname(master.as_raw_fd());
        if name.is_null() {
            return Err(io::Error::last_os_error());
        }
        let path = std::ffi::CStr::from_ptr(name);
        let slave = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path.to_string_lossy().as_ref())?;
        configure_terminal(&slave)?;
        Pty {
            master: master.into(),
            slave: slave.into(),
        }
    };
    Ok(pty)
}

pub fn split(master: OwnedFd) -> io::Result<(File, File)> {
    let reader = File::from(master);
    let writer = reader.try_clone()?;
    Ok((reader, writer))
}

fn configure_terminal(slave: &File) -> io::Result<()> {
    // SAFETY: fd is a valid terminal; settings is fully initialized.
    unsafe {
        if libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &terminal_settings()) == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}
