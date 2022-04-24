//! A frame allocator that manages frame with a bitmap.

mod bitmap;

use core::marker::PhantomData;

use bootinfo::{MemoryMap, MemoryType};

use self::bitmap::Bitmap;
use super::{
    ExactFrameAllocator, FrameAllocError, FrameAllocator, FrameDeallocError, RequestFrameError,
};
use crate::structures::{Frame, PageSize};

/// A frame allocator that uses a bitmap to keep track of available frames.
#[derive(Debug)]
pub struct BitmapFrameAllocator<'a, S: PageSize> {
    /// Represents availability of frames. The representation is a one to one mapping where index
    /// `k` represents the frame with start address of `k * S::SIZE`.
    available_frames: Bitmap<'a, u64>,
    /// Phantom...
    _phantom: PhantomData<S>,
}

/// Returns true if the memory region is generally usable.
fn is_region_usable(region: &bootinfo::MemoryRegion) -> bool {
    matches!(
        region.ty,
        MemoryType::Conventional | MemoryType::UefiAvailable
    )
}

/// Error returned when the construction of the BitmapFrameAllocator fails.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum NewFrameAllocError {
    /// Memory map is malformed.
    MalformedMemoryMap,
    /// Storage provided isn't enough to initialize the memory allocator.
    NotEnoughStorage,
}

impl<'a, S: PageSize> BitmapFrameAllocator<'a, S> {
    /// Builds the frame allocator and returns it alongside with leftover (unneeded storage).
    ///
    /// The `storage` must have at least the total number of frames in the system / 64 elements.
    ///
    /// # Safety
    ///
    /// The `memory_map` should be correct.
    pub unsafe fn new(
        storage: &'a mut [u64],
        memory_map: &MemoryMap<'_>,
    ) -> Result<(Self, &'a mut [u64]), NewFrameAllocError> {
        let mut latest_page = 0;
        let mut bitmap = Bitmap::zeros(storage);
        for region in memory_map
            .regions
            .iter()
            .filter(|region| is_region_usable(region))
        {
            if region.phys_start % S::SIZE != 0 {
                return Err(NewFrameAllocError::MalformedMemoryMap);
            }
            let first_page = region.phys_start / S::SIZE;
            let last_page = region.phys_start / S::SIZE + region.page_count - 1;
            latest_page = latest_page.max(last_page);

            for i in 0..region.page_count {
                if (first_page + i) >= bitmap.len() {
                    return Err(NewFrameAllocError::NotEnoughStorage);
                }
                bitmap.set(first_page + i);
            }
        }

        let (bitmap, leftover) = bitmap.truncate(latest_page + 1);

        Ok((
            Self {
                available_frames: bitmap,
                _phantom: PhantomData,
            },
            leftover,
        ))
    }
}

unsafe impl<S: PageSize> FrameAllocator<S> for BitmapFrameAllocator<'_, S> {
    fn allocate_frame(&mut self) -> Result<Frame<S>, FrameAllocError> {
        let available = self
            .available_frames
            .find_first_set()
            .ok_or(FrameAllocError::OutOfFrames)?;
        self.available_frames.unset(available);
        Ok(Frame::new(available * S::SIZE))
    }

    unsafe fn deallocate_frame(&mut self, frame: Frame<S>) -> Result<(), FrameDeallocError> {
        let page = frame.phys_start() / S::SIZE;
        if self.available_frames.get(page) {
            return Err(FrameDeallocError::FrameNotAllocated);
        }

        self.available_frames.set(page);
        Ok(())
    }
}

unsafe impl<S: PageSize> ExactFrameAllocator<S> for BitmapFrameAllocator<'_, S> {
    fn request_frame(&mut self, frame: Frame<S>) -> Result<(), RequestFrameError> {
        let findex = frame.phys_start() / S::SIZE;
        if self.available_frames.get(findex) {
            self.available_frames.unset(findex);
            Ok(())
        } else {
            Err(RequestFrameError::FrameInUse)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::vec;

    use bootinfo::MemoryRegion;

    use super::*;
    use crate::structures::Size4KiB;

    macro_rules! region {
        (A($start:expr, $count:expr)) => {{
            use bootinfo::{MemoryAttribute, MemoryRegion, MemoryType};
            MemoryRegion {
                ty: MemoryType::Conventional,
                phys_start: $start * 4096,
                page_count: $count,
                attribute: MemoryAttribute::Unknown,
            }
        }};
        (U($start:expr,$count:expr)) => {{
            use bootinfo::{MemoryAttribute, MemoryRegion, MemoryType};
            MemoryRegion {
                ty: MemoryType::AcpiUnavailable,
                phys_start: $start * 4096,
                page_count: $count,
                attribute: MemoryAttribute::Unknown,
            }
        }};
    }

    macro_rules! map {
        [$($type:tt ($start:expr,$count:expr)),*] => {{
            use bootinfo::MemoryMap;
            MemoryMap {
                regions: &mut [$(region!($type($start,$count))),*]
            }
        }};
    }

    #[test]
    fn simple() {
        let mmap = map![A(1, 10)];
        let mut storage = vec![0; 4096];

        let mut fallocator: BitmapFrameAllocator<Size4KiB> =
            unsafe { BitmapFrameAllocator::new(&mut storage, &mmap) }
                .unwrap()
                .0;

        let frame = fallocator.allocate_frame().expect("We have 10 frames.");
        assert!(is_within_map(&frame, &mmap));
    }

    #[test]
    fn all() {
        let mmap = map![A(1, 10)];
        let mut storage = vec![0; 4096];

        let mut fallocator: BitmapFrameAllocator<Size4KiB> =
            unsafe { BitmapFrameAllocator::new(&mut storage, &mmap) }
                .unwrap()
                .0;

        for i in 0..10 {
            let frame = fallocator.allocate_frame().expect(&format!(
                "{}: we should have {}/10 frames available.",
                i,
                10 - i
            ));
            assert!(is_within_map(&frame, &mmap));
        }
    }

    #[test]
    fn all_thrice() {
        let mmap = map![A(1, 10)];
        let mut storage = vec![0; 4096];

        let mut fallocator: BitmapFrameAllocator<Size4KiB> =
            unsafe { BitmapFrameAllocator::new(&mut storage, &mmap) }
                .unwrap()
                .0;

        for _ in 0..3 {
            let mut frames = Vec::new();
            for i in 0..10 {
                let frame = fallocator.allocate_frame().expect(&format!(
                    "{}: we should have {}/10 frames available.",
                    i,
                    10 - i
                ));
                assert!(is_within_map(&frame, &mmap));
                frames.push(frame)
            }
            for frame in frames {
                unsafe {
                    fallocator
                        .deallocate_frame(frame)
                        .expect("Coudln't deallocate frame");
                }
            }
        }
    }

    #[test]
    fn all_thrice_no_dealloc() {
        let mmap = map![A(1, 10)];
        let mut storage = vec![0; 4096];

        let mut fallocator: BitmapFrameAllocator<Size4KiB> =
            unsafe { BitmapFrameAllocator::new(&mut storage, &mmap) }
                .unwrap()
                .0;

        for iter in 0..3 {
            for i in 0..10 {
                if iter == 0 {
                    let frame = fallocator.allocate_frame().expect(&format!(
                        "{}: we should have {}/10 frames available.",
                        i,
                        10 - i
                    ));
                    assert!(is_within_map(&frame, &mmap));
                } else {
                    fallocator
                        .allocate_frame()
                        .expect_err("All frames should have been allocated.");
                }
            }
        }
    }

    #[test]
    fn no_dups() {
        let mmap = map![A(1, 10)];
        let mut storage = vec![0; 4096];

        let mut fallocator: BitmapFrameAllocator<Size4KiB> =
            unsafe { BitmapFrameAllocator::new(&mut storage, &mmap) }
                .unwrap()
                .0;

        let mut frames = HashSet::new();
        for i in 0..10 {
            let frame = fallocator.allocate_frame().expect(&format!(
                "{}: we should have {}/10 frames available.",
                i,
                10 - i
            ));
            assert!(is_within_map(&frame, &mmap));
            assert!(frames.insert(frame), "Duplicate frames!");
        }
    }

    fn is_within_region<S: PageSize>(frame: &Frame<S>, region: &MemoryRegion) -> bool {
        (region.phys_start..(region.phys_start + region.page_count * 4096))
            .contains(&frame.phys_start())
    }

    fn is_within_map<S: PageSize>(frame: &Frame<S>, map: &MemoryMap) -> bool {
        map.regions
            .iter()
            .filter(|region| {
                matches!(
                    region.ty,
                    MemoryType::Conventional | MemoryType::UefiAvailable
                )
            })
            .any(|region| is_within_region(frame, region))
    }

    #[test]
    fn spread_out() {
        let mmap = map![A(1, 10), A(123, 7), A(150, 2)];
        let mut storage = vec![0; 4096];
        let mut frames = HashSet::new();

        let mut fallocator: BitmapFrameAllocator<Size4KiB> =
            unsafe { BitmapFrameAllocator::new(&mut storage, &mmap) }
                .unwrap()
                .0;

        for i in 0..18 {
            let frame = fallocator.allocate_frame().expect(&format!(
                "{}: we should have {}/19 frames available.",
                i,
                19 - i
            ));
            assert!(is_within_map(&frame, &mmap));
            assert!(frames.insert(frame), "Duplicate frames!");
        }
    }

    #[test]
    fn spread_out_unavailable() {
        let mmap = map![A(1, 10), U(23, 10), A(123, 7), A(150, 2)];
        let mut storage = vec![0; 4096];
        let mut frames = HashSet::new();

        let mut fallocator: BitmapFrameAllocator<Size4KiB> =
            unsafe { BitmapFrameAllocator::new(&mut storage, &mmap) }
                .unwrap()
                .0;

        for i in 0..18 {
            let frame = fallocator.allocate_frame().expect(&format!(
                "{}: we should have {}/10 frames available.",
                i,
                18 - i
            ));
            assert!(is_within_map(&frame, &mmap));
            assert!(frames.insert(frame), "Duplicate frames!");
        }
    }

    fn available_pages(mmap: &MemoryMap) -> usize {
        mmap.regions
            .iter()
            .filter(|region| {
                matches!(
                    region.ty,
                    MemoryType::Conventional | MemoryType::UefiAvailable
                )
            })
            .fold(0, |pages, region| pages + region.page_count)
    }

    #[test]
    fn stress() {
        let mmap = map![
            A(1, 10),
            U(23, 10),
            A(123, 7),
            A(150, 2),
            U(153, 5),
            A(180, 300),
            A(1000, 1500),
            // These should not affect the storage requirement.
            U(30000000, 100),
            U(37000000, 100)
        ];
        let mut storage = vec![0; 4096]; // 4096 x 64 = 26,144 (max page that can be handled).
        let mut frames = HashSet::new();

        let mut fallocator: BitmapFrameAllocator<Size4KiB> =
            unsafe { BitmapFrameAllocator::new(&mut storage, &mmap) }
                .unwrap()
                .0;

        for _ in 0..available_pages(&mmap) {
            let frame = fallocator
                .allocate_frame()
                .expect("We should have enough pages.");
            assert!(is_within_map(&frame, &mmap));
            assert!(frames.insert(frame), "Duplicate frames!");
        }

        for _ in 0..20 {
            for _ in 0..10 {
                let frame = frames.iter().copied().next().unwrap();
                unsafe {
                    fallocator
                        .deallocate_frame(frame)
                        .expect("Should be able to deallocated previously allocated frame.");
                }
                frames.remove(&frame);
            }
            for _ in 0..9 {
                let frame = fallocator
                    .allocate_frame()
                    .expect("We should have enough pages.");
                assert!(is_within_map(&frame, &mmap));
                assert!(frames.insert(frame), "Duplicate frames!");
            }
        }
    }
}
