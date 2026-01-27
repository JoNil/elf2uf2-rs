use crate::{
    address_range::{AddressRange, AddressRangeType},
    boards::{AddressLocations, BoardInfo, UsbDevice},
};

#[derive(Debug, Default, Clone)]
pub struct RP2350;

impl BoardInfo for RP2350 {
    fn is_device_board(&self, device: &UsbDevice) -> bool {
        if device.vendor_id == 0x2e8a || device.product_id == 0x000f {
            return true;
        }
        false
    }

    fn family_id(&self) -> u32 {
        // This is the rp2350 arm secure family id, should technically always be true if you held the bootsel button down and cycled power.
        0xe48bff59
    }

    fn address_locations<'a>(&'a self) -> AddressLocations<'a> {
        AddressLocations {
            address_ranges_ram: Some(RP2350_ADDRESS_RANGES_RAM),
            address_ranges_flash: Some(RP2350_ADDRESS_RANGES_FLASH),
            main_ram_start: Some(MAIN_RAM_START_RP2350),
            main_ram_end: Some(MAIN_RAM_END_RP2350),
            xip_sram_start: Some(XIP_SRAM_START_RP2350),
            xip_sram_end: Some(XIP_SRAM_END_RP2350),
        }
    }

    fn board_name(&self) -> &'static str {
        "rp2350"
    }
}

pub const MAIN_RAM_START_RP2350: u64 = 0x20000000;
pub const MAIN_RAM_END_RP2350: u64 = 0x20082000;

// From RP2350 datasheet:
// RP2040 required images to be stored at the beginning of flash (0x10000000). RP2350 supports storing executable images
// in a partitions at arbitrary locations, to support more robust upgrade cycles via A/B versions, among other uses.
// Therefore, the values below are possibly incorrect but FLASH_END_RP2040 appears to be incorrect too
pub const FLASH_START_RP2350: u64 = 0x10000000;
pub const FLASH_END_RP2350: u64 = 0x15000000;

pub const XIP_SRAM_START_RP2350: u64 = 0x13ffc000;
pub const XIP_SRAM_END_RP2350: u64 = 0x14000000;

pub const ROM_START_RP2350: u64 = 0x00000000;
pub const ROM_END_RP2350: u64 = 0x00008000;

pub const RP2350_ADDRESS_RANGES_FLASH: &[AddressRange] = &[
    AddressRange::new(
        FLASH_START_RP2350,
        FLASH_END_RP2350,
        AddressRangeType::Contents,
    ),
    AddressRange::new(
        MAIN_RAM_START_RP2350,
        MAIN_RAM_END_RP2350,
        AddressRangeType::NoContents,
    ),
];

pub const RP2350_ADDRESS_RANGES_RAM: &[AddressRange] = &[
    AddressRange::new(
        MAIN_RAM_START_RP2350,
        MAIN_RAM_END_RP2350,
        AddressRangeType::Contents,
    ),
    AddressRange::new(
        XIP_SRAM_START_RP2350,
        XIP_SRAM_END_RP2350,
        AddressRangeType::Contents,
    ),
    AddressRange::new(ROM_START_RP2350, ROM_END_RP2350, AddressRangeType::Ignore), // for now we ignore the bootrom if present
];
