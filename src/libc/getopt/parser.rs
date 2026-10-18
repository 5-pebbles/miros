use core::ffi::{c_char, c_int};
use std::ptr;

use super::{diagnostics::Reporter, permutation::Permutation, state, syntax};

pub(super) struct Parser<'a> {
    pub(super) argv: &'a mut [*mut c_char],
    pub(super) option_string: &'a [u8],
    pub(super) long_options: Option<&'a [libc::option]>,
    pub(super) permutation: &'a mut Permutation,
    pub(super) reporter: Reporter,
}

impl<'a> Parser<'a> {
    pub(super) fn new(
        argv: &'a mut [*mut c_char],
        option_string: &'a [u8],
        long_options: Option<&'a [libc::option]>,
        argument_zero: *mut c_char,
    ) -> Self {
        Self {
            argv,
            option_string,
            long_options,
            permutation: Permutation::get(),
            reporter: Reporter::new(option_string, argument_zero),
        }
    }

    /// Returns the next option's value, or -1 when the arguments are exhausted.
    pub(super) fn next(&mut self, longindex: *mut c_int) -> c_int {
        state::clear_argument();
        self.initialize();

        let starting_a_cluster =
            self.permutation.next_char.is_null() || unsafe { *self.permutation.next_char } == 0;
        let element = if starting_a_cluster {
            match self
                .permutation
                .advance(self.argv, self.long_options.is_some())
            {
                Some(element) => element,
                None => return -1,
            }
        } else {
            syntax::element(self.argv, state::option_index())
        };

        let is_long = self.long_options.is_some() && unsafe { *element.add(1) } == b'-' as c_char;
        if is_long {
            self.long_option(element, longindex)
        } else {
            self.short_option()
        }
    }

    fn initialize(&mut self) {
        let option_index = state::option_index();
        if option_index == 0 {
            state::set_option_index(1);
        }
        if option_index == 0 || !self.permutation.initialized {
            self.permutation.next_char = ptr::null_mut();
            self.permutation.first_nonopt = state::option_index();
            self.permutation.last_nonopt = state::option_index();
            self.permutation.initialized = true;
        }
    }

    pub(super) fn argument_count(&self) -> c_int {
        self.argv.len() as c_int
    }

    pub(super) fn long_options_slice(&self) -> &'a [libc::option] {
        self.long_options.unwrap_or(&[])
    }

    pub(super) fn report_invalid(&self, character: u8) -> c_int {
        self.reporter.invalid_short_option(character);
        state::set_option_character(character as c_int);
        b'?' as c_int
    }
}
