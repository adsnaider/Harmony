#![no_std]
#![no_main]

use limine::request::FramebufferRequest;
use limine::BaseRevision;

pub mod arch;

mod serial;

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
    serial::init();
    sprintln!("Hello serial");
    assert!(BASE_REVISION.is_supported());

    loop {}
}

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
            // SAFETY: Precondition.
            unsafe {
                arch::interrupts::enable();
            }
        }
    }
}
