//! Page table management for x86_64 architecture.

use heapless::Vec;

use self::addr::{Frame, Page, PageTableLevel, PageTableOffset};
use crate::ops::paging::PermissionMask;
use crate::userspace::structures::PageTable;

pub mod addr;

pub struct Addrspace {
    l4_table: PageTable,
    mappings: Vec<Mapping, 32>,
}

#[derive(Copy, Clone, Debug)]
struct Mapping {
    from: PageTable,
    offset: PageTableOffset,
    to: Either<PageTable, Frame>,
}

#[derive(Copy, Clone, Debug)]
pub enum Either<T, U> {
    First(T),
    Second(U),
}

pub trait PageTableAllocator {
    fn allocate_table(&mut self, level: PageTableLevel) -> PageTable;
}

impl Addrspace {
    pub unsafe fn new(l4_table: PageTable) -> Self {
        Self {
            l4_table,
            mappings: Vec::new(),
        }
    }

    pub unsafe fn map_to<A: PageTableAllocator>(
        &mut self,
        page: Page,
        frame: Frame,
        permission_mask: PermissionMask,
        parent_mask: PermissionMask,
        allocator: &mut A,
    ) -> Result<(), MapperError> {
        log::info!("Mapping page: {page:X?} to frame: {frame:X?}");
        let mut level = Some(PageTableLevel::top());
        let mut table = self.l4_table;
        let addr = page.base();
        while let Some(current_level) = level {
            level = current_level.lower();
            let offset = addr.page_table_index(current_level);
            if current_level.is_bottom() {
                if let Some(Mapping {
                    from: _,
                    offset: _,
                    to: Either::Second(frame),
                }) = self
                    .mappings
                    .iter()
                    .find(|&m| m.from == table && m.offset == offset)
                {
                    return Err(MapperError::AlreadyMapped(*frame));
                }
                table
                    .map(offset.as_usize(), frame, permission_mask)
                    .unwrap();
                self.mappings
                    .push(Mapping {
                        from: table,
                        to: Either::Second(frame),
                        offset,
                    })
                    .unwrap();
            } else {
                if let Some(Mapping {
                    from: _,
                    offset: _,
                    to: Either::First(next_table),
                }) = self
                    .mappings
                    .iter()
                    .find(|&m| m.from == table && m.offset == offset)
                {
                    table = *next_table;
                } else {
                    let next_table = allocator.allocate_table(level.unwrap());
                    table
                        .link(next_table, offset.as_usize(), parent_mask)
                        .unwrap();
                    self.mappings
                        .push(Mapping {
                            from: table,
                            to: Either::First(next_table),
                            offset,
                        })
                        .unwrap();
                    table = next_table;
                }
            }
        }
        Ok(())
    }

    pub unsafe fn unmap(&mut self, page: Page) -> Result<Frame, UnmapError> {
        let mut level = Some(PageTableLevel::top());
        let mut table = self.l4_table;
        let addr = page.base();
        while let Some(current_level) = level {
            level = current_level.lower();
            let offset = addr.page_table_index(current_level);

            match self
                .mappings
                .iter()
                .copied()
                .enumerate()
                .find(|(_, m)| m.from == table && m.offset == offset)
            {
                Some((
                    _,
                    Mapping {
                        from: _,
                        offset: _,
                        to: Either::First(next_table),
                    },
                )) => {
                    table = next_table;
                }
                Some((
                    pos,
                    Mapping {
                        from: _,
                        offset: _,
                        to: Either::Second(frame),
                    },
                )) => {
                    table.unmap(offset.as_usize()).unwrap();
                    self.mappings.remove(pos);
                    return Ok(frame);
                }
                None => return Err(UnmapError::NotMapped),
            }
        }
        unreachable!();
    }

    pub fn into_inner(self) -> PageTable {
        self.l4_table
    }
}

#[derive(Debug)]
pub enum MapperError {
    TableAllocationError,
    HugeParentEntry,
    AlreadyMapped(Frame),
}

#[derive(Debug)]
pub enum UnmapError {
    NotMapped,
    HugeParent,
}
