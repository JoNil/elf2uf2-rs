use address_range::{MAIN_RAM_START, RP2040_ADDRESS_RANGES_FLASH, RP2040_ADDRESS_RANGES_RAM};
use assert_into::AssertInto;
use clap::Parser;
use elf::{read_and_check_elf32_ph_entries, realize_page, PAGE_SIZE};
use once_cell::sync::OnceCell;
use pbr::{ProgressBar, Units};
use static_assertions::const_assert;
use std::{
    error::Error,
    fs::{self, File},
    io::{BufReader, Read, Seek, Write},
    path::{Path, PathBuf},
};
use sysinfo::{DiskExt, SystemExt};
use uf2::{
    Uf2BlockData, Uf2BlockFooter, Uf2BlockHeader, RP2040_FAMILY_ID, UF2_FLAG_FAMILY_ID_PRESENT,
    UF2_MAGIC_END, UF2_MAGIC_START0, UF2_MAGIC_START1,
};
use zerocopy::AsBytes;

mod address_range;
mod elf;
mod uf2;

#[derive(Parser, Debug)]
#[clap(author = "Jonathan Nilsson")]
struct Opts {
    /// Verbose
    #[clap(short, long)]
    verbose: bool,

    /// Deploy to any connected pico
    #[clap(short, long)]
    deploy: bool,

    /// Connect to serial after deploy
    #[cfg(feature = "serial")]
    #[clap(short, long)]
    serial: bool,

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

    fn global() -> &'static Opts {
        OPTS.get().expect("Opts is not initialized")
    }
}

static OPTS: OnceCell<Opts> = OnceCell::new();

fn elf2uf2(mut input: impl Read + Seek, mut output: impl Write) -> Result<(), Box<dyn Error>> {
    let eh = elf::read_and_check_elf32_header(&mut input)?;

    let ram_style = 0x2 == eh.entry >> 28;

    if Opts::global().verbose {
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

    let pages = read_and_check_elf32_ph_entries(&mut input, &eh, valid_ranges)?;

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

    let mut block_header = Uf2BlockHeader {
        magic_start0: UF2_MAGIC_START0,
        magic_start1: UF2_MAGIC_START1,
        flags: UF2_FLAG_FAMILY_ID_PRESENT,
        target_addr: 0,
        payload_size: PAGE_SIZE,
        block_no: 0,
        num_blocks: pages.len().assert_into(),
        file_size: RP2040_FAMILY_ID,
    };

    let mut block_data: Uf2BlockData = [0; 476];

    let block_footer = Uf2BlockFooter {
        magic_end: UF2_MAGIC_END,
    };

    if Opts::global().deploy {
        println!("Transfering program to pico");
    }

    let mut pb = if !Opts::global().verbose && Opts::global().deploy {
        Some(ProgressBar::new((pages.len() * 512).assert_into()))
    } else {
        None
    };

    if let Some(pb) = &mut pb {
        pb.set_units(Units::Bytes);
    }

    let last_page_num = pages.len() - 1;

    for (page_num, (target_addr, fragments)) in pages.into_iter().enumerate() {
        block_header.target_addr = target_addr;
        block_header.block_no = page_num.assert_into();

        if Opts::global().verbose {
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
            if let Some(pb) = &mut pb {
                pb.add(512);
            }
        }
    }

    // Drop the output before the progress bar is allowd to finish
    drop(output);

    if let Some(pb) = &mut pb {
        pb.add(512);
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    OPTS.set(Opts::parse()).unwrap();

    #[cfg(feature = "serial")]
    let serial_ports_before = serialport::available_ports()?;

    let mut deployed_path = None;
    let input = BufReader::new(File::open(&Opts::global().input)?);

    let output = if Opts::global().deploy {
        let sys = sysinfo::System::new_all();

        let mut pico_drive = None;
        for disk in sys.disks() {
            let mount = disk.mount_point();

            if mount.join("INFO_UF2.TXT").is_file() {
                println!("Found pico uf2 disk {}", &mount.to_string_lossy());
                pico_drive = Some(mount.to_owned());
                break;
            }
        }

        if let Some(pico_drive) = pico_drive {
            deployed_path = Some(pico_drive.join("out.uf2"));
            File::create(deployed_path.as_ref().unwrap())?
        } else {
            return Err("Unable to find mounted pico".into());
        }
    } else {
        File::create(Opts::global().output_path())?
    };

    if let Err(err) = elf2uf2(input, output) {
        if Opts::global().deploy {
            fs::remove_file(deployed_path.unwrap())?;
        } else {
            fs::remove_file(Opts::global().output_path())?;
        }
        return Err(err);
    }

    // New line after progress bar
    println!();

    #[cfg(feature = "serial")]
    if Opts::global().serial {
        use std::time::Duration;
        use std::{io, thread};

        let mut counter = 0;

        let serial_port_info = 'find_loop: loop {
            for port in serialport::available_ports()? {
                if !serial_ports_before.contains(&port) {
                    println!("Found pico serial on {}", &port.port_name);
                    break 'find_loop Some(port);
                }
            }

            counter += 1;

            if counter == 10 {
                break None;
            }

            thread::sleep(Duration::from_millis(200));
        };

        if let Some(serial_port_info) = serial_port_info {
            for _ in 0..5 {
                if let Ok(mut port) = serialport::new(&serial_port_info.port_name, 115200)
                    .timeout(Duration::from_millis(100))
                    .flow_control(serialport::FlowControl::Hardware)
                    .open()
                {
                    if port.write_data_terminal_ready(true).is_ok() {
                        let mut serial_buf = [0; 1024];
                        loop {
                            match port.read(&mut serial_buf) {
                                Ok(t) => {
                                    io::stdout().write_all(&serial_buf[..t])?;
                                    io::stdout().flush()?;
                                }
                                Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
                                Err(e) => return Err(e.into()),
                            }
                        }
                    }
                }

                thread::sleep(Duration::from_millis(200));
            }
        }
    }

    Ok(())
}
