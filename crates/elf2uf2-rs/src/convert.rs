use std::{
    fs::{self, File},
    io::BufReader,
    path::Path,
};

use elf2uf2_core::{boards::BoardInfo, build_page_map, open_elf, write_output};
use log::{LevelFilter, info};

use crate::reporter::ProgressBarReporter;

pub fn convert<P1: AsRef<Path>, P2: AsRef<Path>>(
    input_path: &P1,
    output_path: &P2,
    board: &dyn BoardInfo,
) -> Result<(), Box<dyn std::error::Error>> {
    let input = input_path.as_ref();
    let output_path = output_path.as_ref().with_extension("uf2");

    let input = BufReader::new(File::open(input)?);

    let output = File::create(&output_path)?;

    info!("Using UF2 Family {:#010x}", board.family_id());

    let mut elf = open_elf(input)?;
    let should_print_progress = log::max_level() >= LevelFilter::Info;
    let pages = build_page_map(&elf, board)?;

    let result = if should_print_progress {
        let len = pages.len() as u64 * 512;
        log::info!("Writing program to disk");
        let mut reporter = ProgressBarReporter::new(len, output);
        let result = write_output(&mut elf, &pages, &mut reporter, board);
        reporter.finish();
        result
    } else {
        write_output(&mut elf, &pages, output, board)
    };

    if let Err(err) = result {
        fs::remove_file(output_path)?;
        return Err(Box::new(err));
    }

    println!();

    Ok(())
}
