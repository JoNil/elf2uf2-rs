use std::io::Write;
use std::{
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::Path,
};

use elf2uf2_core::{build_page_map, open_elf, write_output, Family};
use log::{info, LevelFilter};
use sysinfo::Disks;

use crate::reporter::ProgressBarReporter;

pub fn deploy<P: AsRef<Path>>(
    input_path: P,
    family: Family,
    serial: bool,
    term: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let input = input_path.as_ref();
    let input = BufReader::new(File::open(input)?);

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

    #[cfg(feature = "serial")]
    let serial_ports_before = serialport::available_ports()?;

    let (output, output_path) = match pico_drive {
        Some(pico_drive) => {
            let path = pico_drive.join("out.uf2");
            (File::create(&path)?, path)
        }
        None => return Err("Unable to find mounted pico".into()),
    };

    info!("Using UF2 Family {:?}", family);

    let writer = BufWriter::new(output);
    let mut elf = open_elf(input)?;
    let should_print_progress = log::max_level() >= LevelFilter::Info;
    let pages = build_page_map(&elf, family)?;

    let result = if should_print_progress {
        let len = pages.len() as u64 * 512;
        log::info!("Transfering program to microcontroller");
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

    println!();

    #[cfg(feature = "serial")]
    if serial {
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

                    if term {
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
                                    if term {
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
