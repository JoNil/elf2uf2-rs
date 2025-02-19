use crate::address_range::{MAIN_RAM_END, XIP_SRAM_END, XIP_SRAM_START};
use address_range::{
    FLASH_SECTOR_ERASE_SIZE, MAIN_RAM_START, RP2040_ADDRESS_RANGES_FLASH, RP2040_ADDRESS_RANGES_RAM,
};
use assert_into::AssertInto;
use clap::Parser;
use elf::{realize_page, AddressRangesExt, Elf32Header, PAGE_SIZE};
use pbr::{ProgressBar, Units};
use static_assertions::const_assert;
use std::{
    collections::HashSet,
    error::Error,
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Seek, Write},
    path::{Path, PathBuf},
    sync::OnceLock,
};
use sysinfo::Disks;
use uf2::{
    Uf2BlockData, Uf2BlockFooter, Uf2BlockHeader, RP2040_FAMILY_ID, UF2_FLAG_FAMILY_ID_PRESENT,
    UF2_MAGIC_END, UF2_MAGIC_START0, UF2_MAGIC_START1,
};
use zerocopy::IntoBytes;

mod address_range;
mod elf;
mod uf2;

#[derive(Parser, Debug, Default)]
#[clap(author = "Jonathan Nilsson")]
struct Opts {
    /// Verbose
    #[clap(short, long)]
    verbose: bool,

    /// Deploy to any connected pico
    #[clap(short, long)]
    deploy: bool,

    /// Select family ID for UF2. See https://github.com/microsoft/uf2/blob/master/utils/uf2families.json for list.
    #[clap(short, long, default_value_t = RP2040_FAMILY_ID, value_parser = num_parser)]
    family: u32,

    /// Connect to serial after deploy
    #[cfg(feature = "serial")]
    #[clap(short, long)]
    serial: bool,

    /// Send termination message to the device on ctrl+c
    #[cfg(feature = "serial")]
    #[clap(short, long)]
    term: bool,

    /// Input file
    input: String,

    /// Output file
    output: Option<String>,
}

// allow user to pass hex formatted numbers (typically the format used by family ids)
fn num_parser(s: &str) -> Result<u32, &'static str> {
    match s.get(0..2) {
        Some("0x") => u32::from_str_radix(&s[2..], 16).map_err(|_| "invalid hex number"),
        Some("0b") => u32::from_str_radix(&s[2..], 2).map_err(|_| "invalid binary number"),
        _ => s.parse::<u32>().map_err(|_| "invalid decimal number"),
    }
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

static OPTS: OnceLock<Opts> = OnceLock::new();

fn elf2uf2(
    mut input: impl Read + Seek,
    mut output: impl Write,
    family_id: u32,
) -> Result<(), Box<dyn Error>> {
    let eh = Elf32Header::from_read(&mut input)?;

    let entries = eh.read_elf32_ph_entries(&mut input)?;

    let ram_style = eh
        .is_ram_binary(&entries)
        .ok_or("entry point is not in mapped part of file".to_string())?;

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

    let mut pages = valid_ranges.check_elf32_ph_entries(&entries)?;

    if pages.is_empty() {
        return Err("The input file has no memory pages".into());
    }

    if ram_style {
        let mut expected_ep_main_ram = u32::MAX;
        let mut expected_ep_xip_sram = u32::MAX;

        #[allow(clippy::manual_range_contains)]
        pages.keys().copied().for_each(|addr| {
            if addr >= MAIN_RAM_START && addr <= MAIN_RAM_END {
                expected_ep_main_ram = expected_ep_main_ram.min(addr) | 0x1;
            } else if addr >= XIP_SRAM_START && addr < XIP_SRAM_END {
                expected_ep_xip_sram = expected_ep_xip_sram.min(addr) | 0x1;
            }
        });

        let expected_ep = if expected_ep_main_ram != u32::MAX {
            expected_ep_main_ram
        } else {
            expected_ep_xip_sram
        };

        if expected_ep == expected_ep_xip_sram {
            return Err("B0/B1 Boot ROM does not support direct entry into XIP_SRAM".into());
        } else if eh.entry != expected_ep {
            #[allow(clippy::unnecessary_cast)]
            return Err(format!(
                "A RAM binary should have an entry point at the beginning: {:#08x} (not {:#08x})",
                expected_ep, eh.entry as u32
            )
            .into());
        }
        const_assert!(0 == (MAIN_RAM_START & (PAGE_SIZE - 1)));

        // TODO: check vector table start up
        // currently don't require this as entry point is now at the start, we don't know where reset vector is
    } else {
        // Fill in empty dummy uf2 pages to align the binary to flash sectors (except for the last sector which we don't
        // need to pad, and choose not to to avoid making all SDK UF2s bigger)
        // That workaround is required because the bootrom uses the block number for erase sector calculations:
        // https://github.com/raspberrypi/pico-bootrom/blob/c09c7f08550e8a36fc38dc74f8873b9576de99eb/bootrom/virtual_disk.c#L205

        let touched_sectors: HashSet<u32> = pages
            .keys()
            .map(|addr| addr / FLASH_SECTOR_ERASE_SIZE)
            .collect();

        let last_page_addr = *pages.last_key_value().unwrap().0;
        for sector in touched_sectors {
            let mut page = sector * FLASH_SECTOR_ERASE_SIZE;

            while page < (sector + 1) * FLASH_SECTOR_ERASE_SIZE {
                if page < last_page_addr && !pages.contains_key(&page) {
                    pages.insert(page, Vec::new());
                }
                page += PAGE_SIZE;
            }
        }
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

        #[allow(clippy::unnecessary_cast)]
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
        let disks = Disks::new_with_refreshed_list();

        let mut pico_drive = None;
        for disk in &disks {
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

    let family_id = Opts::global().family;
    if Opts::global().verbose {
        println!("Using UF2 Family ID 0x{:x}", family_id);
    }

    if let Err(err) = elf2uf2(input, BufWriter::new(output), family_id) {
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
        use std::process;
        use std::sync::{Arc, Mutex};
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

            if counter == 100 {
                break None;
            }

            thread::sleep(Duration::from_millis(200));
        };

        if let Some(serial_port_info) = serial_port_info {
            for _ in 0..100 {
                if let Ok(port) = serialport::new(&serial_port_info.port_name, 115200)
                    .timeout(Duration::from_millis(100))
                    .flow_control(serialport::FlowControl::None)
                    .open()
                {
                    let port = Arc::new(Mutex::new(port));

                    let handler = {
                        let port = port.clone();
                        move || {
                            let mut port = port.lock().unwrap();
                            port.write_all(b"elf2uf2-term\n\r").ok();
                            port.flush().ok();
                            process::exit(0);
                        }
                    };

                    if Opts::global().term {
                        ctrlc::set_handler(handler.clone()).expect("Error setting Ctrl-C handler");
                    }

                    let data_terminal_ready_succeeded = {
                        let mut port = port.lock().unwrap();
                        port.write_data_terminal_ready(true).is_ok()
                    };
                    if data_terminal_ready_succeeded {
                        let mut serial_buf = [0; 1024];
                        loop {
                            let read = {
                                let mut port = port.lock().unwrap();
                                port.read(&mut serial_buf)
                            };

                            match read {
                                Ok(t) => {
                                    io::stdout().write_all(&serial_buf[..t])?;
                                    io::stdout().flush()?;
                                }
                                Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
                                Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                                    if Opts::global().term {
                                        handler();
                                    }
                                    return Err(e.into());
                                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    pub fn hello_usb() {
        // Horrendous hack to get it to stop complaining about opts
        // TODO: just pass opts by reference, or use log crate
        OPTS.set(Default::default()).ok();

        let bytes_in = io::Cursor::new(&include_bytes!("../hello_usb.elf")[..]);
        let mut bytes_out = Vec::new();
        elf2uf2(bytes_in, &mut bytes_out, RP2040_FAMILY_ID).unwrap();

        assert_eq!(bytes_out, include_bytes!("../hello_usb.uf2"));
    }

    #[test]
    pub fn hello_serial() {
        // Horrendous hack to get it to stop complaining about opts
        // TODO: just pass opts by reference, or use log crate
        OPTS.set(Default::default()).ok();

        let bytes_in = io::Cursor::new(&include_bytes!("../hello_serial.elf")[..]);
        let mut bytes_out = Vec::new();
        elf2uf2(bytes_in, &mut bytes_out, RP2040_FAMILY_ID).unwrap();

        assert_eq!(bytes_out, include_bytes!("../hello_serial.uf2"));
    }
}
