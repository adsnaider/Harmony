//! This crate defines the Bootinfo structure that gets passed into the kenrel by the bootloader.
#![no_std]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![warn(missing_copy_implementations)]

/// Memory type used to load all the statics to the kernel.
pub const KERNEL_STATIC: u32 = 0x80000000;
/// Memory type used to identify the kernel stack.
pub const KERNEL_STACK: u32 = 0x80000001;
/// Memory type used to identify the kernel code.
pub const KERNEL_CODE: u32 = 0x80000002;

#[repr(C)]
#[derive(Debug)]
/// The `Bootinfo` struct gets passed from the bootloader to the kernel.
pub struct Bootinfo {
    /// Framebuffer structure that can be used in the kernel to control the screen.
    pub framebuffer: Framebuffer,
    /// The memory map describes the physical regions in memory.
    pub memory_map: MemoryMap<'static>,
    /// Bitmap-encoded font to use within the kernel.
    pub font: &'static [u8],
}

#[repr(C)]
#[derive(Debug)]
/// System memory map. This is a physical representation of the memory (i.e. identity-mapped).
pub struct MemoryMap<'a> {
    /// Region describing 1 or more set of contiguous pages in memory.
    pub regions: &'a mut [MemoryRegion],
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
/// Represents a contiguous region in memory of the same type.
pub struct MemoryRegion {
    /// Type of this memory region. Used to decide if the memory is usable.
    pub ty: MemoryType,
    /// Address of the first page for this memory.
    pub phys_start: usize,
    /// Number of pages included in the memory region. The address range representing all of these
    /// would be [`phys_start`, `phys_start` + `page_count` * 4096)
    pub page_count: usize,
    /// Physical attributes about the memory.
    pub attribute: MemoryAttribute,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
/// Describes the type of memory in a particular region.
pub enum MemoryType {
    /// Unusable, system-reserved memory.
    Reserved,
    /// Usable memory used in the UEFI environment.
    UefiAvailable,
    /// Unusable memory used in the UEFI environment.
    UefiUnavailable,
    /// Usable, conventional memory.
    Conventional,
    /// Usable, persistent memory.
    Persistent,
    /// ACPI memory that holds ACPI tables. Can be reused once not needed.
    AcpiReclaim,
    /// Firmware reserved addresses.
    AcpiUnavailable,
    /// Memory-mapped IO.
    Mmio,
    /// Memory-mapped port space.
    MmioPort,
    /// Unusable memory region where the kernel is loaded.
    KernelCode,
    /// Unusable memory region where kernel boot data is loaded.
    KernelData,
    /// Unusable memory region used for the kernel's stack.
    KernelStack,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
/// Physical properties of the memory region.
pub enum MemoryAttribute {
    // TODO(#2): Set these up.
    /// Unknow memory attributes.
    Unknown,
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
/// Determines the format (i.e. byte ordering) of each pixel such as RGB, BGR, etc.
pub enum PixelFormat {
    /// Red, green blue,
    Rgb,
    /// Blue, green, red.
    Bgr,
    /// Bitmask. If this, `bitmask` will be set in ramebuffer.
    Bitmask,
    /// Blt.
    BltOnly,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
/// Framebuffer bitmask.
pub struct PixelBitmask {
    /// Red mask.
    pub red: u32,
    /// Green mask.
    pub green: u32,
    /// Blue maks.
    pub blue: u32,
    /// Reserved. Not used by the display hardware.
    pub reserved: u32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
/// Framebuffer structure.
pub struct Framebuffer {
    /// Initial address of the framebuffer.
    pub address: *mut u8,
    /// Dimensions of the display in pixels (width, height).
    pub resolution: (usize, usize),
    /// Format of each pixel in the screen.
    pub pixel_format: PixelFormat,
    /// Bitmaks used in case PixelFormat::Bitmask.
    pub bitmask: Option<PixelBitmask>,
    /// Strides determines the size of each row in the framebuffer. This may be >= resolution.0.
    pub stride: usize,
}
