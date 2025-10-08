use crate::address_range::{
    self, AddressRange, AddressRangeType, RP2040_ADDRESS_RANGES_FLASH, RP2040_ADDRESS_RANGES_RAM,
};
use assert_into::AssertInto;
use elf::{ElfStream, abi::PT_LOAD, endian::EndianParse};
use log::debug;
use std::{
    cmp::min,
    collections::BTreeMap,
    io::{Read, Seek, SeekFrom},
};
use thiserror::Error;

pub const LOG2_PAGE_SIZE: u64 = 8;
pub const PAGE_SIZE: u64 = 1 << LOG2_PAGE_SIZE;

// "determine_binary_type"
pub fn is_ram_binary<E: EndianParse, S: Read + Seek>(file: &ElfStream<E, S>) -> Option<bool> {
    let entry = file.ehdr.e_entry;

    for segment in file.segments() {
        if segment.p_type == PT_LOAD && segment.p_memsz > 0 {
            let mapped_size = segment.p_filesz.min(segment.p_memsz);
            if mapped_size > 0 {
                // We back-convert the entrypoint from a VADDR to a PADDR to see if it originates inflash, and if
                // so call THAT a flash binary
                if entry >= segment.p_vaddr && entry < segment.p_vaddr + mapped_size {
                    let effective_entry = entry + segment.p_paddr - segment.p_vaddr;
                    if RP2040_ADDRESS_RANGES_RAM.is_address_initialized(effective_entry) {
                        return Some(true);
                    } else if RP2040_ADDRESS_RANGES_FLASH.is_address_initialized(effective_entry) {
                        return Some(false);
                    }
                }
            }
        }
    }

    None
}

#[derive(Copy, Clone, Debug, Default)]
pub struct PageFragment {
    pub file_offset: u64,
    pub page_offset: u64,
    pub bytes: u64,
}

pub fn realize_page(
    input: &mut (impl Read + Seek),
    fragments: &[PageFragment],
    buf: &mut [u8],
) -> Result<(), std::io::Error> {
    assert!(buf.len() >= PAGE_SIZE.assert_into());

    for frag in fragments {
        assert!(frag.page_offset < PAGE_SIZE && frag.page_offset + frag.bytes <= PAGE_SIZE);

        input.seek(SeekFrom::Start(frag.file_offset.assert_into()))?;

        input.read_exact(
            &mut buf[frag.page_offset.assert_into()..(frag.page_offset + frag.bytes).assert_into()],
        )?;
    }

    Ok(())
}

#[derive(Error, Debug)]
pub enum AddressRangesFromElfError {
    #[error("No segments in ELF")]
    NoSegments,
    #[error("In memory segments overlap")]
    MemorySegmentsOverlap,
    #[error("ELF contains memory contents for uninitialized memory at {0:08x}")]
    MemoryContentsForUninitializedMemory(u64),
    #[error("Memory segment {0:#08x}->{1:#08x} is outside of valid address range for device")]
    MemorySegmentInvalidForDevice(u64, u64),
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
                    return Err(
                        AddressRangesFromElfError::MemoryContentsForUninitializedMemory(addr),
                    );
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
        Err(AddressRangesFromElfError::MemorySegmentInvalidForDevice(
            addr,
            addr + size,
        ))
    }
}

pub fn address_ranges_from_elf<E: EndianParse, S: Read + Seek>(
    file: &ElfStream<E, S>,
) -> Result<Vec<AddressRange>, AddressRangesFromElfError> {
    let segments = file.segments();

    let mut ranges = Vec::new();

    for seg in segments {
        if seg.p_type != PT_LOAD || seg.p_memsz == 0 {
            continue;
        }

        let start = seg.p_paddr;
        let end = start + seg.p_memsz;

        if seg.p_filesz > 0 {
            // initialized contents
            ranges.push(AddressRange::new(
                start,
                start + seg.p_filesz,
                AddressRangeType::Contents,
            ));
        }

        if seg.p_memsz > seg.p_filesz {
            // uninitialized (BSS)
            ranges.push(AddressRange::new(
                start + seg.p_filesz,
                end,
                AddressRangeType::NoContents,
            ));
        }
    }

    Ok(ranges)
}

pub fn get_page_fragments<E: EndianParse, S: Read + Seek>(
    file: &ElfStream<E, S>,
    ranges: &[AddressRange],
    page_size: u64,
) -> Result<BTreeMap<u64, Vec<PageFragment>>, AddressRangesFromElfError> {
    let mut pages = BTreeMap::<u64, Vec<PageFragment>>::new();

    for segment in file.segments() {
        if segment.p_type != PT_LOAD || segment.p_memsz == 0 {
            continue;
        }
        let mapped_size = min(segment.p_filesz, segment.p_memsz);

        if mapped_size > 0 {
            let ar =
                ranges.check_address_range(segment.p_paddr, segment.p_vaddr, mapped_size, false)?;

            // we don't download uninitialized, generally it is BSS and should be zero-ed by crt0.S, or it may be COPY areas which are undefined
            if ar.typ != address_range::AddressRangeType::Contents {
                debug!("ignored");
                continue;
            }
            let mut addr = segment.p_paddr;
            let mut remaining = mapped_size;
            let mut file_offset = segment.p_offset;
            while remaining > 0 {
                let off = addr & (page_size - 1);
                let len = min(remaining, page_size - off);

                // list of fragments
                let fragments = pages.entry(addr - off).or_default();

                // note if filesz is zero, we want zero init which is handled because the
                // statement above creates an empty page fragment list
                // check overlap with any existing fragments
                for fragment in fragments.iter() {
                    if (off < fragment.page_offset + fragment.bytes)
                        != ((off + len) <= fragment.page_offset)
                    {
                        return Err(AddressRangesFromElfError::MemorySegmentsOverlap);
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

    Ok(pages)
}

impl<'a, T> AddressRangesExt<'a> for T where T: IntoIterator<Item = &'a AddressRange> + Clone {}
