//! Kernel entry and executable. Ideally, this is just a thin wrapper over all of the kernel's
//! components.
#![no_std]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![warn(missing_copy_implementations)]

use core::panic::PanicInfo;

use bootinfo::Bootinfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
/// Kernel's starting point.
pub extern "C" fn kmain(bootinfo: &'static mut Bootinfo) -> ! {
    for row in 0..bootinfo.framebuffer.resolution.1 {
        for col in 0..bootinfo.framebuffer.resolution.0 {
            let color: u32 = 0x00_00_00_FF;
            let pixel = bootinfo
                .framebuffer
                .address
                .wrapping_add(row * bootinfo.framebuffer.stride * 4 + col * 4)
                as *mut u32;
            unsafe {
                core::ptr::write_volatile(pixel, color);
            }
        }
    }

    todo!();
}
