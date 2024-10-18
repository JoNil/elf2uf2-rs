// Based off info from 
// https://infocenter.nordicsemi.com/index.jsp?topic=%2Fps_nrf52840%2Fmemory.html
// and https://github.com/adafruit/Adafruit_nRF52_Bootloader/blob/master/src/boards/circuitplayground_nrf52840/pinconfig.c

use crate::address_range::{AddressRange, AddressRangeType};

use super::BoardConfig;

const FLASH_SECTOR_ERASE_SIZE: u32 = 4096;
const MAIN_RAM_START: u32 = 0x00800000;
const MAIN_RAM_END: u32 = 0x10000000;
const FLASH_START: u32 = 0x00100000;
const FLASH_END: u32 = 0x00800000;
const XIP_SRAM_START: u32 = 0x12000000;
const XIP_SRAM_END: u32 = 0x19FFFFFF;
const MAIN_RAM_BANKED_START: u32 = 0x60000000;
const MAIN_RAM_BANKED_END: u32 = 0xA0000000;
const BOOTLOADER_FLASH_START: u32 = 0x00000000;
const BOOTLOADER_FLASH_END: u32 = 0x00100000;

pub struct CircuitPlaygroundBluefruit {}

impl BoardConfig for CircuitPlaygroundBluefruit {
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
        0xada52840
    }
    
    fn address_ranges_flash(&self) -> Vec<AddressRange> {
        vec![            
            AddressRange::new(FLASH_START, FLASH_END, AddressRangeType::Contents),
            AddressRange::new(MAIN_RAM_START, MAIN_RAM_END, AddressRangeType::NoContents),
            AddressRange::new(
                MAIN_RAM_BANKED_START,
                MAIN_RAM_BANKED_END,
                AddressRangeType::NoContents,
            ),
        ]
    }
    
    fn address_range_ram(&self) -> Vec<AddressRange> {
        vec![ 
            AddressRange::new(MAIN_RAM_START, MAIN_RAM_END, AddressRangeType::Contents),
            AddressRange::new(XIP_SRAM_START, XIP_SRAM_END, AddressRangeType::Contents),
            AddressRange::new(
                BOOTLOADER_FLASH_START,
                BOOTLOADER_FLASH_END, AddressRangeType::Ignore
            )
        ]
    }
}