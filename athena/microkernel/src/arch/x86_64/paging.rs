pub const PAGE_SIZE: usize = 4096;

pub struct Frame {
    phys_address: u64,
}

impl Frame {
    pub fn from_start_address(address: u64) -> Self {
        Self {
            phys_address: address,
        }
    }
}
