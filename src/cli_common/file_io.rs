//! File I/O helpers for CLI binaries.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};

const SHORT_OUTPUT_LENGTH: usize = 20 * 80;
const TRUNCATED_TTY_MSG: &[u8] =
    b"\n[truncated; use a pipe, a redirect, or --output to see full message]\n";

/// Wrapper around either a file or standard input.
pub enum InputReader {
    /// Wrapper around a file.
    File(File),
    /// Wrapper around standard input.
    Stdin(io::Stdin),
}

impl InputReader {
    /// Reads input from the given filename, or standard input if `None`.
    pub fn new(input: Option<String>) -> io::Result<Self> {
        Ok(if let Some(filename) = input {
            InputReader::File(File::open(filename)?)
        } else {
            InputReader::Stdin(io::stdin())
        })
    }
}

impl Read for InputReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            InputReader::File(f) => f.read(buf),
            InputReader::Stdin(handle) => handle.read(buf),
        }
    }
}

/// Writer that wraps standard output to handle TTYs nicely.
pub struct StdoutWriter {
    inner: io::Stdout,
    count: usize,
    is_tty: bool,
    truncated: bool,
}

impl StdoutWriter {
    fn new(is_tty: bool) -> Self {
        StdoutWriter {
            inner: io::stdout(),
            count: 0,
            is_tty,
            truncated: false,
        }
    }
}

impl Write for StdoutWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if self.is_tty {
            // Don't send unprintable output to TTY
            if std::str::from_utf8(data).is_err() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "not printing unprintable message to stdout",
                ));
            }

            // Drop output if we've truncated already, or need to.
            if self.truncated || self.count == SHORT_OUTPUT_LENGTH {
                if !self.truncated {
                    self.inner.write_all(TRUNCATED_TTY_MSG)?;
                    self.truncated = true;
                }

                return io::sink().write(data);
            }

            let mut to_write = SHORT_OUTPUT_LENGTH - self.count;
            if to_write > data.len() {
                to_write = data.len();
            }

            let mut ret = self.inner.write(&data[..to_write])?;
            self.count += to_write;

            // If we have reached the output limit with data to spare,
            // truncate and drop the remainder.
            if self.count == SHORT_OUTPUT_LENGTH && data.len() > to_write {
                if !self.truncated {
                    self.inner.write_all(TRUNCATED_TTY_MSG)?;
                    self.truncated = true;
                }
                ret += io::sink().write(&data[to_write..])?;
            }

            Ok(ret)
        } else {
            self.inner.write(data)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Wrapper around either a file or standard output.
pub enum OutputWriter {
    /// Wrapper around a file.
    File(File),
    /// Wrapper around standard output.
    Stdout(StdoutWriter),
}

impl OutputWriter {
    /// Writes output to the given filename, or standard output if `None`.
    pub fn new(output: Option<String>, deny_tty: bool) -> io::Result<Self> {
        let is_tty = console::user_attended();
        if let Some(filename) = output {
            Ok(OutputWriter::File(
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(filename)?,
            ))
        } else if is_tty && deny_tty {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "not printing to stdout",
            ))
        } else {
            Ok(OutputWriter::Stdout(StdoutWriter::new(is_tty)))
        }
    }
}

impl Write for OutputWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        match self {
            OutputWriter::File(f) => f.write(data),
            OutputWriter::Stdout(handle) => handle.write(data),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            OutputWriter::File(f) => f.flush(),
            OutputWriter::Stdout(handle) => handle.flush(),
        }
    }
}
