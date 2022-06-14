//! This is the backend library logic for elf2uf2-rs.
//! If you wish to use it within your own tools,
//! I recommend specifying "default-features = false"
//! in your Cargo.toml.
//!
//! See the documentation of elf2uf2 for more details.

use address_range::AddressRange;
use assert_into::AssertInto;
use elf::{read_and_check_elf32_ph_entries, realize_page};
#[cfg(feature = "progress_bar")]
use pbr::{ProgressBar, Units};
use std::{
    error::Error,
    io::{Read, Seek, Write},
};
use uf2::{
    Uf2BlockData, Uf2BlockFooter, Uf2BlockHeader, UF2_FLAG_FAMILY_ID_PRESENT, UF2_MAGIC_END,
    UF2_MAGIC_START0, UF2_MAGIC_START1,
};
use zerocopy::AsBytes;

// Consuming code needs the AddressRange type, and potentially some of the definitions.
pub mod address_range;
mod elf;
mod uf2;
// These are (potentially) needed by library consuming code, let's export them, w/o including the whole modules
pub use elf::{Elf32Header, PAGE_SIZE};
pub use uf2::RP2040_FAMILY_ID;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// Set the level of verbosity of the elf2uf2 process
pub enum Verbosity {
    /// Print almost nothing to the console
    Quiet,
    /// Print a progress bar to the console
    #[cfg(feature = "progress_bar")]
    Progress,
    /// Verbose mode, print everything, including page offsets
    Verbose,
}

/// Convert the ELF file that backs the Read+Seek object provided into a UF2 bootloader file on the output Writer.
///
/// Currently only supports Little-Endian ARM32 objects as input, that do not use hard-float.
/// Additionally, to use this, a few family speific options must be provided:
/// - You must specify the family ID that shall be written to the UF2 file.
///   We provide a definition for the Raspberry Pi RP2040, but other values are allowed.
/// - A function that given the ELF Header, returns the valid address ranges that should be allowed.
///   This could be used to support different types of memory spaces to be programmed.
///   Again, we provide definitions for the RP2040 here.
/// - A function that does any extra validation of the entry point, if needed.
///
/// # Examples
/// ```
/// crate::elf2uf2(
///     std::fs::File::open(...).unwrap(),
///     std::fs::File::create(...).unwrap(),
///     Verbosity::Quiet,
///     crate::RP2040_FAMILY_ID,
///     |eh| {
///         let ram_style = 0x2 == eh.entry >> 28;
///         if ram_style {
///             crate::address_range::RP2040_ADDRESS_RANGES_RAM
///         } else {
///             crate::address_range::RP2040_ADDRESS_RANGES_FLASH
///         }
///     },
///     |eh, expected_ep| {
///         let ram_style = 0x2 == eh.entry >> 28;
///         if ram_style {
///             if eh.entry != expected_ep {
///                 return Err(format!(
///                 "A RAM binary should have an entry point at the beginning: {:#08x} (not {:#08x})\n",
///                 expected_ep, eh.entry as u32
///             )
///             .into());
///             }
///         }
///         Ok(())
///     },
/// );
/// ```
pub fn elf2uf2(
    mut input: impl Read + Seek,
    mut output: impl Write,
    verbosity: Verbosity,
    family_id: u32,
    validate_address_ranges: impl FnOnce(&elf::Elf32Header) -> &'static [AddressRange],
    validate_entry_point: impl FnOnce(&elf::Elf32Header, u32) -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let eh = elf::read_and_check_elf32_header(&mut input)?;

    let valid_ranges = validate_address_ranges(&eh);

    let pages = read_and_check_elf32_ph_entries(
        &mut input,
        &eh,
        valid_ranges,
        verbosity == Verbosity::Verbose,
    )?;

    if pages.is_empty() {
        return Err("The input file has no memory pages".into());
    }
    {
        let expected_ep = pages.keys().next().unwrap() | 0x1;
        validate_entry_point(&eh, expected_ep)?;
    }

    let mut block_header = Uf2BlockHeader {
        magic_start0: UF2_MAGIC_START0,
        magic_start1: UF2_MAGIC_START1,
        flags: UF2_FLAG_FAMILY_ID_PRESENT,
        target_addr: 0,
        payload_size: PAGE_SIZE,
        block_no: 0,
        num_blocks: pages.len().assert_into(),
        file_size: family_id,
    };

    let mut block_data: Uf2BlockData = [0; 476];

    let block_footer = Uf2BlockFooter {
        magic_end: UF2_MAGIC_END,
    };

    #[cfg(feature = "progress_bar")]
    let mut pb = if verbosity == Verbosity::Progress {
        Some(ProgressBar::new((pages.len() * 512).assert_into()))
    } else {
        None
    };
    #[cfg(feature = "progress_bar")]
    if let Some(pb) = &mut pb {
        pb.set_units(Units::Bytes);
    }

    let last_page_num = pages.len() - 1;

    for (page_num, (target_addr, fragments)) in pages.into_iter().enumerate() {
        block_header.target_addr = target_addr;
        block_header.block_no = page_num.assert_into();

        if verbosity == Verbosity::Verbose {
            println!(
                "Page {} / {} {:#08x}",
                block_header.block_no as u32,
                block_header.num_blocks as u32,
                block_header.target_addr as u32
            );
        }

        block_data.iter_mut().for_each(|v| *v = 0);

        realize_page(&mut input, &fragments, &mut block_data)?;

        output.write_all(block_header.as_bytes())?;
        output.write_all(block_data.as_bytes())?;
        output.write_all(block_footer.as_bytes())?;
        output.flush()?;

        if page_num != last_page_num {
            #[cfg(feature = "progress_bar")]
            if let Some(pb) = &mut pb {
                pb.add(512);
            }
        }
    }

    // Drop the output before the progress bar is allowd to finish
    drop(output);
    #[cfg(feature = "progress_bar")]
    if let Some(pb) = &mut pb {
        pb.add(512);
    }

    Ok(())
}
