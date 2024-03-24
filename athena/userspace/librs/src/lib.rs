#![no_std]

pub use kapi;

pub mod serial {
    use core::fmt::Write;

    use kapi::raw_syscall;

    #[macro_export]
    macro_rules! print {
    ($($arg:tt)*) => {
        use ::core::fmt::Write;
        write!($crate::dbg_out(), $($arg)*).unwrap();
    };
}

    #[macro_export]
    macro_rules! println {
    ($($arg:tt)*) => {{
        use ::core::fmt::Write;
        writeln!($crate::serial::_dbg_out(), $($arg)*).unwrap();
    }};
}

    fn write(msg: &str) {
        let msg = msg.as_bytes();
        unsafe { raw_syscall(usize::MAX, 0, msg.as_ptr() as usize, msg.len(), 0, 0) };
    }

    struct DebugOut;
    #[doc(hidden)]
    pub fn _dbg_out() -> impl Write {
        DebugOut {}
    }

    impl Write for DebugOut {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            write(s);
            Ok(())
        }
    }
}
