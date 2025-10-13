use crate::{
    address_range::{self, AddressRange},
    Opts,
};
use assert_into::AssertInto;
use std::{error::Error, io::Read, mem};
use zerocopy::{FromBytes, IntoBytes};

const ELF_MAGIC: u32 = 0x464c457f;
const PT_LOAD: u32 = 0x00000001;

/// Filter the entries in `entries` to only those that must be loaded
/// (have the `PT_LOAD` type) and are non-empty.
pub fn loadable_nonempty(entries: &[Elf32PhEntry]) -> impl Iterator<Item = &Elf32PhEntry> {
    entries.iter().filter(|e| {
        let mapped_size = e.filez.min(e.memsz);
        e.typ == PT_LOAD && mapped_size > 0
    })
}

#[allow(unused)]
#[repr(C, packed)]
#[derive(IntoBytes, Copy, Clone, Default, Debug, FromBytes)]
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

#[allow(unused)]
#[repr(C, packed)]
#[derive(IntoBytes, Copy, Clone, Default, Debug, FromBytes)]
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

impl Elf32Header {
    // read_and_check_elf32_header
    pub(crate) fn from_read(input: &mut impl Read) -> Result<Self, Box<dyn Error>> {
        let mut eh = Elf32Header::default();

        input.read_exact(eh.as_mut_bytes())?;

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
        if eh.common.abi != 0 && eh.common.abi != 3 {
            return Err(format!("Unrecognized ABI {}", eh.common.abi).into());
        }

        Ok(eh)
    }

    pub(crate) fn read_elf32_ph_entries(
        &self,
        input: &mut impl Read,
    ) -> Result<Vec<Elf32PhEntry>, Box<dyn Error>> {
        if self.ph_entry_size != mem::size_of::<Elf32PhEntry>().assert_into() {
            return Err("Invalid ELF32 program header".into());
        }

        let mut entries: Vec<Elf32PhEntry> = (0..self.ph_num).map(|_| Default::default()).collect();
        input.read_exact(entries.as_mut_slice().as_mut_bytes())?;

        Ok(entries)
    }
}

#[allow(unused)]
#[repr(C, packed)]
#[derive(IntoBytes, Copy, Clone, Default, Debug, FromBytes)]
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

#[derive(Copy, Clone, Debug, Default)]
pub struct PageFragment {
    pub file_offset: u32,
    pub page_offset: u32,
    pub bytes: u32,
}

pub trait AddressRangesExt<'a>: IntoIterator<Item = &'a AddressRange> + Clone {
    fn range_for(&self, addr: u32) -> Option<&'a AddressRange> {
        self.clone()
            .into_iter()
            .find(|r| r.from <= addr && r.to > addr)
    }

    fn is_address_initialized(&self, addr: u32) -> bool {
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
        addr: u32,
        vaddr: u32,
        size: u32,
        uninitialized: bool,
    ) -> Result<AddressRange, Box<dyn Error>> {
        for range in self.clone().into_iter() {
            if range.from <= addr && range.to >= addr + size {
                if range.typ == address_range::AddressRangeType::NoContents && !uninitialized {
                    return Err(format!(
                        "ELF contains memory contents for uninitialized memory at {addr:08x}"
                    )
                    .into());
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
}

impl<'a, T> AddressRangesExt<'a> for T where T: IntoIterator<Item = &'a AddressRange> + Clone {}
