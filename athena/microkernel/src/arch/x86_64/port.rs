use core::arch::asm;

/// Data bus port
pub struct Port {
    port: u16,
}

impl Port {
    /// Creates a read/write port
    pub const fn new(port: u16) -> Self {
        Self { port }
    }

    /// Reads a value from the port
    ///
    /// # Safety
    ///
    /// I/O could have side effects that violate memory safety
    pub unsafe fn read<T: PortRead>(&mut self) -> T {
        // SAFETY: Precondition
        unsafe { T::read_from_port(self.port) }
    }

    /// Writes a value to the port
    ///
    /// # Safety
    ///
    /// I/O could have side effects that violate memory safety
    pub unsafe fn write<T: PortWrite>(&mut self, data: T) {
        // SAFETY: Precondition
        unsafe { data.write_to_port(self.port) }
    }
}

pub trait PortRead {
    /// Reads the value from the port
    ///
    /// # Safety
    ///
    /// I/O could have side effects that violate memory safety
    unsafe fn read_from_port(port: u16) -> Self;
}

pub trait PortWrite {
    /// Writes the value to the port
    ///
    /// # Safety
    ///
    /// I/O could have side effects that violate memory safety
    unsafe fn write_to_port(self, port: u16);
}

impl PortWrite for u8 {
    #[inline]
    unsafe fn write_to_port(self, port: u16) {
        // SAFETY: Precondition
        unsafe {
            asm!("out dx, al", in("dx") port, in("al") self, options(nomem, nostack, preserves_flags));
        }
    }
}

impl PortWrite for u16 {
    #[inline]
    unsafe fn write_to_port(self, port: u16) {
        // SAFETY: Precondition
        unsafe {
            asm!("out dx, ax", in("dx") port, in("ax") self, options(nomem, nostack, preserves_flags));
        }
    }
}

impl PortWrite for u32 {
    #[inline]
    unsafe fn write_to_port(self, port: u16) {
        // SAFETY: Precondition
        unsafe {
            asm!("out dx, eax", in("dx") port, in("eax") self, options(nomem, nostack, preserves_flags));
        }
    }
}
impl PortRead for u8 {
    #[inline]
    unsafe fn read_from_port(port: u16) -> u8 {
        // SAFETY: Precondition
        let value: u8;
        // SAFETY: Precondition
        unsafe {
            asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags));
        }
        value
    }
}

impl PortRead for u16 {
    #[inline]
    unsafe fn read_from_port(port: u16) -> u16 {
        let value: u16;
        // SAFETY: Precondition
        unsafe {
            asm!("in ax, dx", out("ax") value, in("dx") port, options(nomem, nostack, preserves_flags));
        }
        value
    }
}

impl PortRead for u32 {
    #[inline]
    unsafe fn read_from_port(port: u16) -> u32 {
        let value: u32;
        // SAFETY: Precondition
        unsafe {
            asm!("in eax, dx", out("eax") value, in("dx") port, options(nomem, nostack, preserves_flags));
        }
        value
    }
}
