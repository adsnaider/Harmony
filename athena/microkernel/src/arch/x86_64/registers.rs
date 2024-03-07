use core::arch::asm;

pub fn rflags() -> u64 {
    let rflags: u64;
    unsafe {
        asm!(
            "pushfq",
            "pop {rflags}",
            rflags = out(reg) rflags,
        )
    }
    rflags
}
