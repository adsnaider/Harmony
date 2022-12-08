//! Kernel entry and executable. Ideally, this is just a thin wrapper over all of the kernel's
//! components.
#![no_std]
#![feature(allocator_api)]
#![feature(const_fn_floating_point_arithmetic)]
#![feature(negative_impls)]
#![feature(default_alloc_error_handler)]
#![feature(abi_x86_interrupt)]
#![deny(absolute_paths_not_starting_with_crate)]
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(unsafe_op_in_unsafe_fn)]
#![warn(clippy::undocumented_unsafe_blocks)]

pub mod ksync;
pub(crate) mod singleton;
pub mod sys;

#[macro_use]
extern crate alloc;

use core::time::Duration;

use bootinfo::Bootinfo;

use crate::ksync::executor::Executor;
use crate::sys::time::sleep_sync;

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // Can't do much about errors at this point.
    let _ = println!("{}", info);
    loop {}
}

/// Kernel's starting point.
#[no_mangle]
pub extern "C" fn kmain(bootinfo: &'static mut Bootinfo) -> ! {
    // SAFETY: The bootinfo is directly provided by the bootloader.
    unsafe {
        sys::init(bootinfo);
    }
    log::info!("Initialization sequence complete.");

    let mut runtime = Executor::new();
    runtime.spawn(async {
        loop {
            print!(".");
            sleep_sync(Duration::from_secs(1));
        }
    });

    runtime.start();
}
