use core::ffi::c_int;
use std::ptr;

use super::{parser::Parser, state, syntax};

impl Parser<'_> {
    pub(super) fn short_option(&mut self) -> c_int {
        // SAFETY: `next` pointed `next_char` into the current cluster.
        let character = unsafe { *self.permutation.next_char as u8 };
        self.permutation.next_char = unsafe { self.permutation.next_char.add(1) };
        if unsafe { *self.permutation.next_char } == 0 {
            state::set_option_index(state::option_index() + 1);
        }

        let position = self
            .option_string
            .iter()
            .position(|&byte| byte == character)
            .filter(|_| character != b':');
        let Some(position) = position else {
            return self.report_invalid(character);
        };

        if self.option_string.get(position + 1) == Some(&b':') {
            let optional = self.option_string.get(position + 2) == Some(&b':');
            let attached = unsafe { *self.permutation.next_char } != 0;
            if attached {
                state::set_argument(self.permutation.next_char);
                state::set_option_index(state::option_index() + 1);
            } else if !optional {
                if state::option_index() == self.argument_count() {
                    self.reporter.short_argument_required(character);
                    state::set_option_character(character as c_int);
                    return if self.option_string.first() == Some(&b':') {
                        b':' as c_int
                    } else {
                        b'?' as c_int
                    };
                }
                state::set_argument(syntax::element(self.argv, state::option_index()));
                state::set_option_index(state::option_index() + 1);
            }
            self.permutation.next_char = ptr::null_mut();
        }

        character as c_int
    }
}
