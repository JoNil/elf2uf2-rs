use std::io::{Read, Seek};

use elf::{ElfStream, abi::PT_LOAD, endian::EndianParse};

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

pub fn address_ranges_from_elf<E: EndianParse, S: Read + Seek>(
    file: &ElfStream<E, S>,
) -> Vec<AddressRange> {
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

    ranges
}
