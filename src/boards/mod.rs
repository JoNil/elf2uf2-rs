use crate::address_range::AddressRange;

pub trait BoardConfig {
    fn flash_sector_erase_size(&self) -> u32;
    fn main_ram_start(&self) -> u32;
    fn main_ram_end(&self) -> u32;
    fn xip_ram_start(&self) -> u32;
    fn xip_ram_end(&self) -> u32;
    fn family_id(&self) -> u32;

    fn address_ranges_flash(&self) -> Vec<AddressRange>;
    fn address_range_ram(&self) -> Vec<AddressRange>;
}

pub mod rp2040;
pub mod circuit_playground_bluefruit;