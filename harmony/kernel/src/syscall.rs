use kapi::{CapError, CapId, SyscallArgs};

use crate::component::Thread;

pub extern "sysv64" fn handle(a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    let thread = Thread::current().unwrap();

    let Ok(capability) = u32::try_from(a) else {
        return CapError::InvalidArgument.to_errno();
    };
    let capability = CapId::from(capability);
    let operation = b;
    let args = SyscallArgs::new(c, d, e, f);
    match thread.exercise_cap(capability, operation, args) {
        Ok(result) => result.try_into().unwrap(),
        Err(e) => e.to_errno(),
    }
}
