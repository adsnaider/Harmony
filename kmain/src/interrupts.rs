//! Interrupts initialization and handling.

use lazy_static::lazy_static;
use x86_64::instructions::tables::load_tss;
use x86_64::registers::segmentation::{Segment, CS};
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

use crate::println;

const DOUBLE_FAULT_IST_INDEX: u16 = 0;

/// Sets up the GDT with a TSS that is used for double fault handler stack.
fn init_gdt() {
    struct Selectors {
        code_selector: SegmentSelector,
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
            let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
            (
                gdt,
                Selectors {
                    code_selector,
                    tss_selector,
                },
            )
        };
    }
    GDT.0.load();
    // SAFETY: Segment selectors are valid, and appropriately setup in the GDT.
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        load_tss(GDT.1.tss_selector);
    }
}

/// Initializes the interrupt descriptor table.
fn init_idt() {
    lazy_static! {
        static ref IDT: InterruptDescriptorTable = {
            let mut idt = InterruptDescriptorTable::new();
            idt.breakpoint.set_handler_fn(breakpoint_handler);
            // SAFETY: Stack index provided is valid and only used for the double fault handler.
            unsafe {
                idt.double_fault
                    .set_handler_fn(double_fault_handler)
                    .set_stack_index(DOUBLE_FAULT_IST_INDEX);
            }
            idt
        };
    }
    IDT.load();
}

/// Initializes interrupts and exceptions alongside their predefined handlers.
pub fn init() {
    init_gdt();
    init_idt();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION BREAKPOINT:\n{stack_frame:#?}");
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT - {error_code}\n{stack_frame:#?}");
}
