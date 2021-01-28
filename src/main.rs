use address_range::{RP2040_ADDRESS_RANGES_FLASH, RP2040_ADDRESS_RANGES_RAM};
use clap::Clap;
use elf::read_and_check_elf32_ph_entries;
use std::{
    error::Error,
    fs::{self, File},
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
};

mod address_range;
mod elf;

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
            output.into()
        } else {
            Path::new(&self.input).with_extension("uf2")
        }
    }
}

fn elf2uf2(
    opts: &Opts,
    mut input: impl Read,
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
