use crate::{
    address_range::{AddressRange, AddressRangeType},
    boards::{AddressLocations, BoardInfo, UsbDevice},
};

#[derive(Debug, Default, Clone)]
pub struct RP2040;

impl BoardInfo for RP2040 {
    fn is_device_board(&self, device: &UsbDevice) -> bool {
        if device.vendor_id == 0x2e8a || device.product_id == 0x0003 {
            return true;
        }
        return false;
    }

    fn family_id(&self) -> u32 {
        0xe48bff56
    }

    fn address_locations<'a>(&'a self) -> AddressLocations<'a> {
        AddressLocations {
            address_ranges_ram: Some(RP2040_ADDRESS_RANGES_RAM),
            address_ranges_flash: Some(RP2040_ADDRESS_RANGES_FLASH),
            main_ram_start: Some(MAIN_RAM_START_RP2040),
            main_ram_end: Some(MAIN_RAM_END_RP2040),
            xip_sram_start: Some(XIP_SRAM_START_RP2040),
            xip_sram_end: Some(XIP_SRAM_END_RP2040),
        }
    }

    fn board_name(&self) -> String {
        "rp2040".to_string()
    }
}

pub const MAIN_RAM_START_RP2040: u64 = 0x20000000;
pub const MAIN_RAM_END_RP2040: u64 = 0x20042000;
pub const FLASH_START_RP2040: u64 = 0x10000000;
pub const FLASH_END_RP2040: u64 = 0x15000000;

pub const XIP_SRAM_START_RP2040: u64 = 0x15000000;
pub const XIP_SRAM_END_RP2040: u64 = 0x15004000;

pub const MAIN_RAM_BANKED_START_RP2040: u64 = 0x21000000;
pub const MAIN_RAM_BANKED_END_RP2040: u64 = 0x21040000;

pub const ROM_START_RP2040: u64 = 0x00000000;
pub const ROM_END_RP2040: u64 = 0x00004000;

pub const RP2040_ADDRESS_RANGES_FLASH: &[AddressRange] = &[
    AddressRange::new(
        FLASH_START_RP2040,
        FLASH_END_RP2040,
        AddressRangeType::Contents,
    ),
    AddressRange::new(
        MAIN_RAM_START_RP2040,
        MAIN_RAM_END_RP2040,
        AddressRangeType::NoContents,
    ),
    AddressRange::new(
        MAIN_RAM_BANKED_START_RP2040,
        MAIN_RAM_BANKED_END_RP2040,
        AddressRangeType::NoContents,
    ),
];

pub const RP2040_ADDRESS_RANGES_RAM: &[AddressRange] = &[
    AddressRange::new(
        MAIN_RAM_START_RP2040,
        MAIN_RAM_END_RP2040,
        AddressRangeType::Contents,
    ),
    AddressRange::new(
        XIP_SRAM_START_RP2040,
        XIP_SRAM_END_RP2040,
        AddressRangeType::Contents,
    ),
    AddressRange::new(ROM_START_RP2040, ROM_END_RP2040, AddressRangeType::Ignore), // for now we ignore the bootrom if present
];
