use core::ffi::c_int;

/// What a long option's `has_arg` field means.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ArgumentRequirement {
    None,
    Required,
    Optional,
}

impl ArgumentRequirement {
    /// glibc treats any nonzero value other than `1` as "optional argument".
    pub(super) fn from_raw(raw: c_int) -> Self {
        match raw {
            0 => Self::None,
            1 => Self::Required,
            _ => Self::Optional,
        }
    }
}
