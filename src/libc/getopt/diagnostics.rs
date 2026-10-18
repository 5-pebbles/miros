use core::ffi::c_char;
use std::ffi::CStr;

use super::state;
use crate::libc::stdio::write_stderr;

/// Writes getopt's messages to stderr. The parser decides which message; this type owns all of the text.
pub(super) struct Reporter {
    enabled: bool,
    argument_zero: *mut c_char,
}

impl Reporter {
    pub(super) fn new(option_string: &[u8], argument_zero: *mut c_char) -> Self {
        // A leading colon in the option string suppresses messages, just like `opterr = 0`.
        let enabled = state::errors_enabled() && option_string.first() != Some(&b':');
        Self {
            enabled,
            argument_zero,
        }
    }

    pub(super) fn unrecognized(&self, element: *mut c_char) {
        self.begin();
        self.write(b"unrecognized option '");
        self.write_c_string(element);
        self.write(b"'\n");
    }

    pub(super) fn ambiguous(
        &self,
        element: *mut c_char,
        prefix: &[u8],
        long_options: &[libc::option],
    ) {
        self.begin();
        self.write(b"option '");
        self.write_c_string(element);
        self.write(b"' is ambiguous; possibilities:");
        for option in long_options {
            // SAFETY: the option table ends at the first entry with a null name.
            let name = unsafe { CStr::from_ptr(option.name) }.to_bytes();
            if name.starts_with(prefix) {
                self.write(b" '--");
                self.write(name);
                self.write(b"'");
            }
        }
        self.write(b"\n");
    }

    pub(super) fn argument_not_allowed(&self, name: &[u8]) {
        self.begin();
        self.write(b"option '--");
        self.write(name);
        self.write(b"' doesn't allow an argument\n");
    }

    pub(super) fn argument_required(&self, name: &[u8]) {
        self.begin();
        self.write(b"option '--");
        self.write(name);
        self.write(b"' requires an argument\n");
    }

    pub(super) fn short_argument_required(&self, character: u8) {
        self.begin();
        self.write(b"option requires an argument -- '");
        self.write(&[character]);
        self.write(b"'\n");
    }

    pub(super) fn invalid_short_option(&self, character: u8) {
        self.begin();
        self.write(b"invalid option -- '");
        self.write(&[character]);
        self.write(b"'\n");
    }

    fn begin(&self) {
        self.write_c_string(self.argument_zero);
        self.write(b": ");
    }

    fn write(&self, bytes: &[u8]) {
        if self.enabled {
            // SAFETY: the stdio stream layer owns the locking.
            unsafe { write_stderr(bytes) };
        }
    }

    fn write_c_string(&self, string: *mut c_char) {
        if !string.is_null() {
            // SAFETY: argv entries and option names are NUL-terminated.
            self.write(unsafe { CStr::from_ptr(string) }.to_bytes());
        }
    }
}
