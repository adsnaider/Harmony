//! Global descriptor table.

use core::arch::asm;

use once_cell::sync::Lazy;
use x86_64::instructions::tables::load_tss;
use x86_64::registers::segmentation::{Segment, CS, DS, ES, FS, GS, SS};
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

/// The TSS stack table index to be used for the Double Fault exception.
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
/// The TSS stack table index to be used for the Page Fault exception.
pub const PAGE_FAULT_IST_INDEX: u16 = 1;

/// The privilege stack for Ring 0 lives in this virtual address.
pub const PRIVILEGE_STACK_ADDR: u64 = 0xFFFF_B000_0000_0000;

struct Selectors {
    code_selector: SegmentSelector,
    data_selector: SegmentSelector,
    user_code_selector: SegmentSelector,
    user_data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}
static TSS: Lazy<TaskStateSegment> = Lazy::new(|| {
    let mut tss = TaskStateSegment::new();
    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
        const STACK_SIZE: usize = 4096;
        static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

        // SAFETY: Although it's a static mut, STACK is only used in this context.
        let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
        stack_start + STACK_SIZE // stack end.
    };
    tss.interrupt_stack_table[PAGE_FAULT_IST_INDEX as usize] = {
        const STACK_SIZE: usize = 4096;
        static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

        // SAFETY: Although it's a static mut, STACK is only used in this context.
        let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
        stack_start + STACK_SIZE // stack end.
    };
    // Privilege stack table used on interrupts.
    tss.privilege_stack_table[0] = {
        // Every user process will map an privilege stack page at `PRIVILEGE_STACK_ADDR`.
        const STACK_SIZE: usize = 4096;
        let stack_start = VirtAddr::new(PRIVILEGE_STACK_ADDR);
        stack_start + STACK_SIZE // stack end.
    };
    tss
});

static GDT: Lazy<(GlobalDescriptorTable, Selectors)> = Lazy::new(|| {
    let mut gdt = GlobalDescriptorTable::new();
    let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
    let data_selector = gdt.add_entry(Descriptor::kernel_data_segment());
    let user_code_selector = gdt.add_entry(Descriptor::user_code_segment());
    let user_data_selector = gdt.add_entry(Descriptor::user_data_segment());
    let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
    (
        gdt,
        Selectors {
            code_selector,
            data_selector,
            user_code_selector,
            user_data_selector,
            tss_selector,
        },
    )
});

/// Sets up the GDT with a TSS that is used for double fault handler stack, a
/// kernel code segment and a kernel data segment.
pub fn init() {
    GDT.0.load();
    // SAFETY: Segment selectors are valid, and appropriately setup in the GDT.
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        DS::set_reg(GDT.1.data_selector);
        ES::set_reg(GDT.1.data_selector);
        FS::set_reg(GDT.1.data_selector);
        GS::set_reg(GDT.1.data_selector);
        SS::set_reg(GDT.1.data_selector);
        load_tss(GDT.1.tss_selector);
    }
}

/// Performs a `sysret` operation.
///
/// This will set the stack pointer to `rsp` and perform a jump to `rip`.
/// The processor will be switched to ring 3.
///
/// # Safety
///
/// The `rip` and `rsp` must be valid entrypoints for a user space process loaded
/// into the current address space.
pub unsafe fn sysret(rip: u64, rsp: u64) -> ! {
    // SAFETY: This should be safe so long as rip and rsp are valid.
    unsafe {
        asm!(
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "push rax", // SS is handled by iret
            "push r11", // stack pointer
            "push 0x202", // rflags
            "push rcx", // CS with RPL 3
            "push r12",
            "iretq",
            in("rax") GDT.1.user_data_selector.0,
            in("rcx") GDT.1.user_code_selector.0,
            in("r11") rsp,
            in("r12") rip,
            options(noreturn)
        )
    }
}
