//! Global descriptor table.

use sync::cell::AtomicLazyCell;
use x86_64_impl::instructions::tables::load_tss;
use x86_64_impl::registers::segmentation::{Segment, CS, DS, ES, FS, GS, SS};
use x86_64_impl::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64_impl::structures::tss::TaskStateSegment;
use x86_64_impl::VirtAddr;

use crate::arch::paging::PAGE_SIZE;

/// The TSS stack table index to be used for the Double Fault exception.
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
/// The TSS stack table index to be used for the Page Fault exception.
pub const PAGE_FAULT_IST_INDEX: u16 = 1;

/// The privilege stack for Ring 0 lives in this virtual address.
pub const PRIVILEGE_STACK_ADDR: u64 = 0xFFFF_FFFF_7000_0000;

struct Selectors {
    code_selector: SegmentSelector,
    data_selector: SegmentSelector,
    _user_code_selector: SegmentSelector,
    _user_data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}
static TSS: AtomicLazyCell<TaskStateSegment> = AtomicLazyCell::new(|| {
    let mut tss = TaskStateSegment::new();
    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
        const STACK_SIZE: usize = PAGE_SIZE;
        static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

        // SAFETY: Although it's a static mut, STACK is only used in this context.
        let stack_start = VirtAddr::from_ptr(unsafe { STACK.as_slice() });
        stack_start + STACK_SIZE as u64 // stack end.
    };
    tss.interrupt_stack_table[PAGE_FAULT_IST_INDEX as usize] = {
        const STACK_SIZE: usize = PAGE_SIZE;
        static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

        // SAFETY: Although it's a static mut, STACK is only used in this context.
        let stack_start = VirtAddr::from_ptr(unsafe { STACK.as_slice() });
        stack_start + STACK_SIZE as u64 // stack end.
    };
    // Privilege stack table used on interrupts.
    tss.privilege_stack_table[0] = {
        // Every user process will map an privilege stack page at `PRIVILEGE_STACK_ADDR`.
        const STACK_SIZE: usize = PAGE_SIZE;
        let stack_start = VirtAddr::new(PRIVILEGE_STACK_ADDR);
        stack_start + STACK_SIZE as u64 // stack end.
    };
    tss
});

static GDT: AtomicLazyCell<(GlobalDescriptorTable, Selectors)> = AtomicLazyCell::new(|| {
    let mut gdt = GlobalDescriptorTable::new();
    let code_selector = gdt.append(Descriptor::kernel_code_segment());
    let data_selector = gdt.append(Descriptor::kernel_data_segment());
    let user_code_selector = gdt.append(Descriptor::user_code_segment());
    let user_data_selector = gdt.append(Descriptor::user_data_segment());
    let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));
    (
        gdt,
        Selectors {
            code_selector,
            data_selector,
            _user_code_selector: user_code_selector,
            _user_data_selector: user_data_selector,
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
