//! Functionalities for displays and frames.
#![cfg_attr(not(test), no_std)]
#![deny(absolute_paths_not_starting_with_crate)]
#![warn(missing_debug_implementations)]
#![warn(missing_copy_implementations)]
#![warn(missing_docs)]

pub mod console;

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

impl Pixel {
    /// Creates a new pixel with the RGB values.
    pub fn new(red: u8, green: u8, blue: u8) -> Pixel {
        Pixel { red, green, blue }
    }

    /// Creates the black pixel.
    pub fn black() -> Pixel {
        Pixel {
            red: 0,
            green: 0,
            blue: 0,
        }
    }

    /// Creates the white pixel.
    pub fn white() -> Pixel {
        Pixel {
            red: 255,
            green: 255,
            blue: 255,
        }
    }

    /// Creates the red pixel.
    pub fn red() -> Pixel {
        Pixel {
            red: 255,
            green: 0,
            blue: 0,
        }
    }

    /// Creates the green pixel.
    pub fn green() -> Pixel {
        Pixel {
            red: 0,
            green: 255,
            blue: 0,
        }
    }

    /// Creates the blue pixel.
    pub fn blue() -> Pixel {
        Pixel {
            red: 0,
            green: 0,
            blue: 255,
        }
    }
}

/// Error returned when indeces are out of bounds.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct OutOfBoundsError {}

/// The `Frame` trait defines operations on pixels within a frame.
///
/// # Safety
///
/// `width()` and `height()` must correctly represent the width and height of the frame (in
/// pixels).
pub unsafe trait Frame {
    /// Sets the pixel in (`row`, `col`) to `pixel`, effectively modifying the frame.
    ///
    /// Returns an `OutOfBoundsError` when the either `row` or `col` are out of bounds.
    fn set_pixel(&mut self, row: usize, col: usize, pixel: Pixel) -> Result<(), OutOfBoundsError> {
        if row >= self.height() || col >= self.width() {
            Err(OutOfBoundsError {})
        } else {
            unsafe {
                self.set_pixel_unchecked(row, col, pixel);
            }
            Ok(())
        }
    }

    /// Fills the entire frame with the pixel.
    fn fill_with(&mut self, pixel: Pixel) {
        for row in 0..self.height() {
            for col in 0..self.width() {
                unsafe {
                    self.set_pixel_unchecked(row, col, pixel);
                }
            }
        }
    }

    /// Sets the pixel in (`row`, `col`) to `pixel`, effectively modifying the frame.
    ///
    /// # Safety
    ///
    /// `row` and `col` must be in bounds of `self.height()` and `self.width()`, respectively.
    /// It's up to the implementation to guarantee that this operation is safe when
    /// `row < self.height()` and `col < self.width()`.
    unsafe fn set_pixel_unchecked(&mut self, row: usize, col: usize, pixel: Pixel);

    /// Width of the buffer in pixels.
    fn width(&self) -> usize;
    /// Height of the buffer in pixels.
    fn height(&self) -> usize;
}

#[cfg(test)]
pub(crate) mod test_utils {
    use std::ops::Index;

    use super::*;
    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    pub(crate) struct SimpleFrame<const W: usize, const H: usize> {
        frame: [[Pixel; W]; H],
    }

    impl<const W: usize, const H: usize> SimpleFrame<W, H> {
        pub fn new() -> Self {
            SimpleFrame {
                frame: [[Pixel::black(); W]; H],
            }
        }
    }

    impl<const W: usize, const H: usize> Index<(usize, usize)> for SimpleFrame<W, H> {
        type Output = Pixel;
        fn index(&self, index: (usize, usize)) -> &Self::Output {
            &self.frame[index.0][index.1]
        }
    }

    // SAFETY:
    unsafe impl<const W: usize, const H: usize> Frame for SimpleFrame<W, H> {
        unsafe fn set_pixel_unchecked(&mut self, row: usize, col: usize, pixel: Pixel) {
            self.frame[row][col] = pixel;
        }

        fn width(&self) -> usize {
            W
        }

        fn height(&self) -> usize {
            H
        }
    }
}

#[cfg(test)]
pub(crate) use crate::test_utils::SimpleFrame;

#[cfg(target_os = "linux")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_pixel_is_ok() {
        let mut frame = SimpleFrame::<10, 10>::new();
        assert!(frame.set_pixel(3, 5, Pixel::red()).is_ok());
        assert!(frame[(3, 5)] == Pixel::red());

        assert!(frame.set_pixel(0, 0, Pixel::blue()).is_ok());
        assert!(frame[(0, 0)] == Pixel::blue());

        assert!(frame.set_pixel(9, 9, Pixel::blue()).is_ok());
        assert!(frame[(9, 9)] == Pixel::blue());
    }

    #[test]
    fn set_pixel_oor_errors() {
        let mut frame = SimpleFrame::<10, 10>::new();
        assert!(frame.set_pixel(10, 12, Pixel::red()).is_err());
        assert!(frame.set_pixel(10, 5, Pixel::red()).is_err());
        assert!(frame.set_pixel(13, 5, Pixel::red()).is_err());
        assert!(frame.set_pixel(10, 10, Pixel::red()).is_err());
        assert!(frame.set_pixel(10, 0, Pixel::red()).is_err());
    }
}
