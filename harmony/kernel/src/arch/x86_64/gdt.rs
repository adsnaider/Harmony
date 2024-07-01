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

struct Selectors {
    code_selector: SegmentSelector,
    data_selector: SegmentSelector,
    _user_code_selector: SegmentSelector,
    _user_data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

#[used]
static mut INTERRUPT_STACK: [u8; PAGE_SIZE] = [0; PAGE_SIZE];
// FIXME: This needs to be per-core.
static TSS: AtomicLazyCell<TaskStateSegment> = AtomicLazyCell::new(|| {
    let mut tss = TaskStateSegment::new();
    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
        const STACK_SIZE: usize = PAGE_SIZE;
        #[used]
        static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

        // SAFETY: Although it's a static mut, STACK is only used in this context.
        let stack_start = VirtAddr::from_ptr(unsafe { STACK.as_slice() });
        stack_start + STACK_SIZE as u64 // stack end.
    };
    // Privilege stack table used on interrupts.
    tss.privilege_stack_table[0] = {
        // SAFETY: The interrupt stack is (almost) only used as, well, a stack. Other than getting the pointer
        // to define it in the TSS we don't do anything else... except for reading the pushed registers
        // on an interrupt/syscall so that we can save them in case of a thread dispatch *cough* *cough*.
        interrupt_stack_end()
    };
    tss
});

pub(super) fn interrupt_stack_end() -> VirtAddr {
    let start = VirtAddr::from_ptr(unsafe { INTERRUPT_STACK.as_ptr() });
    start + PAGE_SIZE as u64
}

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
    log::info!("Initialized the GDT");
}
