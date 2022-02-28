//! Functionalities for displays and frames.
#![cfg_attr(not(test), no_std)]
#![deny(absolute_paths_not_starting_with_crate)]
#![warn(missing_debug_implementations)]
#![warn(missing_copy_implementations)]
#![warn(missing_docs)]

/// A pixel is the atomic element that `Frames` are constructed from.
///
/// It has fields for red, green, and blue intensities.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Pixel {
    /// Red intensity.
    pub red: u8,
    /// Green intensity.
    pub green: u8,
    /// Blue intensity.
    pub blue: u8,
}

/// The `Frame` trait defines operations on pixels within a frame.
///
/// # Safety
///
/// `width()` and `height()` must correctly represent the width and height of the frame (in
/// pixels).
pub unsafe trait Frame {
    /// Sets the pixel in (`row`, `col`) to `pixel`, effectively modifying the frame.
    ///
    /// # Panics
    ///
    /// If `row` or `col` are out of bounds.
    fn set_pixel(&mut self, row: usize, col: usize, pixel: Pixel) {
        if row >= self.height() || col >= self.width() {
            panic!("Attempted to set out of bounds pixel: ({row}, {col})");
        }
        unsafe {
            self.set_pixel_unchecked(row, col, pixel);
        }
    }

    /// Sets the pixel in (`row`, `col`) to `pixel`, effectively modifying the frame.
    ///
    /// # Safety
    ///
    /// `row` and `col` must be in bounds of `self.height()` and `self.width()`, respectively.
    unsafe fn set_pixel_unchecked(&mut self, row: usize, col: usize, pixel: Pixel);

    /// Width of the buffer in pixels.
    fn width(&self) -> usize;
    /// Height of the buffer in pixels.
    fn height(&self) -> usize;
}

#[cfg(target_os = "linux")]
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert!(true);
    }
}
