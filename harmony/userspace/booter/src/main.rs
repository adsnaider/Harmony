#![no_std]
#![no_main]
#![feature(naked_functions)]

use core::cell::Cell;
use core::convert::Infallible;
use core::mem::MaybeUninit;
use core::ops::Range;

use entry::entry;
use kapi::ops::cap_table::{CapTableConsArgs, PageTableConsArgs, ThreadConsArgs};
use kapi::ops::memory::RetypeKind;
use kapi::ops::paging::PermissionMask;
use kapi::ops::SlotId;
use kapi::raw::CapId;
use kapi::userspace::cap_management::{FrameAllocator, SelfCapabilityManager};
use kapi::userspace::paging::addr::{Frame, Page, PageTableLevel, PhysAddr, VirtAddr};
use kapi::userspace::paging::{Addrspace, PageTableAllocator};
use kapi::userspace::structures::{HardwareAccess, PageTable, Retype};
use kapi::userspace::Booter;
use loader::{Loader, MemFlags, Program};
use serial::{sdbg, sprintln};
use tar_no_std::TarArchiveRef;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use serial::sprintln;

    sprintln!("{}", info);
    loop {}
}

struct FrameBumper(Cell<Frame>);
impl FrameBumper {
    pub fn new(start: Frame) -> Self {
        Self(Cell::new(start))
    }

    pub fn next(&self) -> Frame {
        let frame = self.0.get();
        self.0.set(Frame::from_start_address(PhysAddr::new(
            frame.base().as_u64() + 0x1000,
        )));
        frame
    }
}

impl FrameAllocator for &'_ FrameBumper {
    fn alloc_frame(&mut self) -> Frame {
        self.next()
    }
}

#[entry]
fn main(lowest_frame: usize, initrd: *const u8, initrd_size: usize) -> ! {
    let resources = Booter::make();

    resources.hardware.enable_ports().unwrap();
    serial::init();

    // SAFETY: Passed initrd info from the kernel is correct.
    let initrd = unsafe { core::slice::from_raw_parts(initrd, initrd_size) };
    log::info!(
        "Jumped to userspace: next_frame: {}, initrd: ({:?}, {})",
        lowest_frame,
        initrd.as_ptr(),
        initrd.len()
    );

    let frames = FrameBumper::new(Frame::from_start_address(PhysAddr::new(
        lowest_frame as u64,
    )));
    let mut cap_manager =
        SelfCapabilityManager::new_with_start(resources.self_caps, CapId::new(6), &frames);

    let initrd = TarArchiveRef::new(initrd).expect("Bad initramfs");
    log::info!("Initializing memory manager");
    let memory_manager = {
        let program = initrd
            .entries()
            .find(|entry| {
                entry.filename().as_str().expect("Invalid entry in initrd") == "memory_manager"
            })
            .expect("Missing memory_manager from initrd")
            .data();
        log::info!("Found process in initrd");

        let program = Program::new(program).expect("Invalid memory manager program");
        let l4_table_slot = cap_manager.allocate_capability().unwrap();
        log::info!("Allocated l4 table slot");
        let l4_table = l4_table_slot
            .make_page_table(PageTableConsArgs::new(frames.next(), 4))
            .expect("Couldn't create page table");
        log::info!("Allocated l4 table capability");
        let mut addrspace = unsafe { Addrspace::new(l4_table) };

        log::info!("Setting up process address space");
        let mut loader = UserspaceLoader {
            other_addrspace: &mut addrspace,
            cap_management: &mut cap_manager,
            self_addrspace: &mut unsafe { Addrspace::new(resources.self_paging) },
            frame_allocator: &frames,
            retype_cap: &resources.retype,
            hardware: &resources.hardware,
        };
        let process = program
            .load(&mut loader)
            .expect("Couldn't load the program");
        log::info!("Loaded the process");

        const STACK_TOP: usize = 0x7000_0000_0000;
        const STACK_SIZE: usize = 4096 * 2;
        const STACK_BOTTOM: usize = STACK_TOP - STACK_SIZE;
        loader
            .load_zeroed(STACK_BOTTOM..STACK_TOP, MemFlags::READ | MemFlags::WRITE)
            .expect("Couldn't set up process stack");
        log::info!("Set up the process stack");

        let cap_table_slot = cap_manager.allocate_capability().unwrap();
        log::info!("{cap_table_slot:?}");
        let cap_table = cap_table_slot
            .make_cap_table(CapTableConsArgs::new(frames.next()))
            .expect("Couldn't set up capability table for process");
        log::info!("Setting up resources");

        // Copy the sync return capability
        log::info!("process table slot: {:?}", cap_table_slot);
        cap_manager
            .root()
            .copy_resource(SlotId::new(0).unwrap(), cap_table, SlotId::new(0).unwrap())
            .unwrap();
        log::info!("Copied sync return");
        let thread = cap_manager
            .allocate_capability()
            .unwrap()
            .make_thread(ThreadConsArgs::new(
                unsafe { core::mem::transmute(process.entry() as *const ()) },
                STACK_TOP as *mut u8,
                cap_table,
                addrspace.into_inner(),
                frames.next(),
                0,
            ))
            .expect("Couldn't set up the process's thread");
        log::info!("Allocated thread");
        // Copy the capability table
        cap_table_slot
            .copy_into(cap_table, SlotId::new(1).unwrap())
            .unwrap();
        log::info!("Copied cap table");
        l4_table_slot
            .copy_into(cap_table, SlotId::new(2).unwrap())
            .unwrap();
        // Retype capability
        cap_manager
            .root()
            .copy_resource(SlotId::new(1).unwrap(), cap_table, SlotId::new(3).unwrap())
            .unwrap();
        cap_manager
            .root()
            .copy_resource(SlotId::new(5).unwrap(), cap_table, SlotId::new(4).unwrap())
            .unwrap();
        log::info!("All systems ready");
        thread
    };
    log::info!("Switching to memoery manager!");
    unsafe {
        memory_manager.activate().unwrap();
    }

    log::info!("Initializing user space");
    loop {}
}

pub struct UserspaceLoader<'a, 'b, 'c> {
    self_addrspace: &'a mut Addrspace,
    other_addrspace: &'a mut Addrspace,
    frame_allocator: &'c FrameBumper,
    cap_management: &'b mut SelfCapabilityManager<&'c FrameBumper>,
    hardware: &'a HardwareAccess,
    retype_cap: &'a Retype,
}

impl Loader for UserspaceLoader<'_, '_, '_> {
    type Error = Infallible;

    fn load_with<F>(
        &mut self,
        at: Range<usize>,
        source: F,
        rwx: MemFlags,
    ) -> Result<(), Self::Error>
    where
        F: Fn(usize) -> MaybeUninit<u8>,
    {
        log::info!("Mapping region {at:#X?} with flags {rwx:?}");
        let mut offset = 0;
        let start_page = at.start / Page::size();
        let end_page = at.end.div_ceil(Page::size());
        for page in start_page..end_page {
            let page = Page::from_index(page).unwrap();
            const DEST: Page = Page::from_start_address(VirtAddr::new(0x0000_4000_0000_1000));
            self.request_page(DEST, page, rwx)?;
            let dest =
                unsafe { core::slice::from_raw_parts_mut(DEST.base().as_mut_ptr(), Page::size()) };

            let dest_range = ((at.start + offset) % Page::size())..Page::size();
            let source_range = offset..at.len();

            for (source_off, dest_off) in source_range.zip(dest_range) {
                dest[dest_off] = source(source_off);
                offset += 1;
            }
        }
        Ok(())
    }

    unsafe fn unload(&mut self, vrange: Range<usize>) {
        unimplemented!()
    }
}

impl UserspaceLoader<'_, '_, '_> {
    fn request_page(&mut self, at: Page, page: Page, rwx: MemFlags) -> Result<(), Infallible> {
        let mut pflags = PermissionMask::empty();
        if !rwx.readable() {
            panic!("Can't load non-readable segment");
        }
        if rwx.writeable() {
            pflags |= PermissionMask::WRITE;
        }
        if rwx.executable() {
            pflags |= PermissionMask::EXECUTE;
        }
        let frame = self.frame_allocator.next();
        self.retype_cap
            .retype(frame, RetypeKind::Retype2User)
            .unwrap();
        log::trace!("Mapping {page:?} to {frame:?} with {pflags:?}");
        let mut page_table_alloc = PageTableAlloc {
            frame_alloc: self.frame_allocator,
            retype: &self.retype_cap,
            capability_manager: self.cap_management,
        };
        unsafe {
            self.other_addrspace
                .map_to(
                    page,
                    frame,
                    pflags,
                    // Parent flags are the least restrictive since they will be reused for many pages.
                    PermissionMask::all(),
                    &mut page_table_alloc,
                )
                .unwrap();
            let _ = self.self_addrspace.unmap(at);
            self.self_addrspace
                .map_to(
                    at,
                    frame,
                    PermissionMask::WRITE,
                    // Parent flags are the least restrictive since they will be reused for many pages.
                    PermissionMask::all(),
                    &mut page_table_alloc,
                )
                .unwrap();
            self.hardware.flush_page(at.base().as_usize()).unwrap();
        }
        Ok(())
    }
}

pub struct PageTableAlloc<'a, 'b, 'r> {
    frame_alloc: &'a FrameBumper,
    retype: &'r Retype,
    capability_manager: &'b mut SelfCapabilityManager<&'a FrameBumper>,
}

impl PageTableAllocator for PageTableAlloc<'_, '_, '_> {
    fn allocate_table(&mut self, level: PageTableLevel) -> PageTable {
        let frame = self.frame_alloc.next();
        let page_table = self
            .capability_manager
            .allocate_capability()
            .unwrap()
            .make_page_table(PageTableConsArgs::new(frame, level.level()))
            .unwrap();
        page_table
    }
}
