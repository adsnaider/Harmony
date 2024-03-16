//! Syscall invocations

use crate::component::ThreadControlBlock;

pub fn handle(cap: usize, op: usize) -> isize {
    let tcb = ThreadControlBlock::current();
    match tcb.exercise(cap, op) {
        Ok(()) => 0,
        Err(e) => e.to_errno().into(),
    }
}
