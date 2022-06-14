use clap::Parser;
use once_cell::sync::OnceCell;
use serialport::FlowControl;
use static_assertions::const_assert;
use std::{
    error::Error,
    fs::{self, File},
    io::{self, BufReader, Read, Write},
    path::{Path, PathBuf},
    thread,
    time::Duration,
};
use sysinfo::{DiskExt, SystemExt};

use elf2uf2_rs as lib;
use lib::address_range::{MAIN_RAM_START, RP2040_ADDRESS_RANGES_FLASH, RP2040_ADDRESS_RANGES_RAM};

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

fn main() -> Result<(), Box<dyn Error>> {
    OPTS.set(Opts::parse()).unwrap();

    let serial_ports_before = serialport::available_ports()?;
    let mut deployed_path = None;

    {
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
        if Opts::global().deploy {
            println!("Transfering program to pico");
        }
        let verbosity = match (Opts::global().verbose, Opts::global().deploy) {
            #[cfg(feature = "progress_bar")]
            (false, true) => lib::Verbosity::Progress,
            (true, _) => lib::Verbosity::Verbose,
            _ => lib::Verbosity::Quiet,
        };
        if let Err(err) = lib::elf2uf2(
            input,
            output,
            verbosity,
            lib::RP2040_FAMILY_ID,
            |eh| {
                let ram_style = 0x2 == eh.entry >> 28;

                if Opts::global().verbose {
                    if ram_style {
                        println!("Detected RAM binary");
                    } else {
                        println!("Detected FLASH binary");
                    }
                }

                if ram_style {
                    RP2040_ADDRESS_RANGES_RAM
                } else {
                    RP2040_ADDRESS_RANGES_FLASH
                }
            },
            |eh, expected_ep| {
                let ram_style = 0x2 == eh.entry >> 28;
                if ram_style {
                    if eh.entry != expected_ep {
                        return Err(format!(
                        "A RAM binary should have an entry point at the beginning: {:#08x} (not {:#08x})\n",
                        expected_ep, eh.entry as u32
                    )
                    .into());
                    }
                    const_assert!(0 == (MAIN_RAM_START & (lib::PAGE_SIZE - 1)));
                    // currently don't require this as entry point is now at the start, we don't know where reset vector is
                }
                Ok(())
            },
        ) {
            if Opts::global().deploy {
                fs::remove_file(deployed_path.unwrap())?;
            } else {
                fs::remove_file(Opts::global().output_path())?;
            }
            return Err(err);
        }
    }

    // New line after progress bar
    println!();

    if Opts::global().serial {
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
                    .flow_control(FlowControl::Hardware)
                    .open()
                {
                    if port.write_data_terminal_ready(true).is_ok() {
                        let mut serial_buf = [0; 1024];
                        loop {
                            match port.read(&mut serial_buf) {
                                Ok(t) => io::stdout().write_all(&serial_buf[..t])?,
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
