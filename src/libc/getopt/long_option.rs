use core::ffi::{c_char, c_int};
use std::{ffi::CStr, ptr};

use super::{argument::ArgumentRequirement, parser::Parser, state, syntax};

impl Parser<'_> {
    pub(super) fn long_option(&mut self, element: *mut c_char, longindex: *mut c_int) -> c_int {
        // SAFETY: `next` pointed `next_char` into `element`.
        let remainder = unsafe { CStr::from_ptr(self.permutation.next_char) }.to_bytes();
        let (prefix, argument) = match remainder.iter().position(|&byte| byte == b'=') {
            Some(equals) => (&remainder[..equals], Some(&remainder[equals + 1..])),
            None => (remainder, None),
        };

        let mut found: Option<(c_int, &libc::option)> = None;
        let mut exact = false;
        let mut ambiguous = false;
        for (index, option) in self.long_options_slice().iter().enumerate() {
            // SAFETY: the option table ends at the first entry with a null name.
            let name = unsafe { CStr::from_ptr(option.name) }.to_bytes();
            if !name.starts_with(prefix) {
                continue;
            }
            if name.len() == prefix.len() {
                found = Some((index as c_int, option));
                exact = true;
                break;
            }
            match found {
                None => found = Some((index as c_int, option)),
                Some((_, previous)) => {
                    ambiguous |= previous.has_arg != option.has_arg
                        || previous.flag != option.flag
                        || previous.val != option.val;
                }
            }
        }

        // The whole element is consumed either way, including `=argument`.
        let element_end = unsafe { self.permutation.next_char.add(remainder.len()) };

        if ambiguous && !exact {
            self.reporter
                .ambiguous(element, prefix, self.long_options_slice());
            self.permutation.next_char = element_end;
            state::set_option_index(state::option_index() + 1);
            state::set_option_character(0);
            return b'?' as c_int;
        }

        let Some((found_index, option)) = found else {
            self.reporter.unrecognized(element);
            self.permutation.next_char = ptr::null_mut();
            state::set_option_index(state::option_index() + 1);
            state::set_option_character(0);
            return b'?' as c_int;
        };

        state::set_option_index(state::option_index() + 1);
        let requirement = ArgumentRequirement::from_raw(option.has_arg);
        // SAFETY: the option table ends at the first entry with a null name.
        let name = unsafe { CStr::from_ptr(option.name) }.to_bytes();

        match (argument, requirement) {
            (Some(_), ArgumentRequirement::None) => {
                self.reporter.argument_not_allowed(name);
                self.permutation.next_char = element_end;
                state::set_option_character(option.val);
                return b'?' as c_int;
            }
            (Some(value), _) => state::set_argument(value.as_ptr().cast_mut().cast()),
            (None, ArgumentRequirement::Required)
                if state::option_index() < self.argument_count() =>
            {
                state::set_argument(syntax::element(self.argv, state::option_index()));
                state::set_option_index(state::option_index() + 1);
            }
            (None, ArgumentRequirement::Required) => {
                self.reporter.argument_required(name);
                self.permutation.next_char = element_end;
                state::set_option_character(option.val);
                return if self.option_string.first() == Some(&b':') {
                    b':' as c_int
                } else {
                    b'?' as c_int
                };
            }
            (None, ArgumentRequirement::None | ArgumentRequirement::Optional) => {}
        }

        self.permutation.next_char = element_end;
        if !longindex.is_null() {
            unsafe { *longindex = found_index };
        }
        if !option.flag.is_null() {
            unsafe { *option.flag = option.val };
            return 0;
        }
        option.val
    }
}
