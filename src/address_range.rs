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
    pub to: u32,
    pub from: u32,
}

impl AddressRange {
    pub const fn new(from: u32, to: u32, typ: AddressRangeType) -> Self {
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

pub const FLASH_SECTOR_ERASE_SIZE: u32 = 4096;
pub const MAIN_RAM_START: u32 = 0x20000000;
pub const MAIN_RAM_END: u32 = 0x20042000;
pub const FLASH_START: u32 = 0x10000000;
pub const FLASH_END: u32 = 0x15000000;
pub const XIP_SRAM_START: u32 = 0x15000000;
pub const XIP_SRAM_END: u32 = 0x15004000;
pub const MAIN_RAM_BANKED_START: u32 = 0x21000000;
pub const MAIN_RAM_BANKED_END: u32 = 0x21040000;
pub const ROM_START: u32 = 0x00000000;
pub const ROM_END: u32 = 0x00004000;

pub const RP2040_ADDRESS_RANGES_FLASH: &[AddressRange] = &[
    AddressRange::new(FLASH_START, FLASH_END, AddressRangeType::Contents),
    AddressRange::new(MAIN_RAM_START, MAIN_RAM_END, AddressRangeType::NoContents),
    AddressRange::new(
        MAIN_RAM_BANKED_START,
        MAIN_RAM_BANKED_END,
        AddressRangeType::NoContents,
    ),
];

pub const RP2040_ADDRESS_RANGES_RAM: &[AddressRange] = &[
    AddressRange::new(MAIN_RAM_START, MAIN_RAM_END, AddressRangeType::Contents),
    AddressRange::new(XIP_SRAM_START, XIP_SRAM_END, AddressRangeType::Contents),
    AddressRange::new(ROM_START, ROM_END, AddressRangeType::Ignore), // for now we ignore the bootrom if present
];
