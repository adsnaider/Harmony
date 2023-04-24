use critical_section::CriticalSection;
use singleton::Singleton;
use x86_64::structures::paging::page_table::PageTableEntry;
use x86_64::structures::paging::{OffsetPageTable, PageTable, Translate};
use x86_64::{PhysAddr, VirtAddr};

pub static PAGE_MAPPER: Singleton<OffsetPageTable<'static>> = Singleton::uninit();

// SAFETY: Address is canonical.
pub(super) static PHYSICAL_MEMORY_OFFSET: VirtAddr =
    unsafe { VirtAddr::new_unsafe(0xFFFF_F000_0000_0000) };

pub fn init(pmo: VirtAddr, cs: CriticalSection) {
    assert_eq!(pmo, PHYSICAL_MEMORY_OFFSET);
    let l4_table = {
        let (frame, _) = x86_64::registers::control::Cr3::read();
        let virt = pmo + frame.start_address().as_u64();
        // SAFETY: This is valid since the PageTable is initialized in the cr3 and the physical
        // memory offset must be correct.
        unsafe { &mut *(virt.as_u64() as *mut PageTable) }
    };

    // SAFETY: We get the l4_table provided by the bootloader which maps the memory to
    // `pmo`.
    let page_map = unsafe { OffsetPageTable::new(l4_table, pmo) };

    // Sanity check, let's check some small addresses, should be mapped to themselves.
    assert!(page_map.translate_addr(pmo + 0x0u64) == Some(PhysAddr::new(0)));
    assert!(page_map.translate_addr(pmo + 0xABCDu64) == Some(PhysAddr::new(0xABCD)));
    assert!(page_map.translate_addr(pmo + 0xABAB_0000u64) == Some(PhysAddr::new(0xABAB_0000)));

    PAGE_MAPPER.initialize(page_map, cs);
}

/// Returns a new page table l4 that contains all the kernel entries.
pub(super) fn dup_page_table() -> PageTable {
    let mut l4_table = critical_section::with(|cs| PAGE_MAPPER.lock(cs).level_4_table().clone());
    // clear the user-level entries.
    for i in 0..128 {
        l4_table[i] = PageTableEntry::new();
    }
    l4_table
}
