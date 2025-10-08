use clap::Parser;
use pbr::{ProgressBar, Units};
use std::{
    collections::HashSet,
    error::Error,
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Seek, Write},
    path::{Path, PathBuf},
    sync::OnceLock,
};
use sysinfo::Disks;

#[derive(Parser, Debug, Default)]
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

    /// Send termination message to the device on ctrl+c
    #[cfg(feature = "serial")]
    #[clap(short, long)]
    term: bool,

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

static OPTS: OnceLock<Opts> = OnceLock::new();



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

    if let Err(err) = elf2uf2(input, BufWriter::new(output)) {
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
                            port.write_all(b"elf2uf2-term\r\n").ok();
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
