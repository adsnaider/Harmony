//! Initialization of the Global Descriptor Table.

use lazy_static::lazy_static;
use x86_64::instructions::tables::load_tss;
use x86_64::registers::segmentation::{Segment, CS, DS, ES, FS, GS, SS};
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

/// The TSS stack table index to be used for the Double Fault exception.
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

/// Sets up the GDT with a TSS that is used for double fault handler stack, a
/// kernel code segment and a kernel data segment.
pub fn init() {
    struct Selectors {
        code_selector: SegmentSelector,
        data_selector: SegmentSelector,
        tss_selector: SegmentSelector,
    }
    lazy_static! {
        static ref TSS: TaskStateSegment = {
            let mut tss = TaskStateSegment::new();
            tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
                const STACK_SIZE: usize = 4096 * 5;
                static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

                // SAFETY: Although it's a static mut, STACK is only used in this context.
                let stack_start = VirtAddr::from_ptr(unsafe {&STACK});
                stack_start + STACK_SIZE // stack end.
            };
            tss
        };
        static ref GDT: (GlobalDescriptorTable, Selectors) = {
            let mut gdt = GlobalDescriptorTable::new();
            let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
            let data_selector = gdt.add_entry(Descriptor::kernel_data_segment());
            let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
            (
                gdt,
                Selectors {
                    code_selector,
                    data_selector,
                    tss_selector,
                },
            )
        };
    }
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
