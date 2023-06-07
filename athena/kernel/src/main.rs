//! Kernel entry and executable. Ideally, this is just a thin wrapper over all of the kernel's
//! components.
#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(error_in_core)]
#![feature(never_type)]
#![feature(allocator_api)]
#![feature(abi_x86_interrupt)]
#![feature(negative_impls)]
#![feature(const_fn_floating_point_arithmetic)]
#![deny(absolute_paths_not_starting_with_crate)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(unsafe_op_in_unsafe_fn)]
#![warn(clippy::undocumented_unsafe_blocks)]

pub mod arch;
pub mod ksync;
pub mod sched;
pub mod sync;
pub mod sys;

extern crate alloc;

use alloc::sync::Arc;

use bootloader_api::config::Mapping;
use bootloader_api::{entry_point, BootInfo, BootloaderConfig};

use crate::arch::context::Context;
use crate::sync::{Mutex, Semaphore};

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    critical_section::with(|_| {
        println!("{}", info);
        loop {
            arch::inst::hlt();
        }
    })
}

/// Kernel's starting point.
fn kmain(bootinfo: &'static mut BootInfo) -> ! {
    crate::arch::interrupts::disable();
    // SAFETY: The bootinfo is directly provided by the bootloader.
    critical_section::with(|cs| {
        unsafe { sys::init(bootinfo, cs) };
        sched::init();
    });
    log::info!("Initialization sequence complete");
    const THREADS: usize = 10;

    let count = Arc::new(Mutex::new(0));
    let done_threads = Arc::new(Semaphore::new(0));

    for _ in 0..THREADS {
        let done_threads = Arc::clone(&done_threads);
        let count = Arc::clone(&count);
        sched::push(Context::kthread(move || {
            for _ in 0..100000 {
                let mut count = count.lock();
                let cached = *count;
                sched::switch();
                *count = cached + 1;
            }
            done_threads.signal();
        }));
    }

    unsafe {
        crate::arch::interrupts::enable();
    }

    for _ in 0..THREADS {
        done_threads.wait();
    }
    dbg!(*count.lock());
    sched::exit();
}

const CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.kernel_stack_size = 4 * 1024 * 1024; // 4MiB
    config.mappings.dynamic_range_start = Some(0xFFFF_8000_0000_0000);
    config.mappings.dynamic_range_end = Some(0xFFFF_9000_0000_0000);
    config.mappings.physical_memory = Some(Mapping::FixedAddress(0xFFFF_F000_0000_0000));
    config.mappings.kernel_stack = Mapping::FixedAddress(0xFFFF_EFFF_FFBF_0000);
    config.mappings.boot_info = Mapping::FixedAddress(0xFFFF_EFFF_FFFF_0000);
    config.mappings.framebuffer = Mapping::FixedAddress(0xFFFF_A000_0000_0000);
    config
};
entry_point!(kmain, config = &CONFIG);

struct SingleThreadCS();
critical_section::set_impl!(SingleThreadCS);
/// SAFETY: While the OS kernel is running in a single thread, then disabling interrupts is a safe
/// to guarantee a critical section's conditions.
unsafe impl critical_section::Impl for SingleThreadCS {
    unsafe fn acquire() -> critical_section::RawRestoreState {
        let interrupts_enabled = arch::interrupts::are_enabled();
        arch::interrupts::disable();
        interrupts_enabled
    }

    unsafe fn release(interrupts_were_enabled: critical_section::RawRestoreState) {
        if interrupts_were_enabled {
            unsafe {
                arch::interrupts::enable();
            }
        }
    }
}
