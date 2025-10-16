use std::io::Stdout;

use log::{max_level, LevelFilter};
use pbr::{ProgressBar, Units};

pub struct ProgressBarReporter {
    pb: Option<ProgressBar<Stdout>>,
}

impl ProgressReporter for ProgressBarReporter {
    fn start(&mut self, total_bytes: usize) {
        if let Some(pb) = self.pb.as_mut() {
            log::info!("Transfering program to pico");
            pb.total = total_bytes as u64;
            pb.set_units(Units::Bytes);
        }
    }

    fn advance(&mut self, bytes: usize) {
        if let Some(pb) = self.pb.as_mut() {
            pb.add(bytes as u64);
        }
    }

    fn finish(&mut self) {
        if let Some(pb) = self.pb.as_mut() {
            pb.finish();
        }
    }
}

impl ProgressBarReporter {
    pub fn new(deploy: bool) -> Self {
        if max_level() >= LevelFilter::Info && deploy {
            Self {
                pb: Some(ProgressBar::new(0)),
            }
        } else {
            Self { pb: None }
        }
    }
}

pub trait ProgressReporter {
    fn start(&mut self, total_bytes: usize);
    fn advance(&mut self, bytes: usize);
    fn finish(&mut self);
}

#[allow(unused)]
pub struct NoProgress;
impl ProgressReporter for NoProgress {
    fn start(&mut self, _total_bytes: usize) {}
    fn advance(&mut self, _bytes: usize) {}
    fn finish(&mut self) {}
}
