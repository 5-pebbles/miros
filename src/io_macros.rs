macro_rules! syscall_assert {
    ($condition:expr $(, $message:expr)? $(,)?) => {
        if !$condition {
            print!("assertion ");
            $(
                print!("`");
                print!($message);
                print!("` ");
            )?
            print!(concat!(
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
