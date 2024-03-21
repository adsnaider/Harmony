#![no_std]
#![feature(naked_functions)]

pub mod raw {
    use core::arch::asm;

    #[naked]
    pub unsafe extern "sysv64" fn syscall(cap: usize, op: usize, a: usize, b: usize) -> isize {
        // NOTE: We don't need to align the stack on an int instruction.
        asm!("int 0x80", "ret", options(noreturn));
    }
}

pub mod serial {
    use core::fmt::Write;

    use crate::raw::syscall;

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
        unsafe { syscall(usize::MAX, 0, msg.as_ptr() as usize, msg.len()) };
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

/// A capability to a capability table
pub struct CapTableCap {
    cap: u32,
}

impl CapTableCap {
    pub fn new(cap: u32) -> Self {
        Self { cap }
    }
}
