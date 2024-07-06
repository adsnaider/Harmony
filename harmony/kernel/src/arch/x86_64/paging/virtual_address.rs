use super::page_table::{PageTableLevel, PageTableOffset};
use super::PhysAddr;

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct VirtAddr(usize);

impl core::fmt::Debug for VirtAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "VirtAddr({:#X?})", self.0)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct BadVirtAddr;

impl VirtAddr {
    pub const fn new(addr: usize) -> Self {
        match Self::try_new(addr) {
            Ok(addr) => addr,
            Err(_) => panic!("Invalid address: Must be 48-bit sign-extended"),
        }
    }

    pub fn from_ptr<T>(ptr: *const T) -> Self {
        Self::new(ptr as usize)
    }

    pub const fn try_new(addr: usize) -> Result<Self, BadVirtAddr> {
        if Self::new_truncate(addr).0 == addr {
            Ok(Self::new_truncate(addr))
        } else {
            Err(BadVirtAddr)
        }
    }

    pub const fn new_truncate(addr: usize) -> Self {
        Self(((addr << 16) as i64 >> 16) as usize)
    }

    pub const fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }

    pub const fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }

    pub const fn as_usize(&self) -> usize {
        self.0
    }

    pub const fn zero() -> Self {
        Self::new(0)
    }

    /// Returns the 9-bit level 1 page table index.
    #[inline]
    pub const fn p1_index(self) -> PageTableOffset {
        PageTableOffset::new_truncate((self.0 >> 12) as u16)
    }

    /// Returns the 9-bit level 2 page table index.
    #[inline]
    pub const fn p2_index(self) -> PageTableOffset {
        PageTableOffset::new_truncate((self.0 >> 12 >> 9) as u16)
    }

    /// Returns the 9-bit level 3 page table index.
    #[inline]
    pub const fn p3_index(self) -> PageTableOffset {
        PageTableOffset::new_truncate((self.0 >> 12 >> 9 >> 9) as u16)
    }

    /// Returns the 9-bit level 4 page table index.
    #[inline]
    pub const fn p4_index(self) -> PageTableOffset {
        PageTableOffset::new_truncate((self.0 >> 12 >> 9 >> 9 >> 9) as u16)
    }

    /// Returns the 9-bit level page table index.
    #[inline]
    pub const fn page_table_index(self, level: PageTableLevel) -> PageTableOffset {
        PageTableOffset::new_truncate((self.0 >> 12 >> ((level.level() - 1) * 9)) as u16)
    }

    /// Converts a virtual address to physical assuming the physical memory offset.
    ///
    /// # Safety
    ///
    /// The virtual address must have been created with `PhysAddr::to_virtual`
    pub unsafe fn to_physical(&self) -> PhysAddr {
        PhysAddr::from_virtual(*self)
    }
}
