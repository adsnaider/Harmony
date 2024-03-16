//! Syscall invocations

use crate::component::ThreadControlBlock;

pub extern "sysv64" fn handle(cap: usize, op: usize, a: usize, b: usize) -> isize {
    // FIXME:
    // Use as serial print.
    if cap == usize::MAX {
        use crate::sprint;
        let slice = unsafe { core::slice::from_raw_parts(a as *const u8, b) };
        let message = unsafe { core::str::from_utf8_unchecked(slice) };
        sprint!("{}", message);
        return 0;
    }
    let tcb = ThreadControlBlock::current();
    match tcb.exercise(cap, op) {
        Ok(()) => 0,
        Err(e) => e.to_errno().into(),
    }
}
