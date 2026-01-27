use clap::{Parser, ValueEnum};
use elf2uf2_core::boards::BoardIter;
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
    #[command(arg_required_else_help = true)]
    Convert {
        /// Input ELF file
        input: String,

        /// Output UF2 file
        output: String,

        /// Explicit board (rp2040, rp2350, etc.)
        #[clap(short, long, value_parser = board_parser)]
        board: String,
    },
    /// Deploy ELF directly to a connected board
    #[command(arg_required_else_help = true)]
    Deploy {
        /// Input ELF file
        input: String,

        /// Explicit board (rp2040, rp2350, etc.)
        #[clap(short, long, value_parser = board_parser)]
        board: String,

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

fn board_parser(s: &str) -> Result<String, String> {
    if let Some(board) = BoardIter::find_by_name(s) {
        Ok(board.board_name().to_string())
    } else {
        Err(format!("Unknown board '{}'", s))
    }
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
            board,
        } => {
            let board = BoardIter::find_by_name(&board)
                .expect("This already has been verified by board_parser");

            convert(&input, &output, board.as_ref())
        }
        Command::Deploy {
            input,
            board,
            serial,
            term,
        } => {
            let board = BoardIter::find_by_name(&board)
                .expect("This already has been verified by board_parser");

            deploy(&input, board.as_ref(), serial, term)
        }
    }
}
