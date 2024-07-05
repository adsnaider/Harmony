use kapi::raw::{CapError, CapId, SyscallArgs};

use crate::component::Thread;
use crate::{sprint, sprintln};

pub extern "sysv64" fn handle(a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> isize {
    if a == usize::MAX {
        let ptr = c as *const u8;
        let msg = unsafe { core::slice::from_raw_parts(ptr, d) };
        let msg = unsafe { core::str::from_utf8_unchecked(msg) };
        sprint!("{}", msg);
        return 0;
    }
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
    match thread.exercise_cap(capability, args) {
        Ok(result) => result.try_into().unwrap(),
        Err(e) => e.to_errno(),
    }
}
