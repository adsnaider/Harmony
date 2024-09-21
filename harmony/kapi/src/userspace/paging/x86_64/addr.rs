pub const PAGE_SIZE: usize = 4096;
pub const FRAME_SIZE: u64 = 4096;

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

    pub const fn is_higher_half(&self) -> bool {
        self.0 >= 0xFFFF_8000_0000_0000
    }

    pub const fn is_lower_half(&self) -> bool {
        !self.is_higher_half()
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
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PageTableOffset(u16);

#[derive(Debug, Copy, Clone)]
pub struct PageTableLevel(u8);

#[derive(Debug)]
pub struct InvalidLevel;

impl PageTableLevel {
    pub const fn new(level: u8) -> Self {
        match Self::try_new(level) {
            Ok(level) => level,
            Err(_) => panic!("Page table level must be within 1 and 4",),
        }
    }

    pub const fn try_new(level: u8) -> Result<Self, InvalidLevel> {
        if level < 1 || level > 4 {
            return Err(InvalidLevel);
        }
        Ok(Self(level))
    }

    pub const fn level(&self) -> u8 {
        self.0
    }

    pub const fn top() -> Self {
        Self(4)
    }

    pub const fn is_bottom(&self) -> bool {
        self.level() == 1
    }

    pub const fn lower(self) -> Option<Self> {
        match Self::try_new(self.level() - 1) {
            Ok(l) => Some(l),
            Err(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum PageTableOffsetError {
    OutOfBounds,
}

impl TryFrom<u16> for PageTableOffset {
    type Error = PageTableOffsetError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<usize> for PageTableOffset {
    type Error = PageTableOffsetError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::new(u16::try_from(value).map_err(|_| PageTableOffsetError::OutOfBounds)?)
    }
}

impl PageTableOffset {
    pub const fn new(offset: u16) -> Result<Self, PageTableOffsetError> {
        if offset < 512 {
            Ok(Self(offset))
        } else {
            Err(PageTableOffsetError::OutOfBounds)
        }
    }

    pub const fn is_lower_half(&self) -> bool {
        self.0 < 256
    }

    pub const fn new_truncate(addr: u16) -> Self {
        Self(addr % 512)
    }
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct Page {
    start_address: VirtAddr,
}

#[derive(Debug)]
pub struct Unaligned;

impl Page {
    pub const fn from_start_address(addr: VirtAddr) -> Self {
        match Self::try_from_start_address(addr) {
            Err(Unaligned) => panic!("Unaligned start address"),
            Ok(this) => this,
        }
    }

    pub const fn size() -> usize {
        PAGE_SIZE
    }

    pub const fn try_from_start_address(addr: VirtAddr) -> Result<Self, Unaligned> {
        if addr.as_usize() % PAGE_SIZE != 0 {
            return Err(Unaligned);
        }
        Ok(Self {
            start_address: addr,
        })
    }

    pub fn containing_address(addr: VirtAddr) -> Self {
        let addr = VirtAddr::new((addr.as_usize() / PAGE_SIZE) * PAGE_SIZE);
        Self::from_start_address(addr)
    }

    pub fn base(&self) -> VirtAddr {
        self.start_address
    }

    /// Returns the page defined by `base = index * PAGE_SIZE`
    pub const fn from_index(index: usize) -> Result<Self, BadVirtAddr> {
        match VirtAddr::try_new(index * PAGE_SIZE) {
            Ok(addr) => Ok(Self::from_start_address(addr)),
            Err(e) => Err(e),
        }
    }

    /// Returns the index of this page (inverse of `from_index`)
    pub const fn index(&self) -> usize {
        self.start_address.as_usize() / PAGE_SIZE
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct Frame {
    base: PhysAddr,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct UnalignedAddress;

impl Frame {
    pub fn from_start_address(base: PhysAddr) -> Self {
        Self::try_from_start_address(base).unwrap()
    }

    pub fn base(&self) -> PhysAddr {
        self.base
    }

    pub fn from_index(index: u64) -> Result<Self, BadAddress> {
        let start = index * FRAME_SIZE;
        Ok(Self::from_start_address(PhysAddr::try_new(start)?))
    }

    pub fn try_from_start_address(base: PhysAddr) -> Result<Self, UnalignedAddress> {
        if base.as_u64() % FRAME_SIZE != 0 {
            return Err(UnalignedAddress);
        }
        Ok(Self { base })
    }

    pub fn within_frame(addr: PhysAddr) -> Self {
        let base = PhysAddr::new(addr.as_u64() % FRAME_SIZE);
        Self { base }
    }

    pub const fn addr(&self) -> PhysAddr {
        self.base
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct PhysAddr(u64);

impl From<PhysAddr> for u64 {
    fn from(value: PhysAddr) -> Self {
        value.0
    }
}

impl From<PhysAddr> for usize {
    fn from(value: PhysAddr) -> Self {
        value.0 as usize
    }
}

impl core::fmt::Debug for PhysAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "PhysAddr({:#X})", self.0)
    }
}

#[derive(Debug)]
pub struct BadAddress;

impl PhysAddr {
    pub const fn new(addr: u64) -> Self {
        match Self::try_new(addr) {
            Ok(addr) => addr,
            Err(_) => panic!("Invalid Physical Address: Must be up to 52 bits"),
        }
    }

    pub const fn try_new(addr: u64) -> Result<Self, BadAddress> {
        if Self::new_truncate(addr).0 == addr {
            Ok(Self(addr))
        } else {
            Err(BadAddress)
        }
    }

    pub const fn new_truncate(addr: u64) -> Self {
        Self(addr % (1 << 52))
    }

    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}
