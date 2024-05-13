use limine::memory_map::{Entry, EntryType};
use sync::cell::AtomicLazyCell;

use crate::arch::paging::{RawFrame, FRAME_SIZE};
use crate::MemoryMap;
pub static BOOT_ALLOCATION: AtomicLazyCell<EntryType> = AtomicLazyCell::new(|| EntryType::from(8));
pub struct BumpAllocator {
    memory_map: &'static mut [&'static mut Entry],
    index: usize,
}

impl BumpAllocator {
    pub fn new(memory_map: MemoryMap) -> Self {
        Self {
            memory_map,
            index: 0,
        }
    }

    pub fn alloc_frame(&mut self) -> Option<RawFrame> {
        let frame = loop {
            let entry = self.memory_map.get_mut(self.index)?;
            assert!(entry.length % FRAME_SIZE == 0);
            if entry.entry_type == EntryType::USABLE && entry.length > 0 {
                let start_address = entry.base;
                entry.base += FRAME_SIZE;
                entry.length -= FRAME_SIZE;
                break RawFrame::from_start_address(start_address);
            }
            self.index += 1;
        };
        Some(frame)
    }

    pub fn alloc_frames(&mut self, count: usize) -> Option<u64> {
        let requested_length = count as u64 * FRAME_SIZE;
        let start_address = loop {
            let entry = self.memory_map.get_mut(self.index)?;
            assert!(entry.length % FRAME_SIZE == 0);
            if entry.entry_type == EntryType::USABLE && entry.length >= requested_length {
                let start_address = entry.base;
                entry.base += requested_length;
                entry.length -= requested_length;
                break start_address;
            }
            self.index += 1;
        };
        Some(start_address)
    }

    pub fn into_memory_map(self) -> &'static mut [&'static mut Entry] {
        self.memory_map
    }

    pub fn memory_map(&mut self) -> &mut [&'static mut Entry] {
        self.memory_map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn allocation_test() {
        static mut TEST_MAP: [&mut Entry; 4] = [
            &mut Entry {
                base: 0,
                length: FRAME_SIZE * 2,
                entry_type: EntryType::USABLE,
            },
            &mut Entry {
                base: FRAME_SIZE * 4,
                length: FRAME_SIZE,
                entry_type: EntryType::USABLE,
            },
            &mut Entry {
                base: FRAME_SIZE * 5,
                length: FRAME_SIZE,
                entry_type: EntryType::RESERVED,
            },
            &mut Entry {
                base: FRAME_SIZE * 6,
                length: FRAME_SIZE,
                entry_type: EntryType::USABLE,
            },
        ];

        // SAFETY: Mutable access is unique.
        #[allow(static_mut_refs)]
        let mut allocator = BumpAllocator::new(unsafe { &mut TEST_MAP });
        for expected_frame in [0, FRAME_SIZE, FRAME_SIZE * 4, FRAME_SIZE * 6] {
            let expected_frame = RawFrame::from_start_address(expected_frame);
            let frame = allocator.alloc_frame().unwrap();
            assert_eq!(frame, expected_frame);
        }

        assert!(allocator.alloc_frame().is_none())
    }

    #[test_case]
    fn multi_allocation_test() {
        static mut TEST_MAP: [&mut Entry; 4] = [
            &mut Entry {
                base: 0,
                length: FRAME_SIZE * 2,
                entry_type: EntryType::USABLE,
            },
            &mut Entry {
                base: FRAME_SIZE * 4,
                length: FRAME_SIZE,
                entry_type: EntryType::USABLE,
            },
            &mut Entry {
                base: FRAME_SIZE * 5,
                length: FRAME_SIZE,
                entry_type: EntryType::RESERVED,
            },
            &mut Entry {
                base: FRAME_SIZE * 6,
                length: FRAME_SIZE * 2,
                entry_type: EntryType::USABLE,
            },
        ];

        // SAFETY: Mutable access is unique.
        #[allow(static_mut_refs)]
        let mut allocator = BumpAllocator::new(unsafe { &mut TEST_MAP });
        for expected_frame in [0, FRAME_SIZE * 6] {
            let frame = allocator.alloc_frames(2).unwrap();
            assert_eq!(frame, expected_frame);
        }

        assert!(allocator.alloc_frame().is_none())
    }
}
