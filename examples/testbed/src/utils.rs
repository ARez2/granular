use regex::Regex;
use std::io::{self, Write};
use std::sync::Arc;

pub struct CleaningWriter<W> {
    inner: W,
    buffer: Vec<u8>,
    regex: Arc<Regex>,
}

impl<W: Write> CleaningWriter<W> {
    pub fn new(inner: W, regex: Arc<Regex>) -> Self {
        Self {
            inner,
            buffer: Vec::new(),
            regex,
        }
    }

    fn clean_line(&self, line: &str) -> String {
        // Find the LAST "granular/" in the line.
        let Some(granular_pos) = line.rfind("granular/") else {
            return line.to_owned();
        };

        // Only search for the path before the final "granular/".
        let before_granular = &line[..granular_pos];

        let Some(captures) = self.regex.captures(before_granular) else {
            return line.to_owned();
        };

        let Some(path_match) = captures.get(1) else {
            return line.to_owned();
        };

        let mut cleaned = String::with_capacity(line.len());

        // Keep everything before the path.
        // This includes the whitespace before the path.
        cleaned.push_str(&before_granular[..path_match.start()]);

        // Keep everything after the final "granular/".
        cleaned.push_str(&line[granular_pos + "granular/".len()..]);

        cleaned
    }

    pub fn write_buffer(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let text = String::from_utf8_lossy(&self.buffer);
        let cleaned = self.clean_line(&text);

        self.inner.write_all(cleaned.as_bytes())?;
        self.buffer.clear();

        Ok(())
    }
}

impl<W: Write> Write for CleaningWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);

        while let Some(pos) = self.buffer.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=pos).collect();

            let text = String::from_utf8_lossy(&line);
            let cleaned = self.clean_line(&text);

            self.inner.write_all(cleaned.as_bytes())?;
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.write_buffer()?;
        self.inner.flush()
    }
}
