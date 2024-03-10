use crate::PMO;

pub const PAGE_SIZE: usize = 4096;

/// A physical frame that should only be used at boot time.
pub struct RawFrame {
    phys_address: u64,
}

impl RawFrame {
    pub fn from_start_address(address: u64) -> Self {
        Self {
            phys_address: address,
        }
    }

    /// This assumes identity mapping.
    pub fn as_ptr<T>(&self) -> *const T {
        (self.phys_address + *PMO as u64) as *const T
    }

    pub fn as_ptr_mut<T>(&self) -> *mut T {
        (self.phys_address + *PMO as u64) as *mut T
    }
}
/*

/// A virtual memory space suitable for user-level components.
#[repr(transparent)]
pub struct AddrSpace {
    l4_table: PageTable,
}

impl AddrSpace {
    /// Create a new address space with no user level mappings.
    ///
    /// This function will use the active address space and copy the kernel level
    /// (higher half) mappings.
    ///
    /// # Safety
    ///
    /// Provided frame must be available for use (i.e. unused).
    pub unsafe fn new(l4_frame: RawFrame) -> Self {
        let current = unsafe { Self::current() };
        let current_l4_table: &PageTable = current.page_table.level_4_table();

        let l4_table: &mut PageTable = unsafe {
            core::ptr::write(l4_frame.as_ptr_mut(), PageTable::new());
            &mut *l4_frame.as_ptr_mut()
        };
        for i in 256..512 {
            l4_table[i] = current_l4_table[i].clone();
        }

        Self::from_l4_frame(l4_frame)
    }

    unsafe fn current() -> Self {
        let (frame, _flags) = Cr3::read();
        let raw_frame = RawFrame::from_start_address(frame.start_address().as_u64());
        Self::from_l4_frame(raw_frame)
    }

    unsafe fn from_l4_frame(l4_frame: RawFrame) -> Self {
        Self {
            page_table: OffsetPageTable::new(
                unsafe { &mut *l4_frame.as_ptr_mut() },
                VirtAddr::new(*PMO as u64),
            ),
        }
    }
}
*/
