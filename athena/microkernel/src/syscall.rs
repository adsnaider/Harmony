//! Syscall invocations

use kapi::{CapError, CapId, SyscallArgs};

use crate::component::ThreadControlBlock;

pub extern "sysv64" fn handle(
    cap: usize,
    op: usize,
    a: usize,
    b: usize,
    c: usize,
    d: usize,
) -> isize {
    // FIXME:
    // Use as serial print.
    if cap == usize::MAX {
        use crate::sprint;
        let slice = unsafe { core::slice::from_raw_parts(a as *const u8, b) };
        let message = unsafe { core::str::from_utf8_unchecked(slice) };
        sprint!("{}", message);
        return 0;
    }

    fn inner(cap: usize, op: usize, args: SyscallArgs) -> Result<(), CapError> {
        let tcb = ThreadControlBlock::current();
        tcb.exercise_capability(CapId::from(cap as u32), op, args)
    }

    match inner(cap, op, SyscallArgs::new(a, b, c, d)) {
        Ok(()) => 0,
        Err(e) => e.to_errno(),
    }
}
