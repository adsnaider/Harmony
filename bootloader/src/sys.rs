//! UEFI System.
pub mod alloc;
pub mod fs;
pub mod io;
pub mod mem;

use core::cell::UnsafeCell;

use uefi::table::{Boot, SystemTable};

/// GlobalTable struct.
pub(crate) struct GlobalTable {
    pub table: UnsafeCell<Option<SystemTable<Boot>>>,
}
// SAFETY: Not safe, but UEFI has no threading support.
unsafe impl Sync for GlobalTable {}

/// System table used by the rest of the system. In order for get/get_mut to be safe, each part of
/// the code should only access the specific sub-system that they have access to.
///
/// For instance, the logging system, can access stdout(), and the framebuffer can access gop().
pub(crate) static SYSTEM_TABLE: GlobalTable = GlobalTable {
    table: UnsafeCell::new(None),
};

impl GlobalTable {
    pub unsafe fn get(&self) -> &SystemTable<Boot> {
        (&*self.table.get())
            .as_ref()
            .expect("System table hasn't been initialized. Forget to call `init()`?")
    }

    pub unsafe fn get_mut(&self) -> &mut SystemTable<Boot> {
        (&mut *self.table.get())
            .as_mut()
            .expect("System table hasn't been initialized. Forget to call `init()`?")
    }

    unsafe fn set(&self, table: SystemTable<Boot>) {
        *self.table.get() = Some(table)
    }

    unsafe fn is_set(&self) -> bool {
        (&*self.table.get()).is_some()
    }
}

/// Initializes the UEFI system. After this call, it's possible to use allocation services and
/// logging.
pub fn init(system_table: SystemTable<Boot>) {
    unsafe {
        if SYSTEM_TABLE.is_set() {
            panic!("Attempt to call sys::init() twice.");
        }
        SYSTEM_TABLE.set(system_table);
    }

    io::init();
}

/// Returns true if the UEFI system has been initialized with a call to `init()`.
pub fn is_init() -> bool {
    unsafe { SYSTEM_TABLE.is_set() }
}
