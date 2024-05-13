use core::mem::MaybeUninit;

use limine::memory_map::EntryType;
use sync::cell::AtomicOnceCell;

use crate::arch::paging::{FRAME_SIZE, PAGE_SIZE};
use crate::bump_allocator::BumpAllocator;
use crate::MemoryMap;

static RETYPE_TABLE: AtomicOnceCell<RetypeTable> = AtomicOnceCell::new();

pub struct RetypeTable {
    retype_map: &'static mut [RetypeEntry],
}

impl RetypeTable {
    pub fn new(memory_map: MemoryMap) -> Option<Self> {
        let physical_top = {
            let last = memory_map.iter().last()?;
            last.base + last.length
        };
        assert!(physical_top % FRAME_SIZE == 0);
        let number_frames = (physical_top / FRAME_SIZE) as usize;
        let mut allocator = BumpAllocator::new(memory_map);

        let retype_map_frames = {
            let retype_map_size = core::mem::size_of::<RetypeEntry>() * number_frames;
            assert!(retype_map_size > 0);
            (retype_map_size - 1) / PAGE_SIZE + 1
        };
        let retype_map = {
            let start_physical_address = allocator.alloc_frames(retype_map_frames)?;
            let start_addr: *mut MaybeUninit<RetypeEntry> =
                start_physical_address.to_virtual().as_mut_ptr();
            // SAFETY: Memory is allocated and off the memory map
            unsafe { core::slice::from_raw_parts_mut(start_addr, number_frames) }
        };

        for entry in retype_map.iter_mut() {
            entry.write(RetypeEntry::Reserved);
        }
        // SAEFETY: Initialized in earlier loop
        let retype_map: &mut [RetypeEntry] = unsafe { core::mem::transmute(retype_map) };

        let memory_map = allocator.into_memory_map();
        for entry in memory_map
            .iter()
            .filter(|entry| entry.entry_type == EntryType::USABLE)
        {
            assert!(entry.base % FRAME_SIZE == 0);
            assert!(entry.length % FRAME_SIZE == 0);
            let start_idx = (entry.base / FRAME_SIZE) as usize;
            let count = (entry.length / FRAME_SIZE) as usize;
            for i in start_idx..(start_idx + count) {
                retype_map[i] = RetypeEntry::Untyped;
            }
        }
        Some(Self { retype_map })
    }

    pub fn set_as_global(self) -> Result<(), sync::cell::OnceError> {
        RETYPE_TABLE.set(self)
    }
}

#[repr(u8)]
#[derive(Default, Debug)]
enum RetypeEntry {
    #[default]
    Reserved = 0,
    Untyped = 1,
}
