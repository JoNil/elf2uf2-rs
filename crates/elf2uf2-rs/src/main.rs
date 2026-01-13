use clap::{Parser, ValueEnum};
use elf2uf2_core::Family;
use env_logger::Env;
use log::*;

use std::io::Write;

use crate::{convert::convert, deploy::deploy};

mod convert;
mod deploy;
mod reporter;

#[derive(Parser, Debug)]
enum Command {
    /// Convert ELF to UF2 file on disk
    Convert {
        /// Input ELF file
        input: String,

        /// Output UF2 file
        output: String,

        /// Select family short name for UF2
        #[clap(value_enum, short, long, default_value_t = Family::default())]
        family: Family,
    },
    /// Deploy ELF directly to a connected board
    Deploy {
        /// Input ELF file
        input: String,

        /// Select family short name for UF2
        #[clap(value_enum, short, long, default_value_t = Family::default())]
        family: Family,

        /// Connect to serial after deploy
        #[cfg(feature = "serial")]
        #[clap(short, long)]
        serial: bool,

        /// Send termination message on Ctrl+C
        #[cfg(feature = "serial")]
        #[clap(short, long)]
        term: bool,
    },
}

#[derive(Parser, Debug, Default)]
#[clap(version, about, long_about = None, author = "Jonathan Nilsson")]
#[command(arg_required_else_help = true)]
struct Cli {
    /// Set the logging verbosity
    #[clap(short, long, value_enum, global = true, default_value_t = LogLevel::Info)]
    verbose: LogLevel,

    #[clap(subcommand)]
    command: Option<Command>,
}

#[derive(Copy, Clone, Debug, Default, ValueEnum)]
enum LogLevel {
    Off,
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl From<LogLevel> for LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => LevelFilter::Error,
            LogLevel::Warn => LevelFilter::Warn,
            LogLevel::Info => LevelFilter::Info,
            LogLevel::Debug => LevelFilter::Debug,
            LogLevel::Trace => LevelFilter::Trace,
            LogLevel::Off => LevelFilter::Off,
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    env_logger::Builder::from_env(Env::default())
        .filter_level(cli.verbose.into())
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

    let command = match cli.command {
        Some(command) => command,
        None => return Ok(()),
    };

    match command {
        Command::Convert {
            input,
            output,
            family,
        } => convert(&input, &output, family),
        Command::Deploy {
            input,
            family,
            serial,
            term,
        } => deploy(&input, family, serial, term),
    }
}
