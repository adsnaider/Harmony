use super::kthread::KThread;
use super::HasContext;
use crate::arch::mm::paging::{AddrSpace, PageTableFlags};
use crate::arch::mm::{Frame, VirtPage};
use crate::arch::PRIVILEGE_STACK_ADDR;
use crate::proc::Process;

#[derive(Debug)]
pub struct UThread {
    thread: KThread,
}

impl UThread {
    pub fn new(program: &[u8]) -> Option<Self> {
        log::debug!("Creating new UThread");
        let mut addrspace = AddrSpace::new()?;
        unsafe {
            addrspace.activate();
        }
        log::debug!("Setting up address space");
        let interrupt_stack = Frame::alloc().unwrap();
        let interrupt_stack_page = VirtPage::from_start_address(PRIVILEGE_STACK_ADDR).unwrap();
        unsafe {
            let _ = addrspace.unmap(interrupt_stack_page);
            addrspace
                .map_to(
                    interrupt_stack_page,
                    interrupt_stack,
                    PageTableFlags::WRITABLE | PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
                )
                .unwrap();
        }
        log::debug!("Mapped interrupt stack");
        let process = Process::load(program, 1, &mut addrspace).unwrap();
        log::debug!("Loaded process");
        Some(Self {
            thread: KThread::new(move || unsafe {
                log::debug!("Executing process!");
                process.exec();
            }),
        })
    }
}

impl HasContext for UThread {
    fn context(&self) -> *const crate::arch::context::Context {
        self.thread.context()
    }

    fn context_mut(&mut self) -> *mut crate::arch::context::Context {
        self.thread.context_mut()
    }
}
