#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AddressRangeType {
    /// May have contents
    Contents,
    /// Must be uninitialized
    NoContents,
    /// will be ignored
    Ignore,
}

#[derive(Copy, Clone, Debug)]
pub struct AddressRange {
    pub typ: AddressRangeType,
    pub to: u64,
    pub from: u64,
}

impl AddressRange {
    pub const fn new(from: u64, to: u64, typ: AddressRangeType) -> Self {
        Self { typ, to, from }
    }
}

impl Default for AddressRange {
    fn default() -> Self {
        Self {
            typ: AddressRangeType::Ignore,
            to: 0,
            from: 0,
        }
    }
}

pub const FLASH_SECTOR_ERASE_SIZE: u64 = 4096;
pub const MAIN_RAM_START_RP2040: u64 = 0x20000000;
pub const MAIN_RAM_END_RP2040: u64 = 0x20042000;
pub const MAIN_RAM_START_RP2350: u64 = 0x20000000;
pub const MAIN_RAM_END_RP2350: u64 = 0x20082000;
pub const FLASH_START_RP2040: u64 = 0x10000000;
pub const FLASH_END_RP2040: u64 = 0x15000000;
// From RP2350 datasheet:
// RP2040 required images to be stored at the beginning of flash (0x10000000). RP2350 supports storing executable images
// in a partitions at arbitrary locations, to support more robust upgrade cycles via A/B versions, among other uses.
// Therefore, the values below are possibly incorrect but FLASH_END_RP2040 appears to be incorrect too
pub const FLASH_START_RP2350: u64 = 0x10000000;
pub const FLASH_END_RP2350: u64 = 0x15000000;
pub const XIP_SRAM_START_RP2040: u64 = 0x15000000;
pub const XIP_SRAM_END_RP2040: u64 = 0x15004000;
pub const XIP_SRAM_START_RP2350: u64 = 0x13ffc000;
pub const XIP_SRAM_END_RP2350: u64 = 0x14000000;
pub const MAIN_RAM_BANKED_START_RP2040: u64 = 0x21000000;
pub const MAIN_RAM_BANKED_END_RP2040: u64 = 0x21040000;
pub const ROM_START_RP2040: u64 = 0x00000000;
pub const ROM_END_RP2040: u64 = 0x00004000;
pub const ROM_START_RP2350: u64 = 0x00000000;
pub const ROM_END_RP2350: u64 = 0x00008000;

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
