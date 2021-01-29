use address_range::{MAIN_RAM_START, RP2040_ADDRESS_RANGES_FLASH, RP2040_ADDRESS_RANGES_RAM};
use assert_into::AssertInto;
use clap::Clap;
use elf::{read_and_check_elf32_ph_entries, realize_page, PAGE_SIZE};
use static_assertions::const_assert;
use std::{
    error::Error,
    fs::{self, File},
    io::{BufReader, Read, Seek, Write},
    path::{Path, PathBuf},
};
use uf2::{
    Uf2Block, RP2040_FAMILY_ID, UF2_FLAG_FAMILY_ID_PRESENT, UF2_MAGIC_END, UF2_MAGIC_START0,
    UF2_MAGIC_START1,
};
use zerocopy::AsBytes;

mod address_range;
mod elf;
mod uf2;

#[derive(Clap)]
#[clap(version = "1.0", author = "Jonathan Nilsson")]
struct Opts {
    /// Verbose
    #[clap(short, long)]
    verbose: bool,

    /// Input file
    input: String,

    /// Output file
    output: Option<String>,
}

impl Opts {
    fn output_path(&self) -> PathBuf {
        if let Some(output) = &self.output {
            Path::new(output).with_extension("uf2")
        } else {
            Path::new(&self.input).with_extension("uf2")
        }
    }
}

fn elf2uf2(
    opts: &Opts,
    mut input: impl Read + Seek,
    mut output: impl Write,
) -> Result<(), Box<dyn Error>> {
    let eh = elf::read_and_check_elf32_header(&mut input)?;

    let ram_style = 0x2 == eh.entry >> 28;

    if opts.verbose {
        if ram_style {
            println!("Detected RAM binary");
        } else {
            println!("Detected FLASH binary");
        }
    }

    let valid_ranges = if ram_style {
        RP2040_ADDRESS_RANGES_RAM
    } else {
        RP2040_ADDRESS_RANGES_FLASH
    };

    let pages = read_and_check_elf32_ph_entries(opts, &mut input, &eh, &valid_ranges)?;

    if pages.is_empty() {
        return Err("The input file has no memory pages".into());
    }

    if ram_style {
        let expected_ep = pages.keys().next().unwrap() | 0x1;
        if eh.entry != expected_ep {
            return Err(format!(
                "A RAM binary should have an entry point at the beginning: {:#08x} (not {:#08x})\n",
                expected_ep, eh.entry as u32
            )
            .into());
        }
        const_assert!(0 == (MAIN_RAM_START & (PAGE_SIZE - 1)));
        // currently don't require this as entry point is now at the start, we don't know where reset vector is
    }

    let mut block = Uf2Block {
        magic_start0: UF2_MAGIC_START0,
        magic_start1: UF2_MAGIC_START1,
        flags: UF2_FLAG_FAMILY_ID_PRESENT,
        target_addr: 0,
        payload_size: PAGE_SIZE,
        block_no: 0,
        num_blocks: pages.len().assert_into(),
        file_size: RP2040_FAMILY_ID,
        data: [0; 476],
        magic_end: UF2_MAGIC_END,
    };

    for (page_num, (target_addr, fragments)) in pages.into_iter().enumerate() {
        block.target_addr = target_addr;
        block.block_no = page_num.assert_into();

        if opts.verbose {
            println!(
                "Page {} / {} {:#08x}",
                block.block_no as u32, block.num_blocks as u32, block.target_addr as u32
            );
        }

        block.data.iter_mut().for_each(|v| *v = 0);

        realize_page(&mut input, &fragments, &mut block.data)?;

        output.write_all(block.as_bytes())?;
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let opts: Opts = Opts::parse();

    let input = BufReader::new(File::open(&opts.input)?);
    let output = File::create(opts.output_path())?;

    if let Err(err) = elf2uf2(&opts, input, output) {
        println!("{}", err);
        fs::remove_file(opts.output_path())?;
    }

    Ok(())
}
