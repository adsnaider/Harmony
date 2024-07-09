use kapi::raw::{CapError, CapId, SyscallArgs};

use crate::component::Thread;
use crate::sprint;

pub extern "sysv64" fn handle(a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    log::debug!("SYSCALL: {a}, {b}, {c}, {d}, {e}, {f}");
    let thread = Thread::current().unwrap();

    let Ok(capability) = u32::try_from(a) else {
        log::debug!("Returning error");
        return CapError::InvalidArgument.to_errno();
    };
    let capability = CapId::from(capability);
    log::debug!("cap: {capability:?}");
    let args = SyscallArgs::new(b, c, d, e, f);
    log::debug!("args: {args:?}");
    match thread.component().exercise_cap(capability, args) {
        Ok(result) => result.try_into().unwrap(),
        Err(e) => e.to_errno(),
    }
}
