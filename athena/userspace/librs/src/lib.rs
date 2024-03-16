#![no_std]
#![feature(naked_functions)]

use core::fmt::Write;

use raw::syscall;

pub mod raw {
    use core::arch::asm;

    #[naked]
    pub unsafe extern "sysv64" fn syscall(cap: usize, op: usize, a: usize, b: usize) -> isize {
        // NOTE: We don't need to align the stack on an int instruction.
        asm!("int 0x80", "ret", options(noreturn));
    }
}

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
        writeln!($crate::dbg_out(), $($arg)*).unwrap();
    }};
}

fn write(msg: &str) {
    let msg = msg.as_bytes();
    unsafe { syscall(usize::MAX, 0, msg.as_ptr() as usize, msg.len()) };
}

pub struct DebugOut;
pub fn dbg_out() -> DebugOut {
    DebugOut {}
}

impl Write for DebugOut {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        write(s);
        Ok(())
    }
}
