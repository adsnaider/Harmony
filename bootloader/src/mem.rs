//! General memory utilities.

/// Returns a pointer that has been aligned to alignment by increasing it's value to appropriate
/// alignment.
pub unsafe fn aligned_to_high(pointer: *mut u8, alignment: usize) -> *mut u8 {
    // (8 - 8 % 8) % 8 = 0;
    // (8 - 7 % 8) % 8 = 1;
    // (8 - 6 % 8) % 8 = 2;
    let offset = (alignment - pointer as usize % alignment) % alignment;
    pointer.add(offset)
}
