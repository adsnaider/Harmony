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
pub mod sys;

// #[macro_use]
extern crate alloc;

use bootloader_api::config::Mapping;
use bootloader_api::{BootInfo, BootloaderConfig};
use limine::LimineFramebufferRequest;

use crate::arch::context::Context;

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

static FRAMEBUFFER_REQUEST: LimineFramebufferRequest = LimineFramebufferRequest::new(0);

#[no_mangle]
extern "C" fn _start() -> ! {
    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response().get() {
        assert!(framebuffer_response.framebuffer_count > 0);
        // Get the first framebuffer's information.
        let framebuffer = &framebuffer_response.framebuffers()[0];

        for i in 0..100_usize {
            // Calculate the pixel offset using the framebuffer information we obtained above.
            // We skip `i` scanlines (pitch is provided in bytes) and add `i * 4` to skip `i` pixels forward.
            let pixel_offset = i * framebuffer.pitch as usize + i * 4;

            // Write 0xFFFFFFFF to the provided pixel offset to fill it white.
            // We can safely unwrap the result of `as_ptr()` because the framebuffer address is
            // guaranteed to be provided by the bootloader.
            unsafe {
                core::ptr::write_volatile(
                    framebuffer
                        .address
                        .as_ptr()
                        .unwrap()
                        .offset(pixel_offset as isize) as *mut u32,
                    0xFFFFFFFF,
                );
            }
        }
    }
    loop {}
}

/// Kernel's starting point.
fn kmain(bootinfo: &'static mut BootInfo) -> ! {
    crate::arch::int::disable();
    // SAFETY: The bootinfo is directly provided by the bootloader.
    critical_section::with(|_cs| {
        unsafe {
            sys::init(bootinfo);
        }
        sched::init();
    });
    log::info!("Initialization sequence complete");

    sched::push(Context::kthread(|| {
        for i in 0..20 {
            println!("Hi from task 1 - ({i})");
            core::hint::black_box(for _ in 0..1000000 {});
        }
    }));
    sched::push(Context::kthread(|| {
        for i in 0..20 {
            println!("Hi from task 2 - ({i})");
            core::hint::black_box(for _ in 0..1000000 {});
        }
    }));

    // SAFETY: No locks held, we disabled it at the start of the function.
    unsafe {
        crate::arch::int::enable();
    }
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
// entry_point!(kmain, config = &CONFIG);

struct SingleThreadCS();
critical_section::set_impl!(SingleThreadCS);
/// SAFETY: While the OS kernel is running in a single thread, then disabling interrupts is a safe
/// to guarantee a critical section's conditions.
unsafe impl critical_section::Impl for SingleThreadCS {
    unsafe fn acquire() -> critical_section::RawRestoreState {
        let interrupts_enabled = arch::int::are_enabled();
        arch::int::disable();
        interrupts_enabled
    }

    unsafe fn release(interrupts_were_enabled: critical_section::RawRestoreState) {
        if interrupts_were_enabled {
            unsafe {
                arch::int::enable();
            }
        }
    }
}
