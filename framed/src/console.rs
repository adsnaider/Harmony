//! Defines a console to wrap the frame.
use core::fmt::Write;
use core::mem::MaybeUninit;
use core::ops::Index;

use crate::{Frame, Pixel};

const FONT_HEIGHT: usize = 16;

/// A console struct that wraps a frame to allow for text rendering.
#[derive(Debug)]
pub struct Console<F: Frame> {
    cursor: (usize, usize),
    frame: F,
    font: BitmapFont,
}

impl<F: Frame> Console<F> {
    /// Constructs a console from the frame.
    pub fn new(frame: F, font: BitmapFont) -> Self {
        Self {
            frame,
            cursor: (0, 0),
            font,
        }
    }
}

impl<F: Frame> Console<F> {
    /// Go to the next line.
    fn next_line_or_clear(&mut self) {
        self.cursor.0 += FONT_HEIGHT;
        self.cursor.1 = 0;
        if self.cursor.0 >= self.frame.height() - FONT_HEIGHT {
            self.frame.fill_with(Pixel::black());
            self.cursor.0 = 0;
            self.cursor.1 = 0;
        }
    }

    /// Move the cursor forward `pixel` steps. If reach the end of the frame, move to the next
    /// line.
    fn wrap_add(&mut self, pixels: usize) {
        self.cursor.1 += pixels;
        if self.cursor.1 >= self.frame.width() - 8 {
            self.next_line_or_clear();
        }
    }

    /// Writes the byte to the console and moves the cursor.
    fn write_byte(&mut self, byte: u8) {
        let bitchar = self.font[byte];
        for (row_offset, col_offset, value) in bitchar.iter() {
            self.frame
                .set_pixel(
                    self.cursor.0 + row_offset,
                    self.cursor.1 + col_offset,
                    match value {
                        true => Pixel::white(),
                        false => Pixel::black(),
                    },
                )
                .expect("Attempted to set pixel out of bounds.");
        }
        self.wrap_add(8);
    }
}

impl<F: Frame> Write for Console<F> {
    fn write_str(&mut self, s: &str) -> Result<(), core::fmt::Error> {
        if !s.is_ascii() {
            return Err(core::fmt::Error {});
        }
        // Safe to interpret as bytes since we checked that s.is_ascii().
        for c in s.bytes() {
            match c {
                b'\n' => self.next_line_or_clear(),
                0x20..=0x7E => self.write_byte(c),
                _ => self.write_byte(0xFE),
            }
        }
        Ok(())
    }
}

// TODO(#11): Create Font trait and make these private.

/// Bitmap encoded fonts.
///
/// A bitmap font is a font that is encoded as a large array where each letter becomes a grid of
/// bits. The bits represent whether it's on or off at that position. Each letter is indexed by its
/// ascii code. See more at <https://wiki.osdev.org/VGA_Fonts>.
#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
pub struct BitmapFont {
    letters: [BitmapChar; 256],
}

/// Error returned when the bitmap font couldn't be decoded.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct DecodeError {}

impl BitmapFont {
    /// Decode a font and return the `BitmapFont`.
    pub fn decode_from(encoded: &[u8]) -> Result<BitmapFont, DecodeError> {
        if encoded.len() == core::mem::size_of::<BitmapFont>() {
            // SAFETY: Size is the same and representation is transparent. All of these decode to a
            // [u8; 256 * FONT_HEIGHT] array, so there can't be malinitialized memory.
            unsafe {
                let mut font = MaybeUninit::uninit();
                core::ptr::copy(
                    encoded.as_ptr(),
                    &mut font as *mut MaybeUninit<BitmapFont> as *mut u8,
                    core::mem::size_of::<BitmapFont>(),
                );
                Ok(font.assume_init())
            }
        } else {
            Err(DecodeError {})
        }
    }
}

impl Index<u8> for BitmapFont {
    type Output = BitmapChar;

    fn index(&self, index: u8) -> &Self::Output {
        &self.letters[index as usize]
    }
}

/// A single bitmap character.
///
/// Each character is `FONT_HEIGHT` bytes where each byte is one row.
#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
pub struct BitmapChar {
    letter: [u8; FONT_HEIGHT],
}

impl BitmapChar {
    /// Creates an iterator to go through each bit in the character.
    pub fn iter(&self) -> BitmapCharIterator<'_> {
        BitmapCharIterator {
            bitmap_char: self,
            row: 0,
            col: 0,
        }
    }
}

/// An iterator for a bitmap character.
#[derive(Debug, Copy, Clone)]
pub struct BitmapCharIterator<'a> {
    bitmap_char: &'a BitmapChar,
    row: usize,
    col: usize,
}

impl Iterator for BitmapCharIterator<'_> {
    type Item = (usize, usize, bool);

    fn next(&mut self) -> Option<Self::Item> {
        if self.row >= FONT_HEIGHT {
            None
        } else {
            let result = (
                self.row,
                self.col,
                (self.bitmap_char.letter[self.row] >> (7 - self.col)) % 2 == 1,
            );
            self.col += 1;
            if self.col >= 8 {
                self.row += 1;
                self.col = 0;
            }
            Some(result)
        }
    }
}

#[cfg(target_os = "linux")]
#[cfg(test)]
mod tests {
    // TODO(#12): Add some tests after #11 is fixed.
}
