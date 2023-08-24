use std::fmt::Debug;
use std::io::Write;
use std::sync::{Arc, Mutex};

/// Threadsafe writer that can be read from
///
/// Useful for testing
#[derive(Debug)]
pub struct ReadYourWrite<W>
where
    W: Write + AsRef<[u8]>,
{
    arc: Arc<Mutex<W>>,
}

impl<W> ReadYourWrite<W>
where
    W: Write + AsRef<[u8]>,
{
    #[allow(dead_code)]
    pub(crate) fn writer(writer: W) -> Self {
        Self {
            arc: Arc::new(Mutex::new(writer)),
        }
    }

    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn reader(&self) -> Arc<Mutex<W>> {
        self.arc.clone()
    }
}

impl<W> Write for ReadYourWrite<W>
where
    W: Write + AsRef<[u8]> + Debug,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut writer = self.arc.lock().expect("Internal error");
        writer.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut writer = self.arc.lock().expect("Internal error");
        writer.flush()
    }
}
