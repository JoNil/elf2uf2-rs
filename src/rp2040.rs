use std::{
    collections::{BTreeMap, HashSet},
    error::Error,
    io::{Read, Seek, SeekFrom},
};

use assert_into::AssertInto;
use static_assertions::const_assert;

use crate::{
    address_range::{self, AddressRange, AddressRangeType},
    elf::{loadable_nonempty, AddressRangesExt, Elf32Header, Elf32PhEntry, PageFragment},
    Opts,
};

pub type FragmentMap = BTreeMap<u32, Vec<PageFragment>>;

pub const PAGE_SIZE: u32 = 256;
pub const FAMILY_ID: u32 = 0xe48bff56;

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

const ADDRESS_RANGES_FLASH: &[AddressRange] = &[
    AddressRange::new(FLASH_START, FLASH_END, AddressRangeType::Contents),
    AddressRange::new(MAIN_RAM_START, MAIN_RAM_END, AddressRangeType::NoContents),
    AddressRange::new(
        MAIN_RAM_BANKED_START,
        MAIN_RAM_BANKED_END,
        AddressRangeType::NoContents,
    ),
];

const ADDRESS_RANGES_RAM: &[AddressRange] = &[
    AddressRange::new(MAIN_RAM_START, MAIN_RAM_END, AddressRangeType::Contents),
    AddressRange::new(XIP_SRAM_START, XIP_SRAM_END, AddressRangeType::Contents),
    AddressRange::new(ROM_START, ROM_END, AddressRangeType::Ignore), // for now we ignore the bootrom if present
];

/// Given `eh` and `entries`, describing an ELF that contains a RP2040 binary,
/// generate a [FragmentMap] that can be used to realize UF2 blocks for the
/// RP2040.
pub fn generate_fragment_map(
    eh: &Elf32Header,
    entries: &[Elf32PhEntry],
) -> Result<FragmentMap, Box<dyn Error>> {
    let ram_style = is_ram_binary(eh, entries)
        .ok_or("entry point is not in mapped part of file".to_string())?;

    if Opts::global().verbose {
        if ram_style {
            println!("Detected RAM binary");
        } else {
            println!("Detected FLASH binary");
        }
    }

    let valid_ranges = if ram_style {
        ADDRESS_RANGES_RAM
    } else {
        ADDRESS_RANGES_FLASH
    };

    let mut pages = check_elf32_ph_entries(valid_ranges, entries)?;

    if pages.is_empty() {
        return Err("The input file has no memory pages".into());
    }

    if ram_style {
        let mut expected_ep_main_ram = u32::MAX;
        let mut expected_ep_xip_sram = u32::MAX;

        #[allow(clippy::manual_range_contains)]
        pages.keys().copied().for_each(|addr| {
            if addr >= MAIN_RAM_START && addr <= MAIN_RAM_END {
                expected_ep_main_ram = expected_ep_main_ram.min(addr) | 0x1;
            } else if addr >= XIP_SRAM_START && addr < XIP_SRAM_END {
                expected_ep_xip_sram = expected_ep_xip_sram.min(addr) | 0x1;
            }
        });

        let expected_ep = if expected_ep_main_ram != u32::MAX {
            expected_ep_main_ram
        } else {
            expected_ep_xip_sram
        };

        if expected_ep == expected_ep_xip_sram {
            return Err("B0/B1 Boot ROM does not support direct entry into XIP_SRAM".into());
        } else if eh.entry != expected_ep {
            #[allow(clippy::unnecessary_cast)]
            return Err(format!(
                "A RAM binary should have an entry point at the beginning: {:#08x} (not {:#08x})",
                expected_ep, eh.entry as u32
            )
            .into());
        }
        const_assert!(MAIN_RAM_START.is_multiple_of(PAGE_SIZE));

        // TODO: check vector table start up
        // currently don't require this as entry point is now at the start, we don't know where reset vector is
    } else {
        // Fill in empty dummy uf2 pages to align the binary to flash sectors (except for the last sector which we don't
        // need to pad, and choose not to to avoid making all SDK UF2s bigger)
        // That workaround is required because the bootrom uses the block number for erase sector calculations:
        // https://github.com/raspberrypi/pico-bootrom/blob/c09c7f08550e8a36fc38dc74f8873b9576de99eb/bootrom/virtual_disk.c#L205

        let touched_sectors: HashSet<u32> = pages
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

fn is_ram_binary(header: &Elf32Header, entries: &[Elf32PhEntry]) -> Option<bool> {
    for entry in loadable_nonempty(entries) {
        let mapped_size = entry.filez.min(entry.memsz);
        if header.entry >= entry.vaddr && header.entry < entry.vaddr + mapped_size {
            // We back-convert the entrypoint from a VADDR to a PADDR to see if it originates inflash, and if
            // so call THAT a flash binary
            let effective_entry = header.entry + entry.paddr - entry.vaddr;
            if ADDRESS_RANGES_RAM.is_address_initialized(effective_entry) {
                return Some(true);
            } else if ADDRESS_RANGES_FLASH.is_address_initialized(effective_entry) {
                return Some(false);
            }
        }
    }

    None
}

/// Write the raw binary data for the page described by `fragments`
/// to `buf` by reading data from `input`.
///
/// The output buffer `buf` will be the data for a UF2 block with a [Uf2BlockHeader::payload_size][0]
/// of `PAGE_SIZE` bytes.
///
/// # Panics
/// This function panics of `buf.len() < PAGE_SIZE`, or if any of
/// the [PageFragment]s in `fragments` are outside of the bounds of
/// a single page (i.e. `frag.page_offset >= PAGE_SIZE || frag.page_offset + frage.bytes > PAGE_SIZE`)
///
/// [0]: crate::uf2::Uf2BlockHeader::payload_size
pub fn realize_page(
    input: &mut (impl Read + Seek),
    fragments: &[PageFragment],
    buf: &mut [u8; 476],
) -> Result<(), Box<dyn Error>> {
    for frag in fragments {
        assert!(frag.page_offset < PAGE_SIZE && frag.page_offset + frag.bytes <= PAGE_SIZE);

        input.seek(SeekFrom::Start(frag.file_offset.assert_into()))?;

        input.read_exact(
            &mut buf[frag.page_offset.assert_into()..(frag.page_offset + frag.bytes).assert_into()],
        )?;
    }

    Ok(())
}

fn check_elf32_ph_entries(
    ranges: &[AddressRange],
    entries: &[Elf32PhEntry],
) -> Result<FragmentMap, Box<dyn Error>> {
    let mut pages = FragmentMap::new();

    for entry in loadable_nonempty(entries) {
        let mapped_size = entry.filez.min(entry.memsz);
        let ar = ranges.check_address_range(entry.paddr, entry.vaddr, mapped_size, false)?;

        // we don't download uninitialized, generally it is BSS and should be zero-ed by crt0.S, or it may be COPY areas which are undefined
        if ar.typ != address_range::AddressRangeType::Contents {
            if Opts::global().verbose {
                println!("ignored");
            }
            continue;
        }
        let mut addr = entry.paddr;
        let mut remaining = mapped_size;
        let mut file_offset = entry.offset;
        while remaining > 0 {
            let off = addr % PAGE_SIZE;
            let len = remaining.min(PAGE_SIZE - off);

            // list of fragments
            let fragments = pages.entry(addr - off).or_default();

            // note if filesz is zero, we want zero init which is handled because the
            // statement above creates an empty page fragment list
            // check overlap with any existing fragments
            for fragment in fragments.iter() {
                if (off < fragment.page_offset + fragment.bytes)
                    != ((off + len) <= fragment.page_offset)
                {
                    return Err("In memory segments overlap".into());
                }
            }
            fragments.push(PageFragment {
                file_offset,
                page_offset: off,
                bytes: len,
            });
            addr += len;
            file_offset += len;
            remaining -= len;
        }
        if entry.memsz > entry.filez {
            // we have some uninitialized data too
            ranges.check_address_range(
                entry.paddr + entry.filez,
                entry.vaddr + entry.filez,
                entry.memsz - entry.filez,
                true,
            )?;
        }
    }

    Ok(pages)
}
