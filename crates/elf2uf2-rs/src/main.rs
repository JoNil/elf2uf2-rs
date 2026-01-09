use clap::Parser;
use elf2uf2_core::{build_page_map, open_elf, write_output, Family};
use env_logger::Env;
use log::*;
use sysinfo::Disks;

use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Write},
    path::Path,
};

use crate::reporter::ProgressBarReporter;

mod reporter;

#[derive(Parser, Debug, Default)]
#[clap(author = "Jonathan Nilsson")]
struct Opts {
    /// Verbose
    #[clap(short, long)]
    verbose: bool,

    /// Deploy to any connected pico
    #[clap(short, long)]
    deploy: bool,

    /// Select family short name for UF2
    #[clap(value_enum, short, long, default_value_t = Family::default())]
    family: Family,

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = Opts::parse();

    env_logger::Builder::from_env(Env::default())
        .filter_level(match options.verbose {
            true => LevelFilter::Debug,
            false => LevelFilter::Info,
        })
        .target(env_logger::Target::Stdout)
        .format(|buf, record| {
            let level = record.level();
            if level == Level::Info {
                writeln!(buf, "{}", record.args())
            } else {
                writeln!(buf, "{}: {}", record.level(), record.args())
            }
        })
        .init();

    let output_path = if let Some(output) = &options.output {
        Path::new(output).with_extension("uf2")
    } else {
        Path::new(&options.input).with_extension("uf2")
    };

    #[cfg(feature = "serial")]
    let serial_ports_before = serialport::available_ports()?;

    let input = BufReader::new(File::open(&options.input)?);

    let (output, output_path) = if options.deploy {
        let disks = Disks::new_with_refreshed_list();

        let mut pico_drive = None;
        for disk in &disks {
            let mount = disk.mount_point();

            if mount.join("INFO_UF2.TXT").is_file() {
                info!("Found pico uf2 disk {}", &mount.to_string_lossy());
                pico_drive = Some(mount.to_owned());
                break;
            }
        }

        if let Some(pico_drive) = pico_drive {
            let path = pico_drive.join("out.uf2");
            (File::create(&path)?, path)
        } else {
            return Err("Unable to find mounted pico".into());
        }
    } else {
        (File::create(&output_path)?, output_path)
    };

    let family = options.family;

    if options.verbose {
        info!("Using UF2 Family {:?}", family);
    }

    let writer = BufWriter::new(output);
    let mut elf = open_elf(input)?;
    let should_print_progress = log::max_level() >= LevelFilter::Info;
    let pages = build_page_map(&elf, family)?;

    let result = if should_print_progress {
        let len = pages.len() as u64 * 512;
        let mut reporter = ProgressBarReporter::new(len, writer);
        let result = write_output(&mut elf, &pages, &mut reporter, family);
        reporter.finish();
        result
    } else {
        write_output(&mut elf, &pages, writer, family)
    };

    if let Err(err) = result {
        fs::remove_file(output_path)?;
        return Err(Box::new(err));
    }

    // New line after progress bar
    println!();

    #[cfg(feature = "serial")]
    if options.serial {
        use std::process;
        use std::sync::{Arc, Mutex};
        use std::time::Duration;
        use std::{io, thread};

        let mut counter = 0;

        let serial_port_info = 'find_loop: loop {
            for port in serialport::available_ports()? {
                if !serial_ports_before.contains(&port) {
                    info!("Found pico serial on {}", &port.port_name);
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
                            port.write_all(b"elf2uf2-term\r\n").ok();
                            port.flush().ok();
                            process::exit(0);
                        }
                    };

                    if options.term {
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
                                    if options.term {
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
