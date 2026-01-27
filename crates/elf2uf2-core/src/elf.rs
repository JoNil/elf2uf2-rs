use crate::{
    Elf2Uf2Error,
    address_range::{self, AddressRange, address_ranges_from_elf},
    boards::{AddressLocations, BoardInfo},
};
use assert_into::AssertInto;
use elf::{ElfStream, ParseError, abi::PT_LOAD, endian::EndianParse, segment::ProgramHeader};
use log::debug;
use std::{
    cmp::min,
    collections::BTreeMap,
    io::{Read, Seek},
};
use thiserror::Error;

pub const LOG2_PAGE_SIZE: u64 = 8;
pub const PAGE_SIZE: u64 = 1 << LOG2_PAGE_SIZE;

pub type PageMap = BTreeMap<u64, Vec<PageFragment>>;

// "determine_binary_type"
pub fn is_ram_binary<E: EndianParse, S: Read + Seek>(
    file: &ElfStream<E, S>,
    board: &dyn BoardInfo,
) -> Result<Option<bool>, Elf2Uf2Error> {
    let entry = file.ehdr.e_entry;

    let (address_ranges_ram, address_ranges_flash) = match board.address_locations() {
        AddressLocations {
            address_ranges_ram: Some(address_ranges_ram),
            address_ranges_flash: Some(address_ranges_flash),
            main_ram_start: _,
            main_ram_end: _,
            xip_sram_start: _,
            xip_sram_end: _,
        } => (address_ranges_ram, address_ranges_flash),
        AddressLocations {
            address_ranges_ram: _,
            address_ranges_flash: _,
            main_ram_start: _,
            main_ram_end: _,
            xip_sram_start: _,
            xip_sram_end: _,
        } => return Ok(None),
    };

    for segment in file.segments() {
        if segment.p_type == PT_LOAD && segment.p_memsz > 0 {
            let mapped_size = segment.p_filesz.min(segment.p_memsz);
            if mapped_size > 0 {
                // We back-convert the entrypoint from a VADDR to a PADDR to see if it originates inflash, and if
                // so call THAT a flash binary
                if entry >= segment.p_vaddr && entry < segment.p_vaddr + mapped_size {
                    let effective_entry = entry + segment.p_paddr - segment.p_vaddr;
                    if address_ranges_ram.is_address_initialized(effective_entry) {
                        return Ok(Some(true));
                    } else if address_ranges_flash.is_address_initialized(effective_entry) {
                        return Ok(Some(false));
                    }
                }
            }
        }
    }

    Err(Elf2Uf2Error::EntryPointNotMapped)
}

#[derive(Copy, Clone, Debug)]
pub struct PageFragment {
    pub segment: ProgramHeader,
    pub file_offset: u64,
    pub page_offset: u64,
    pub bytes: u64,
}

pub fn realize_page<E: EndianParse, S: Read + Seek>(
    file: &mut ElfStream<E, S>,
    fragments: &[PageFragment],
    buf: &mut [u8],
) -> Result<(), ParseError> {
    assert!(buf.len() >= PAGE_SIZE.assert_into());

    for frag in fragments {
        let data = file.segment_data(&frag.segment)?;
        assert!(frag.page_offset < PAGE_SIZE && frag.page_offset + frag.bytes <= PAGE_SIZE);

        let start = (frag.file_offset - frag.segment.p_offset) as usize;
        let end = start + frag.bytes as usize;

        buf[frag.page_offset.assert_into()..(frag.page_offset + frag.bytes).assert_into()]
            .copy_from_slice(&data[start..end]);
    }

    Ok(())
}

#[derive(Error, Debug)]
pub enum AddressRangesFromElfError {
    #[error("No segments in ELF")]
    NoSegments,
    #[error("In-memory segments overlap")]
    SegmentsOverlap,
    #[error("ELF contains memory contents for uninitialized memory at {0:08x}")]
    ContentsForUninitializedMemory(u64),
    #[error("Memory segment {0:#08x}->{1:#08x} is outside of valid address range for device")]
    SegmentInvalidForDevice(u64, u64),
}

pub trait AddressRangesExt<'a>: IntoIterator<Item = &'a AddressRange> + Clone {
    fn range_for(&self, addr: u64) -> Option<&'a AddressRange> {
        self.clone()
            .into_iter()
            .find(|r| r.from <= addr && r.to > addr)
    }

    fn is_address_initialized(&self, addr: u64) -> bool {
        let range = if let Some(range) = self.range_for(addr) {
            range
        } else {
            return false;
        };

        matches!(range.typ, address_range::AddressRangeType::Contents)
    }

    // "check_address_range"
    fn check_address_range(
        &self,
        addr: u64,
        vaddr: u64,
        size: u64,
        uninitialized: bool,
    ) -> Result<AddressRange, AddressRangesFromElfError> {
        for range in self.clone().into_iter() {
            if range.from <= addr && range.to >= addr + size {
                if range.typ == address_range::AddressRangeType::NoContents && !uninitialized {
                    return Err(AddressRangesFromElfError::ContentsForUninitializedMemory(
                        addr,
                    ));
                }
                debug!(
                    "{} segment {:#08x}->{:#08x} ({:#08x}->{:#08x})",
                    if uninitialized {
                        "Uninitialized"
                    } else {
                        "Mapped"
                    },
                    addr,
                    addr + size,
                    vaddr,
                    vaddr + size
                );
                return Ok(*range);
            }
        }
        Err(AddressRangesFromElfError::SegmentInvalidForDevice(
            addr,
            addr + size,
        ))
    }
}

pub fn get_page_fragments<E: EndianParse, S: Read + Seek>(
    file: &ElfStream<E, S>,
    page_size: u32,
    address_range: Option<&[AddressRange]>,
) -> Result<PageMap, AddressRangesFromElfError> {
    let mut pages = PageMap::new();

    let elf_ranges = address_ranges_from_elf(file);

    // We fallback to these ranges if address_range is not provided
    let ranges = match address_range {
        Some(range) => range,
        None => elf_ranges.as_slice(),
    };

    for segment in file.segments() {
        if segment.p_type == PT_LOAD && segment.p_memsz > 0 {
            let mapped_size = min(segment.p_filesz, segment.p_memsz);

            if mapped_size > 0 {
                let ar = ranges.check_address_range(
                    segment.p_paddr,
                    segment.p_vaddr,
                    mapped_size,
                    false,
                )?;

                // we don't download uninitialized, generally it is BSS and should be zero-ed by crt0.S, or it may be COPY areas which are undefined
                if ar.typ != address_range::AddressRangeType::Contents {
                    debug!("ignored");
                    continue;
                }
                let mut addr = segment.p_paddr;
                let mut remaining = mapped_size;
                let mut file_offset = segment.p_offset;
                while remaining > 0 {
                    let off = addr & (page_size - 1) as u64;
                    let len = min(remaining, page_size as u64 - off);

                    // list of fragments
                    let fragments = pages.entry(addr - off).or_default();

                    // note if filesz is zero, we want zero init which is handled because the
                    // statement above creates an empty page fragment list
                    // check overlap with any existing fragments
                    for fragment in fragments.iter() {
                        if (off < fragment.page_offset + fragment.bytes)
                            != ((off + len) <= fragment.page_offset)
                        {
                            return Err(AddressRangesFromElfError::SegmentsOverlap);
                        }
                    }
                    fragments.push(PageFragment {
                        segment: *segment,
                        file_offset,
                        page_offset: off,
                        bytes: len,
                    });
                    addr += len;
                    file_offset += len;
                    remaining -= len;
                }
                if segment.p_memsz > segment.p_filesz {
                    // we have some uninitialized data too
                    ranges.check_address_range(
                        segment.p_paddr + segment.p_filesz,
                        segment.p_vaddr + segment.p_filesz,
                        segment.p_memsz - segment.p_filesz,
                        true,
                    )?;
                }
            }
        }
    }

    Ok(pages)
}

impl<'a, T> AddressRangesExt<'a> for T where T: IntoIterator<Item = &'a AddressRange> + Clone {}
