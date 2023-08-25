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

/// Iterator yielding every line in a string. The line includes newline character(s).
///
/// <https://stackoverflow.com/a/40457615>
///
/// The problem this solves is when iterating over lines of a string, the whitespace may be significant.
/// For example if you want to split a string and then get the original string back then calling
/// `lines().collect<Vec<_>>().join("\n")` will never preserve trailing newlines.
///
/// There's another option to `lines().fold(String::new(), |s, l| s + l + "\n")`, but that
/// always adds a trailing newline even if the original string doesn't have one.
pub(crate) struct LinesWithEndings<'a> {
    input: &'a str,
}

impl<'a> LinesWithEndings<'a> {
    pub fn from(input: &'a str) -> LinesWithEndings<'a> {
        LinesWithEndings { input }
    }
}

impl<'a> Iterator for LinesWithEndings<'a> {
    type Item = &'a str;

    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        if self.input.is_empty() {
            return None;
        }
        let split = self.input.find('\n').map_or(self.input.len(), |i| i + 1);

        let (line, rest) = self.input.split_at(split);
        self.input = rest;
        Some(line)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_lines_with_endings() {
        let actual = LinesWithEndings::from("foo\nbar")
            .map(|line| format!("z{line}"))
            .collect::<String>();

        assert_eq!("zfoo\nzbar", actual);

        let actual = LinesWithEndings::from("foo\nbar\n")
            .map(|line| format!("z{line}"))
            .collect::<String>();

        assert_eq!("zfoo\nzbar\n", actual);
    }
}
