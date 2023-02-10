//! System management and functionality.

#[macro_use]
mod display;
mod drivers;
mod gdt;
mod interrupts;
mod memory;

pub mod io;
pub mod time;

use alloc::boxed::Box;
use core::future::Future;

use bootloader_api::info::{MemoryRegion, Optional};
use bootloader_api::BootInfo;
pub use display::{_print, _try_print};
use framed::console::{BitmapFont, Console};
use framed::{Frame, Pixel};
use futures::join;
use x86_64::VirtAddr;

use self::display::Display;
use crate::sys::interrupts::async_interrupt::{BoundedBufferInterrupt, InterruptWakerCore};
use crate::sys::interrupts::{KEYBOARD_INTERRUPT_CORE, TIMER_INTERRUPT_CORE};

const FONT: &[u8] = include_bytes!("../font.bdf");

/// System intialization routine.
///
/// It sets up the display, initializes the console, sets up the logger, memory utilities and
/// allocation, and it initializes interrupts.
///
/// # Safety
///
/// The information in `bootinfo` must be accurate.
pub(super) unsafe fn init(bootinfo: &mut BootInfo) -> impl Future<Output = ()> + 'static {
    let tasks = critical_section::with(|cs| {
        // SAFETY: Bootloader passed the framebuffer correctly.
        let framebuffer = core::mem::replace(&mut bootinfo.framebuffer, Optional::None)
            .into_option()
            .unwrap();
        let framebuffer_addr = framebuffer.buffer() as *const [u8];
        let mut display = unsafe { Display::new(framebuffer) };

        display.fill_with(Pixel::black());
        let font = match BitmapFont::decode_from(FONT) {
            Ok(font) => font,
            Err(_) => {
                display.fill_with(Pixel::red());
                panic!("Can't get display to work.");
            }
        };
        display::init(Console::new(display, font));
        println!("Hello, Kernel!");
        log::info!("Hello, logging!");

        log::debug!(
            "Memory map starts at {:#?}",
            &*bootinfo.memory_regions as *const [MemoryRegion]
        );
        let pmo = bootinfo
            .physical_memory_offset
            .into_option()
            .expect("No memory offset found from bootloader.");
        log::debug!("Physical memory offset is {:#?}", pmo as *const ());
        log::debug!("Framebuffer mapped to {:#?}", framebuffer_addr);

        // SAFETY: The physical memory offset is correct, well-aligned, and canonical, and the memory
        // map is correct from the bootloader.
        unsafe {
            memory::init(VirtAddr::new_unsafe(pmo), &mut bootinfo.memory_regions);
        }

        let boxed_value = Box::new(25);
        log::info!("We are boxing! {boxed_value:?}");
        let vec_value = vec![1, 2, 3, 4, 5, 6];
        log::info!("And we are vecing: {vec_value:?}");
        let huge_vec = vec![0u64; 100000];
        for x in huge_vec.iter() {
            // SAFETY: Pointer is valid as we construct it from reference.
            unsafe { core::ptr::read_volatile(x as *const u64) };
        }
        log::info!("Allocated a huge vector!");

        let tick_task = time::init(
            drivers::take_pit(cs).unwrap(),
            TIMER_INTERRUPT_CORE
                .take_future()
                .expect("Someone stole the timer future :O"),
        );
        KEYBOARD_INTERRUPT_CORE
            .set(BoundedBufferInterrupt::new(128))
            .unwrap_or_else(|_| {
                panic!("Someone already initialized the keyboard interrupt core :O")
            });
        let io_task = io::init(
            KEYBOARD_INTERRUPT_CORE
                .get()
                .unwrap()
                .take_future()
                .expect("Someone stole the keyboard future :O"),
        );

        gdt::init();
        interrupts::init(cs);
        log::info!("Interrupt handlers initialized");
        async {
            join!(tick_task, io_task);
        }
    });
    x86_64::instructions::interrupts::enable();
    tasks
}

struct SingleThreadCS();
critical_section::set_impl!(SingleThreadCS);
/// SAFETY: While the OS kernel is running in a single thread, then disabling interrupts is a safe
/// to guarantee a critical section's conditions.
unsafe impl critical_section::Impl for SingleThreadCS {
    unsafe fn acquire() -> critical_section::RawRestoreState {
        let interrupts_enabled = x86_64::instructions::interrupts::are_enabled();
        x86_64::instructions::interrupts::disable();
        interrupts_enabled
    }

    unsafe fn release(interrupts_were_enabled: critical_section::RawRestoreState) {
        if interrupts_were_enabled {
            x86_64::instructions::interrupts::enable();
        }
    }
}
