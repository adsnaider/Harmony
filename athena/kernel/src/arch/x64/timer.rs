#![allow(unused)]
//! Implementaion of the 8253 PIT.

use core::marker::PhantomData;

use x86_64::instructions::port::Port;

/// Frequency of the internal oscillator in Hz.
pub const OSCILATING_FREQ: f32 = 1193182.0;

#[allow(missing_copy_implementations)]
#[derive(Debug)]
pub struct Pit8253 {
    _private: (),
}

impl Pit8253 {
    /// Constructs a new instance of the 8253 PIT.
    ///
    /// # Safety
    ///
    /// There can only be 1 instance of the PIT at any time in the program.
    pub(super) const unsafe fn steal() -> Self {
        Self { _private: () }
    }

    /// Transforms this PIT into a timer with interrupts at IRQ0.
    pub fn into_timer(self, reset_value: u16) -> PitTimer {
        PitTimer::init(reset_value)
    }
}

#[allow(missing_copy_implementations)]
#[derive(Debug)]
pub struct PitTimer {
    reset_value: u16,
    channel0: Port<u8>,
    mode: Port<u8>,
}

impl PitTimer {
    /// Sets the PIT into a timer interrupt generator that goes off every
    /// `reset_value / OSCCILATING_FREQ`s.
    ///
    /// # Panics
    ///
    /// We currently don't allow a `reset_value` of 0.
    fn init(reset_value: u16) -> Self {
        let mut this = Self {
            reset_value,
            channel0: Port::new(0x40),
            mode: Port::new(0x43),
        };
        // SAFETY: No other side effects.
        critical_section::with(|_cs| unsafe {
            // Set PIT to channel 0, mode 3 in low/high byte.
            this.mode.write(0b00110110);
            this.reset(reset_value);
        });
        this
    }

    /// Changes the reset counter of the PIT. This also forces the internal counter to be reset.
    pub fn reset(&mut self, reset_value: u16) {
        assert!(reset_value % 2 == 0, "The PIT timer works in mode 3 which only makes sense for even values: See https://wiki.osdev.org/Programmable_Interval_Timer#Mode_3_.E2.80.93_Square_Wave_Generator");
        assert!(
            reset_value != 0,
            "Reset value of 0 is currently not supported."
        );
        // SAFETY: No other side effects, the reset_value is valid.
        critical_section::with(|_cs| unsafe {
            // Low byte
            self.channel0.write((reset_value & 0xFF) as u8);
            // High byte
            self.channel0.write(((reset_value >> 8) & 0xFF) as u8);
        });
    }

    /// Read's the PIT's current count.
    pub fn read_count(&mut self) -> u16 {
        critical_section::with(|_cs| {
            let mut count: u16;
            // SAFETY: No other side effects.
            unsafe {
                self.mode.write(0b00000000);
                count = self.channel0.read() as u16;
                count |= (self.channel0.read() as u16) << 8;
            }
            count
        })
    }

    /// Get's the PIT's configured reset value.
    pub fn reset_value(&self) -> u16 {
        self.reset_value
    }
}
