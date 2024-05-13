use limine::memory_map::{Entry, EntryType};
use sync::cell::AtomicLazyCell;

use crate::arch::paging::RawFrame;
pub static BOOT_ALLOCATION: AtomicLazyCell<EntryType> = AtomicLazyCell::new(|| EntryType::from(8));
pub struct BumpAllocator {
    memory_map: &'static mut [&'static mut Entry],
    index: usize,
}

impl BumpAllocator {
    pub fn new(memory_map: &'static mut [&'static mut Entry]) -> Self {
        Self {
            memory_map,
            index: 0,
        }
    }

    pub fn alloc_frame(&mut self) -> Option<RawFrame> {
        const PAGE_SIZE: u64 = crate::arch::paging::PAGE_SIZE as u64;
        let frame = loop {
            let entry = self.memory_map.get_mut(self.index)?;
            assert!(entry.length % PAGE_SIZE as u64 == 0);
            if entry.entry_type == EntryType::USABLE && entry.length > 0 {
                let start_address = entry.base;
                entry.base += PAGE_SIZE;
                entry.length -= PAGE_SIZE;
                break RawFrame::from_start_address(start_address);
            }
            self.index += 1;
        };
        Some(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn allocation_test() {
        const PAGE_SIZE: u64 = crate::arch::paging::PAGE_SIZE as u64;
        static mut TEST_MAP: [&mut Entry; 4] = [
            &mut Entry {
                base: 0,
                length: PAGE_SIZE * 2,
                entry_type: EntryType::USABLE,
            },
            &mut Entry {
                base: PAGE_SIZE * 4,
                length: PAGE_SIZE,
                entry_type: EntryType::USABLE,
            },
            &mut Entry {
                base: PAGE_SIZE * 5,
                length: PAGE_SIZE,
                entry_type: EntryType::RESERVED,
            },
            &mut Entry {
                base: PAGE_SIZE * 6,
                length: PAGE_SIZE,
                entry_type: EntryType::USABLE,
            },
        ];

        // SAFETY: Mutable access is unique.
        #[allow(static_mut_refs)]
        let mut allocator = BumpAllocator::new(unsafe { &mut TEST_MAP });
        for expected_frame in [0, PAGE_SIZE, PAGE_SIZE * 4, PAGE_SIZE * 6] {
            let expected_frame = RawFrame::from_start_address(expected_frame);
            let frame = allocator.alloc_frame().unwrap();
            assert_eq!(frame, expected_frame);
        }

        assert!(allocator.alloc_frame().is_none())
    }
}
