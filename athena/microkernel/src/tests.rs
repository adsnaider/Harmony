use crate::arch::port::Port;
use crate::{sprint, sprintln};

#[cfg(test)]
#[no_mangle]
unsafe extern "C" fn kmain() -> ! {
    crate::init();
    crate::test_main();
    exit_qemu(QemuExitCode::Success)
}

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    critical_section::with(|_| {
        sprintln!("{}", info);
        exit_qemu(QemuExitCode::Failed)
    })
}

pub fn runner(tests: &[&dyn Testable]) {
    sprintln!("Running {} tests", tests.len());
    for (i, test) in tests.iter().enumerate() {
        sprint!("{}/{} - ", i + 1, tests.len());
        test.run();
    }
}

pub(crate) trait Testable {
    fn run(&self);
}

impl<T: Fn()> Testable for T {
    fn run(&self) {
        sprint!("{}...\t", core::any::type_name::<T>());
        self();
        sprintln!("[ok]");
    }
}

#[repr(u32)]
#[allow(dead_code)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) -> ! {
    // SAFETY: Port has no other side effects.
    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32)
    }
    unreachable!();
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(2 + 2, 4);
}
