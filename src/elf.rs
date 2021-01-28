use assert_into::AssertInto;
use std::{collections::HashMap, error::Error, io::Read, mem};
use zerocopy::{AsBytes, FromBytes};

use crate::{address_range::AddressRange, page_fragment::PageFragment};

const ELF_MAGIC: u32 = 0x464c457f;
const EM_ARM: u16 = 0x28;
const EF_ARM_ABI_FLOAT_HARD: u32 = 0x00000400;
const PT_LOAD: u32 = 0x00000001;

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
#[derive(AsBytes, Copy, Clone, Default, Debug)]
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

pub fn read_and_check_elf32_header(input: &mut impl Read) -> Result<Elf32Header, Box<dyn Error>> {
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

pub fn read_and_check_elf32_ph_entries(
    input: &mut impl Read,
    eh: &Elf32Header,
    valid_ranges: &[AddressRange],
) -> Result<HashMap<u32, Vec<PageFragment>>, Box<dyn Error>> {
    let res = HashMap::new();

    Ok(res)
}

/*int read_and_check_elf32_ph_entries(FILE *in, const elf32_header &eh, const address_ranges& valid_ranges, std::map<uint32_t, std::vector<page_fragment>>& pages) {
    if (eh.ph_entry_size != sizeof(elf32_ph_entry)) {
        return fail(ERROR_FORMAT, "Invalid ELF32 program header");
    }
    if (eh.ph_num) {
        std::vector<elf32_ph_entry> entries(eh.ph_num);
        if (eh.ph_num != fread(&entries[0], sizeof(struct elf32_ph_entry), eh.ph_num, in)) {
            return fail_read_error();
        }
        for(uint i=0;i<eh.ph_num;i++) {
            elf32_ph_entry& entry = entries[i];
            if (entry.type == PT_LOAD && entry.memsz) {
                address_range ar;
                int rc;
                uint mapped_size = std::min(entry.filez, entry.memsz);
                if (mapped_size) {
                    rc = check_address_range(valid_ranges, entry.paddr, entry.vaddr, mapped_size, false, ar);
                    if (rc) return rc;
                    // we don't download uninitialized, generally it is BSS and should be zero-ed by crt0.S, or it may be COPY areas which are undefined
                    if (ar.type != address_range::type::CONTENTS) {
                        if (verbose) printf("  ignored\n");
                        continue;
                    }
                    uint addr = entry.paddr;
                    uint remaining = mapped_size;
                    uint file_offset = entry.offset;
                    while (remaining) {
                        uint off = addr & (PAGE_SIZE - 1);
                        uint len = std::min(remaining, PAGE_SIZE - off);
                        auto &fragments = pages[addr - off]; // list of fragments
                        // note if filesz is zero, we want zero init which is handled because the
                        // statement above creates an empty page fragment list
                        // check overlap with any existing fragments
                        for (const auto &fragment : fragments) {
                            if ((off < fragment.page_offset + fragment.bytes) !=
                                ((off + len) <= fragment.page_offset)) {
                                fail(ERROR_FORMAT, "In memory segments overlap");
                            }
                        }
                        fragments.push_back(
                                page_fragment{file_offset,off,len});
                        addr += len;
                        file_offset += len;
                        remaining -= len;
                    }
                }
                if (entry.memsz > entry.filez) {
                    // we have some uninitialized data too
                    rc = check_address_range(valid_ranges, entry.paddr + entry.filez, entry.vaddr + entry.filez, entry.memsz - entry.filez, true,
                                             ar);
                    if (rc) return rc;
                }
            }
        }
    }
    return 0;
}*/
