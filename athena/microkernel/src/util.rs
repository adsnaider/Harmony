use limine::memory_map::{Entry, EntryType};

use crate::arch::paging::{RawFrame, PAGE_SIZE};

pub struct FrameBumpAllocator {
    index: usize,
    memory_map: &'static mut [&'static mut Entry],
}

impl FrameBumpAllocator {
    pub fn new(memory_map: &'static mut [&'static mut Entry]) -> Self {
        Self {
            index: 0,
            memory_map,
        }
    }

    pub fn alloc_frame(&mut self) -> Option<RawFrame> {
        let (idx, entry) = self
            .memory_map
            .iter_mut()
            .enumerate()
            .skip(self.index)
            .filter(|(_idx, entry)| entry.entry_type == EntryType::USABLE)
            .find(|(_idx, entry)| entry.length as usize > PAGE_SIZE)?;

        let addr = entry.base;
        entry.base += PAGE_SIZE as u64;
        entry.length -= PAGE_SIZE as u64;
        self.index = idx;
        Some(RawFrame::from_start_address(addr))
    }

    pub fn consume(self) -> &'static mut [&'static mut Entry] {
        self.memory_map
    }

    pub fn entries<'a>(&'a self) -> &'a [&'static mut Entry] {
        &*self.memory_map
    }
}
