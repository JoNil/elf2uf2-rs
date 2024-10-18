use crate::address_range::{AddressRange, AddressRangeType};

use super::BoardConfig;

const FLASH_SECTOR_ERASE_SIZE: u32 = 4096;
const MAIN_RAM_START: u32 = 0x20000000;
const MAIN_RAM_END: u32 = 0x20042000;
const FLASH_START: u32 = 0x10000000;
const FLASH_END: u32 = 0x15000000;
const XIP_SRAM_START: u32 = 0x15000000;
const XIP_SRAM_END: u32 = 0x15004000;
const MAIN_RAM_BANKED_START: u32 = 0x21000000;
const MAIN_RAM_BANKED_END: u32 = 0x21040000;
const ROM_START: u32 = 0x00000000;
const ROM_END: u32 = 0x00004000;

pub struct RP2040 {}

impl BoardConfig for RP2040 {
    fn flash_sector_erase_size(&self) -> u32 {
        FLASH_SECTOR_ERASE_SIZE
    }

    fn main_ram_start(&self) -> u32 {
        MAIN_RAM_START
    }

    fn main_ram_end(&self) -> u32 {
        MAIN_RAM_END
    }

    fn xip_ram_start(&self) -> u32 {
        XIP_SRAM_START
    }

    fn xip_ram_end(&self) -> u32 {
        XIP_SRAM_END
    }

    fn family_id(&self) -> u32 {
        0xe48bff56
    }
    
    fn address_ranges_flash(&self) -> Vec<AddressRange> {
        vec![            
            AddressRange::new(FLASH_START, FLASH_END, AddressRangeType::Contents),
            AddressRange::new(MAIN_RAM_START, MAIN_RAM_END, AddressRangeType::NoContents),
            AddressRange::new(
                MAIN_RAM_BANKED_START,
                MAIN_RAM_BANKED_END,
                AddressRangeType::NoContents,
            )
        ]
    }
    
    fn address_range_ram(&self) -> Vec<AddressRange> {
        vec![ 
            AddressRange::new(MAIN_RAM_START, MAIN_RAM_END, AddressRangeType::Contents),
            AddressRange::new(XIP_SRAM_START, XIP_SRAM_END, AddressRangeType::Contents),
            AddressRange::new(ROM_START, ROM_END, AddressRangeType::Ignore), // for now we ignore the bootrom if present
        ]
    }
}