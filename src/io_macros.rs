macro_rules! syscall_assert {
    ($condition:expr $(, $message:expr)? $(,)?) => {
        if !$condition {
            $crate::syscall::io::write(2, "assertion ");

            $(
                $crate::syscall::io::write(2, "`");
                $crate::syscall::io::write(2, $message);
                $crate::syscall::io::write(2, "` ");
            )?

            $crate::syscall::io::write(2, concat!(
                "failed: ",
                stringify!($condition), "\n",
                "  --> ",
                file!(), ":",
                line!(), ":",
                column!(), "\n",
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
