use core::ffi::{c_char, c_int};
use std::ptr;

use super::{state, syntax};

/// GNU permutation state, mirroring glibc's `getopt_data`. `optind` itself lives in the interposable cell.
pub(super) struct Permutation {
    pub(super) next_char: *mut c_char,
    pub(super) first_nonopt: c_int,
    pub(super) last_nonopt: c_int,
    pub(super) initialized: bool,
}

static mut PERMUTATION: Permutation = Permutation {
    next_char: ptr::null_mut(),
    first_nonopt: 0,
    last_nonopt: 0,
    initialized: false,
};

impl Permutation {
    pub(super) fn get() -> &'static mut Self {
        // SAFETY: getopt is not reentrant, matching glibc.
        unsafe { &mut *(&raw mut PERMUTATION) }
    }

    /// Finds the next option-bearing argument, rotating accumulated non-options past the options.
    pub(super) fn advance(
        &mut self,
        argv: &mut [*mut c_char],
        has_long_options: bool,
    ) -> Option<*mut c_char> {
        let argument_count = argv.len() as c_int;
        let option_index = state::option_index();
        self.last_nonopt = self.last_nonopt.min(option_index);
        self.first_nonopt = self.first_nonopt.min(option_index);

        let non_options_accumulated = self.first_nonopt != self.last_nonopt;
        if non_options_accumulated && self.last_nonopt != option_index {
            self.exchange(argv, option_index);
        } else if self.last_nonopt != option_index {
            self.first_nonopt = option_index;
        }

        let mut scan = option_index;
        while scan < argument_count && !syntax::is_option(syntax::element(argv, scan)) {
            scan += 1;
        }
        state::set_option_index(scan);
        self.last_nonopt = scan;

        if scan != argument_count && syntax::is_separator(syntax::element(argv, scan)) {
            let after_separator = scan + 1;
            state::set_option_index(after_separator);
            let non_options_accumulated = self.first_nonopt != self.last_nonopt;
            if non_options_accumulated && self.last_nonopt != after_separator {
                self.exchange(argv, after_separator);
            } else if self.first_nonopt == self.last_nonopt {
                self.first_nonopt = after_separator;
            }
            self.last_nonopt = argument_count;
            state::set_option_index(argument_count);
        }

        if state::option_index() == argument_count {
            if self.first_nonopt != self.last_nonopt {
                state::set_option_index(self.first_nonopt);
            }
            return None;
        }

        let element = syntax::element(argv, state::option_index());
        let is_long = has_long_options && unsafe { *element.add(1) } == b'-' as c_char;
        self.next_char = unsafe { element.add(1 + is_long as usize) };
        Some(element)
    }

    /// Rotates the non-option block `[first_nonopt, last_nonopt)` past the option block after it.
    fn exchange(&mut self, argv: &mut [*mut c_char], option_index: c_int) {
        let first_nonopt = self.first_nonopt as usize;
        let last_nonopt = self.last_nonopt as usize;
        argv[first_nonopt..option_index as usize].rotate_left(last_nonopt - first_nonopt);
        self.first_nonopt += option_index - self.last_nonopt;
        self.last_nonopt = option_index;
    }
}
