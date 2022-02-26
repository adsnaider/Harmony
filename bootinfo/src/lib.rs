//! This crate defines the Bootinfo structure that gets passed into the kenrel by the bootloader.
#![no_std]
#![warn(missing_docs)]
#![warn(missing_debug_implementations)]
#![warn(missing_copy_implementations)]

/// Memory type used to load all the statics to the kernel.
pub const KERNEL_STATIC: u32 = 0x80000000;
/// Memory type used to identify the kernel stack.
pub const KERNEL_STACK: u32 = 0x80000001;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
/// The `Bootinfo` struct gets passed from the bootloader to the kernel.
pub struct Bootinfo {
    /// Framebuffer structure that can be used in the kernel to control the screen.
    pub framebuffer: Framebuffer,
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
