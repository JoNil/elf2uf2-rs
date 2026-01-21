pub use rp2040::RP2040;
pub use rp2350::RP2350;

use crate::address_range::AddressRange;

pub mod rp2040;
pub mod rp2350;

/// This is a helper struct, which allows you to iterate over every board defined
#[derive(Default)]
pub struct BoardIter {
    inner: std::vec::IntoIter<Box<dyn BoardInfo>>,
}

impl BoardIter {
    /// Creates a new BoardIter
    pub fn new() -> Self {
        Self {
            inner: vec![Box::new(RP2040) as Box<dyn BoardInfo>, Box::new(RP2350)].into_iter(),
        }
    }

    pub fn find_by_name(name: &str) -> Option<Box<dyn BoardInfo>> {
        Self::new()
            .find(|board| board.board_name().eq_ignore_ascii_case(name))
            .map(|v| v as _)
    }
}

impl Iterator for BoardIter {
    type Item = Box<dyn BoardInfo>;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// This is the version of the firmware on the usb device
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct UsbVersion(pub u8, pub u8, pub u8);

/// This is the usb device information from the usb device. It is possible to generate this information with something like
/// nusb
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct UsbDevice {
    pub bus_number: u8,
    pub address: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub version: UsbVersion,
}

#[derive(Debug, Clone, Default)]
pub struct AddressLocations<'a> {
    pub address_ranges_ram: Option<&'a [AddressRange]>,
    pub address_ranges_flash: Option<&'a [AddressRange]>,
    pub main_ram_start: Option<u64>,
    pub main_ram_end: Option<u64>,
    pub xip_sram_start: Option<u64>,
    pub xip_sram_end: Option<u64>,
}

/// This trait helps by allowing for definitions of multiple different boards.
pub trait BoardInfo {
    /// Check if the board is connected to the specified UsbDevice
    fn is_device_board(&self, device: &UsbDevice) -> bool;

    /// Returns the proper family id to use for the uf2 device
    fn family_id(&self) -> u32;

    /// Optional, just sent to a sensible default of 256, as long as it is less than 512 - 32 it should be okay, but boards very, and so does the bootloader firmware
    fn page_size(&self) -> u32 {
        256
    }

    /// Optional, with a default erase size of 4096
    fn flash_sector_erase_size(&self) -> u64 {
        4096
    }

    fn address_locations<'a>(&'a self) -> AddressLocations<'a> {
        AddressLocations::default()
    }

    /// Get the board's name
    fn board_name(&self) -> String;
}
