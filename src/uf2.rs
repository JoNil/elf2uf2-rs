#![allow(dead_code)]

use static_assertions::const_assert;
use std::mem;
use zerocopy::{AsBytes, FromBytes};

pub const UF2_MAGIC_START0: u32 = 0x0A324655;
pub const UF2_MAGIC_START1: u32 = 0x9E5D5157;
pub const UF2_MAGIC_END: u32 = 0x0AB16F30;

pub const UF2_FLAG_NOT_MAIN_FLASH: u32 = 0x00000001;
pub const UF2_FLAG_FILE_CONTAINER: u32 = 0x00001000;
pub const UF2_FLAG_FAMILY_ID_PRESENT: u32 = 0x00002000;
pub const UF2_FLAG_MD5_PRESENT: u32 = 0x00004000;

pub const RP2040_FAMILY_ID: u32 = 0xe48bff56;

#[repr(packed)]
#[derive(AsBytes, FromBytes)]
pub struct Uf2Block {
    // 32 byte header
    pub magic_start0: u32,
    pub magic_start1: u32,
    pub flags: u32,
    pub target_addr: u32,
    pub payload_size: u32,
    pub block_no: u32,
    pub num_blocks: u32,
    pub file_size: u32, // or familyID
    pub data: [u8; 476],
    pub magic_end: u32,
}

const_assert!(mem::size_of::<Uf2Block>() == 512);
