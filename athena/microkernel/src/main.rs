#![no_std]
#![no_main]

use limine::request::FramebufferRequest;
use limine::BaseRevision;

/// Sets the base revision to the latest revision supported by the crate.
/// See specification for further info.
#[used]
static BASE_REVISION: BaseRevision = BaseRevision::new();

#[used]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // TODO: Reboot
    loop {}
}

#[no_mangle]
unsafe extern "C" fn kmain() -> ! {
    assert!(BASE_REVISION.is_supported());

    if let Some(framebuffer_response) = FRAMEBUFFER_REQUEST.get_response() {
        if let Some(framebuffer) = framebuffer_response.framebuffers().next() {
            for row in 0..framebuffer.height() {
                for col in 0..framebuffer.width() {
                    // Calculate the pixel offset using the framebuffer information we obtained above.
                    // We skip `i` scanlines (pitch is provided in bytes) and add `i * 4` to skip `i` pixels forward.
                    let pixel_offset = row * framebuffer.pitch() + col * 4;

                    // Write 0xFFFFFFFF to the provided pixel offset to fill it white.
                    core::ptr::write_volatile(
                        framebuffer.addr().add(pixel_offset as usize) as *mut u32,
                        0xFFFFFFFF,
                    );
                }
            }
        }
    }

    loop {}
}
