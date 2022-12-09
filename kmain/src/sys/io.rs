//! I/O functionality for interacting with the keyboard, console, etc.

use core::future::Future;

use pc_keyboard::layouts::Us104Key;
use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1};

use super::interrupts::async_interrupt::{BoundedBufferInterrupt, InterruptFuture};

/// Initializes the I/O submodule.
///
/// # Arguments
///
/// * `keyboard_future`: The future associated with the keyboard interrupt.
pub(super) fn init(
    keyboard_future: InterruptFuture<'static, BoundedBufferInterrupt<u8>>,
) -> impl Future<Output = ()> + 'static {
    keyboard_handler(keyboard_future)
}

async fn keyboard_handler(
    mut keyboard_future: InterruptFuture<'static, BoundedBufferInterrupt<u8>>,
) {
    let mut keyboard = Keyboard::new(Us104Key, ScancodeSet1, HandleControl::Ignore);

    loop {
        let scancode = keyboard_future.next().await;

        match keyboard.add_byte(scancode) {
            Ok(Some(event)) => {
                if let Some(key) = keyboard.process_keyevent(event) {
                    match key {
                        DecodedKey::Unicode(character) => {
                            // Until we have something better to do with this...
                            let _ = print!("{}", character);
                        }
                        DecodedKey::RawKey(key) => {
                            // Until we have something better to do with this...
                            let _ = print!("{:?}", key);
                        }
                    }
                }
            }
            Ok(None) => {}
            Err(e) => {
                let _ = print!("Keyboard error: {e:?}");
            }
        }
    }
}
