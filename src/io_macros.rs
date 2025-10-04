macro_rules! syscall_assert {
    ($condition:expr $(, $message:expr)? $(,)?) => {
        if !$condition {
            fn write_str(s: &str) {
                let bytes = s.as_bytes();
                $crate::syscall::write::write(
                    $crate::syscall::write::STD_ERR,
                    bytes.as_ptr() as *const std::ffi::c_void,
                    bytes.len()
                );
            }

            write_str("assertion ");
            $(
                write_str("`");
                write_str($message);
                write_str("` ");
            )?
            write_str(concat!(
                "failed: ", stringify!($condition), "\n",
                "  --> ", file!(), ":", line!(), ":", column!(), "\n",
            ));

            $crate::syscall::exit::exit(101);
        }
    };
}

pub(crate) use syscall_assert;

macro_rules! syscall_debug_assert {
    ($condition:expr $(, $message:expr)? $(,)?) => {
        #[cfg(debug_assertions)]
        {
            $crate::io_macros::syscall_assert!($condition $(, $message)?);
        }
    };
}

pub(crate) use syscall_debug_assert;
