#[repr(transparent)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct VirtAddr(usize);

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
}
