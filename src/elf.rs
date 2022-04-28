use crate::{
    address_range::{self, AddressRange},
    Opts,
};
use assert_into::AssertInto;
use std::{
    cmp::min,
    collections::BTreeMap,
    error::Error,
    io::{Read, Seek, SeekFrom},
    mem,
};
use zerocopy::{AsBytes, FromBytes};

const ELF_MAGIC: u32 = 0x464c457f;
const EM_ARM: u16 = 0x28;
const EF_ARM_ABI_FLOAT_HARD: u32 = 0x00000400;
const PT_LOAD: u32 = 0x00000001;

pub const LOG2_PAGE_SIZE: u32 = 8;
pub const PAGE_SIZE: u32 = 1 << LOG2_PAGE_SIZE;

#[repr(packed)]
#[derive(AsBytes, Copy, Clone, Default, Debug, FromBytes)]
pub struct ElfHeader {
    pub magic: u32,
    pub arch_class: u8,
    pub endianness: u8,
    pub version: u8,
    pub abi: u8,
    pub abi_version: u8,
    pub pad: [u8; 7],
    pub typ: u16,
    pub machine: u16,
    pub version2: u32,
}

#[repr(packed)]
#[derive(AsBytes, Copy, Clone, Default, Debug, FromBytes)]
pub struct Elf32Header {
    pub common: ElfHeader,
    pub entry: u32,
    pub ph_offset: u32,
    pub sh_offset: u32,
    pub flags: u32,
    pub eh_size: u16,
    pub ph_entry_size: u16,
    pub ph_num: u16,
    pub sh_entry_size: u16,
    pub sh_num: u16,
    pub sh_str_index: u16,
}

#[repr(packed)]
#[derive(AsBytes, Copy, Clone, Default, Debug, FromBytes)]
pub struct Elf32PhEntry {
    pub typ: u32,
    pub offset: u32,
    pub vaddr: u32,
    pub paddr: u32,
    pub filez: u32,
    pub memsz: u32,
    pub flags: u32,
    pub align: u32,
}

pub(crate) fn read_and_check_elf32_header(
    input: &mut impl Read,
) -> Result<Elf32Header, Box<dyn Error>> {
    let mut eh = Elf32Header::default();

    input.read_exact(eh.as_bytes_mut())?;

    if eh.common.magic != ELF_MAGIC {
        return Err("Not an ELF file".into());
    }
    if eh.common.version != 1 || eh.common.version2 != 1 {
        return Err("Unrecognized ELF version".into());
    }
    if eh.common.arch_class != 1 || eh.common.endianness != 1 {
        return Err("Require 32 bit little-endian ELF".into());
    }
    if eh.eh_size != mem::size_of::<Elf32Header>().assert_into() {
        return Err("Invalid ELF32 format".into());
    }
    if eh.common.machine != EM_ARM {
        return Err("Not an ARM executable".into());
    }
    if eh.common.abi != 0 {
        return Err("Unrecognized ABI".into());
    }
    if eh.flags & EF_ARM_ABI_FLOAT_HARD > 0 {
        return Err("HARD-FLOAT not supported".into());
    }

    Ok(eh)
}

fn check_address_range(
    valid_ranges: &[AddressRange],
    addr: u32,
    vaddr: u32,
    size: u32,
    uninitialized: bool,
) -> Result<AddressRange, Box<dyn Error>> {
    for range in valid_ranges {
        if range.from <= addr && range.to >= addr + size {
            if range.typ == address_range::AddressRangeType::NoContents && !uninitialized {
                return Err("ELF contains memory contents for uninitialized memory".into());
            }
            if Opts::global().verbose {
                println!(
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
            }
            return Ok(*range);
        }
    }
    Err(format!(
        "Memory segment {:#08x}->{:#08x} is outside of valid address range for device",
        addr,
        addr + size
    )
    .into())
}

#[derive(Copy, Clone, Debug, Default)]
pub struct PageFragment {
    pub file_offset: u32,
    pub page_offset: u32,
    pub bytes: u32,
}

pub(crate) fn read_and_check_elf32_ph_entries(
    input: &mut impl Read,
    eh: &Elf32Header,
    valid_ranges: &[AddressRange],
) -> Result<BTreeMap<u32, Vec<PageFragment>>, Box<dyn Error>> {
    let mut pages = BTreeMap::<u32, Vec<PageFragment>>::new();

    if eh.ph_entry_size != mem::size_of::<Elf32PhEntry>().assert_into() {
        return Err("Invalid ELF32 program header".into());
    }

    if eh.ph_num > 0 {
        let mut entries = Vec::<Elf32PhEntry>::new();
        entries.resize_with(eh.ph_num.assert_into(), Default::default);

        input.read_exact(entries.as_mut_slice().as_bytes_mut())?;

        for entry in &entries {
            if entry.typ == PT_LOAD && entry.memsz > 0 {
                let mapped_size = min(entry.filez, entry.memsz);

                if mapped_size > 0 {
                    let ar = check_address_range(
                        valid_ranges,
                        entry.paddr,
                        entry.vaddr,
                        mapped_size,
                        false,
                    )?;

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
                        let off = addr & (PAGE_SIZE - 1);
                        let len = min(remaining, PAGE_SIZE - off);

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
                        check_address_range(
                            valid_ranges,
                            entry.paddr + entry.filez,
                            entry.vaddr + entry.filez,
                            entry.memsz - entry.filez,
                            true,
                        )?;
                    }
                }
            }
        }
    }

    Ok(pages)
}

pub fn realize_page(
    input: &mut (impl Read + Seek),
    fragments: &[PageFragment],
    buf: &mut [u8],
) -> Result<(), Box<dyn Error>> {
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
