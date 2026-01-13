use std::io::Stdout;

use pbr::{ProgressBar, Units};

pub struct ProgressBarReporter<T> {
    pb: ProgressBar<Stdout>,
    inner: T,
}

impl<T> ProgressBarReporter<T>
where
    T: std::io::Write,
{
    pub fn new(total_bytes: u64, inner: T) -> Self {
        let mut pb = ProgressBar::new(total_bytes);
        pb.set_units(Units::Bytes);

        Self { pb, inner }
    }

    pub fn finish(&mut self) {
        self.pb.finish();
    }
}

impl<T> std::io::Write for ProgressBarReporter<T>
where
    T: std::io::Write,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.pb.add(written as _);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
