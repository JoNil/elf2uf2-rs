use crate::{
    address_range::{
        FLASH_SECTOR_ERASE_SIZE, MAIN_RAM_END_RP2040, MAIN_RAM_END_RP2350, MAIN_RAM_START_RP2040,
        MAIN_RAM_START_RP2350, RP2040_ADDRESS_RANGES_FLASH, RP2040_ADDRESS_RANGES_RAM,
        RP2350_ADDRESS_RANGES_FLASH, RP2350_ADDRESS_RANGES_RAM, XIP_SRAM_END_RP2040,
        XIP_SRAM_END_RP2350, XIP_SRAM_START_RP2040, XIP_SRAM_START_RP2350,
    },
    elf::{
        is_ram_binary, realize_page, AddressRangesExt, AddressRangesFromElfError, PageMap,
        PAGE_SIZE,
    },
    uf2::{
        Uf2BlockData, Uf2BlockFooter, Uf2BlockHeader, UF2_FLAG_FAMILY_ID_PRESENT, UF2_MAGIC_END,
        UF2_MAGIC_START0, UF2_MAGIC_START1,
    },
};
use std::{
    collections::HashSet,
    io::{Read, Seek, Write},
};

use ::elf::{endian::AnyEndian, ElfStream, ParseError};
use assert_into::AssertInto;
use clap::ValueEnum;
use log::*;
use thiserror::Error;
use zerocopy::IntoBytes;

pub mod address_range;
pub mod elf;
pub mod uf2;

// See https://github.com/microsoft/uf2/blob/master/utils/uf2families.json for list
#[derive(Debug, ValueEnum, Clone, Copy)]
#[repr(u32)]
#[allow(non_camel_case_types)]
pub enum Family {
    /// Raspberry Pi RP2040
    RP2040 = 0xe48bff56,

    /// Raspberry Pi Microcontrollers: Absolute (unpartitioned) download
    RP2XXX_ABSOLUTE = 0xe48bff57,

    /// Raspberry Pi Microcontrollers: Data partition download
    RP2XXX_DATA = 0xe48bff58,

    /// Raspberry Pi RP2350, Secure Arm image
    RP2350_ARM_S = 0xe48bff59,

    /// Raspberry Pi RP2350, RISC-V image
    RP2350_RISCV = 0xe48bff5a,

    /// Raspberry Pi RP2350, Non-secure Arm image
    RP2350_ARM_NS = 0xe48bff5b,
}

impl Default for Family {
    fn default() -> Self {
        Self::RP2040
    }
}

pub fn write_output(
    elf_file: &mut ElfStream<AnyEndian, impl Read + Seek>,
    pages: &PageMap,
    mut output: impl Write,
    family: Family,
) -> Result<(), Elf2Uf2Error> {
    let mut block_header = Uf2BlockHeader {
        magic_start0: UF2_MAGIC_START0,
        magic_start1: UF2_MAGIC_START1,
        flags: UF2_FLAG_FAMILY_ID_PRESENT,
        target_addr: 0,
        payload_size: PAGE_SIZE.assert_into(),
        block_no: 0,
        num_blocks: pages.len().assert_into(),
        file_size: family as u32,
    };

    let mut block_data: Uf2BlockData = [0; 476];

    let block_footer = Uf2BlockFooter {
        magic_end: UF2_MAGIC_END,
    };

    for (page_num, (target_addr, fragments)) in pages.iter().enumerate() {
        block_header.target_addr = (*target_addr).assert_into();
        block_header.block_no = page_num.assert_into();

        debug!(
            "Page {} / {} {:#08x}",
            { block_header.block_no },
            { block_header.num_blocks },
            { block_header.target_addr }
        );

        block_data.iter_mut().for_each(|v| *v = 0);

        realize_page(elf_file, fragments, &mut block_data)
            .map_err(Elf2Uf2Error::FailedToRealizePages)?;

        output
            .write_all(block_header.as_bytes())
            .map_err(Elf2Uf2Error::FailedToWrite)?;
        output
            .write_all(block_data.as_bytes())
            .map_err(Elf2Uf2Error::FailedToWrite)?;
        output
            .write_all(block_footer.as_bytes())
            .map_err(Elf2Uf2Error::FailedToWrite)?;
    }

    Ok(())
}

pub fn open_elf<T: Read + Seek>(input: T) -> Result<ElfStream<AnyEndian, T>, Elf2Uf2Error> {
    ElfStream::<AnyEndian, _>::open_stream(input).map_err(Elf2Uf2Error::FailedToOpenElfFile)
}

#[cfg_attr(not(test), expect(unused))]
fn elf2uf2(
    input: impl Read + Seek,
    output: impl Write,
    family: Family,
) -> Result<(), Elf2Uf2Error> {
    let mut elf = open_elf(input)?;
    let pages = build_page_map(&elf, family)?;
    write_output(&mut elf, &pages, output, family)
}

#[derive(Error, Debug)]
pub enum Elf2Uf2Error {
    #[error("Failed to get address ranges from elf")]
    FailedToGetPagesFromRanges(AddressRangesFromElfError),
    #[error("Failed to open elf file")]
    FailedToOpenElfFile(ParseError),
    #[error("Failed to realize pages for elf file")]
    FailedToRealizePages(ParseError),
    #[error("Failed to write to output")]
    FailedToWrite(std::io::Error),
    #[error("The input file has no memory pages")]
    InputFileNoMemoryPages,
    #[error("B0/B1 Boot ROM does not support direct entry into XIP_SRAM")]
    DirectEntryIntoXipSram,
    #[error("A RAM binary should have an entry point at the beginning: {0:#08x} (not {1:#08x})")]
    RamBinaryEntryPoint(u32, u32),
    #[error("entry point is not in mapped part of file")]
    EntryPointNotMapped,
}

pub fn build_page_map(
    elf: &ElfStream<AnyEndian, impl Read + Seek>,
    family: Family,
) -> Result<PageMap, Elf2Uf2Error> {
    let ram_style = is_ram_binary(elf, family).ok_or(Elf2Uf2Error::EntryPointNotMapped)?;

    if ram_style {
        debug!("Detected RAM binary");
    } else {
        debug!("Detected FLASH binary");
    }

    let (
        address_ranges_ram,
        address_ranges_flash,
        main_ram_start,
        main_ram_end,
        xip_sram_start,
        xip_sram_end,
    ) = match family {
        Family::RP2040 => (
            RP2040_ADDRESS_RANGES_RAM,
            RP2040_ADDRESS_RANGES_FLASH,
            MAIN_RAM_START_RP2040,
            MAIN_RAM_END_RP2040,
            XIP_SRAM_START_RP2040,
            XIP_SRAM_END_RP2040,
        ),
        Family::RP2XXX_ABSOLUTE
        | Family::RP2XXX_DATA
        | Family::RP2350_ARM_S
        | Family::RP2350_RISCV
        | Family::RP2350_ARM_NS => (
            RP2350_ADDRESS_RANGES_RAM,
            RP2350_ADDRESS_RANGES_FLASH,
            MAIN_RAM_START_RP2350,
            MAIN_RAM_END_RP2350,
            XIP_SRAM_START_RP2350,
            XIP_SRAM_END_RP2350,
        ),
    };

    let valid_ranges = if ram_style {
        address_ranges_ram
    } else {
        address_ranges_flash
    };

    let mut pages = valid_ranges
        .check_elf32_ph_entries(elf)
        .map_err(Elf2Uf2Error::FailedToGetPagesFromRanges)?;

    if pages.is_empty() {
        return Err(Elf2Uf2Error::InputFileNoMemoryPages);
    }

    if ram_style {
        let mut expected_ep_main_ram = u32::MAX as u64;
        let mut expected_ep_xip_sram = u32::MAX as u64;

        #[allow(clippy::manual_range_contains)]
        pages.keys().copied().for_each(|addr| {
            if addr >= main_ram_start && addr <= main_ram_end {
                expected_ep_main_ram = expected_ep_main_ram.min(addr) | 0x1;
            } else if addr >= xip_sram_start && addr < xip_sram_end {
                expected_ep_xip_sram = expected_ep_xip_sram.min(addr) | 0x1;
            }
        });

        let expected_ep = if expected_ep_main_ram != u32::MAX as u64 {
            expected_ep_main_ram
        } else {
            expected_ep_xip_sram
        };

        if expected_ep == expected_ep_xip_sram {
            return Err(Elf2Uf2Error::DirectEntryIntoXipSram);
        } else if elf.ehdr.e_entry != expected_ep {
            #[allow(clippy::unnecessary_cast)]
            return Err(Elf2Uf2Error::RamBinaryEntryPoint(
                expected_ep as u32,
                elf.ehdr.e_entry as u32,
            ));
        }
        assert!(0 == (main_ram_start & (PAGE_SIZE - 1)));

        // TODO: check vector table start up
        // currently don't require this as entry point is now at the start, we don't know where reset vector is
    } else {
        // Fill in empty dummy uf2 pages to align the binary to flash sectors (except for the last sector which we don't
        // need to pad, and choose not to to avoid making all SDK UF2s bigger)
        // That workaround is required because the bootrom uses the block number for erase sector calculations:
        // https://github.com/raspberrypi/pico-bootrom/blob/c09c7f08550e8a36fc38dc74f8873b9576de99eb/bootrom/virtual_disk.c#L205

        let touched_sectors: HashSet<u64> = pages
            .keys()
            .map(|addr| addr / FLASH_SECTOR_ERASE_SIZE)
            .collect();

        let last_page_addr = *pages.last_key_value().unwrap().0;
        for sector in touched_sectors {
            let mut page = sector * FLASH_SECTOR_ERASE_SIZE;

            while page < (sector + 1) * FLASH_SECTOR_ERASE_SIZE {
                if page < last_page_addr && !pages.contains_key(&page) {
                    pages.insert(page, Vec::new());
                }
                page += PAGE_SIZE;
            }
        }
    }

    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    pub fn hello_usb() {
        let bytes_in = io::Cursor::new(&include_bytes!("../tests/rp2040/hello_usb.elf")[..]);
        let mut bytes_out = Vec::new();
        elf2uf2(bytes_in, &mut bytes_out, Family::RP2040).unwrap();

        assert_eq!(bytes_out, include_bytes!("../tests/rp2040/hello_usb.uf2"));
    }

    #[test]
    pub fn hello_serial() {
        let bytes_in = io::Cursor::new(&include_bytes!("../tests/rp2040/hello_serial.elf")[..]);
        let mut bytes_out = Vec::new();
        elf2uf2(bytes_in, &mut bytes_out, Family::RP2040).unwrap();

        assert_eq!(
            bytes_out,
            include_bytes!("../tests/rp2040/hello_serial.uf2")
        );
    }
}
