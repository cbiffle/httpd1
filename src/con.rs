//! HTTP connection management

use std::io;
use std::fs;

use super::error::*;
use super::timeout;
use super::unix;

use std::io::BufRead;
use std::io::Write;

use std::os::unix::ffi::OsStrExt;

const INPUT_BUF_BYTES: usize = 1024;
const OUTPUT_BUF_BYTES: usize = 1024;
const LOG_BUF_BYTES: usize = 256;
const FILE_BUF_BYTES: usize = 1024;


pub struct Connection {
  input: io::BufReader<timeout::SafeFile>,
  output: io::BufWriter<timeout::SafeFile>,
  error: io::BufWriter<fs::File>,
  remote: String,
  pub buf: Box<[u8; FILE_BUF_BYTES]>,
}

impl Connection {
  pub fn new(remote: String) -> Connection {
    Connection {
      input: io::BufReader::with_capacity(INPUT_BUF_BYTES,
          timeout::SafeFile::new(unix::stdin())),
      output: io::BufWriter::with_capacity(OUTPUT_BUF_BYTES,
          timeout::SafeFile::new(unix::stdout())),
      error: io::BufWriter::with_capacity(LOG_BUF_BYTES, unix::stderr()),
      remote: remote,
      buf: Box::new([0; FILE_BUF_BYTES]),
    }
  }

  /// Reads a CRLF-terminated line, of the sort used in HTTP requests.
  /// This function guarantees that a successful result describes an entire
  /// line -- if the input is closed before CRLF, it signals `BrokenPipe`.
  ///
  /// As suggested in section 19.3 of the HTTP/1.1 spec ("Tolerant
  /// Applications"), we actually accept LF-terminated lines as well as CRLF.
  ///
  /// The delimiter is removed before the result is returned.
  pub fn readline(&mut self) -> Result<Vec<u8>> {
    let mut line = Vec::new();
    match try!(self.input.read_until(b'\n', &mut line)) {
      0 => return Err(HttpError::ConnectionClosed),
      _ => {
        let len = line.len();
        if line.last().cloned() == Some(b'\n') {
          // We actually found our delimiter.
          line.truncate(len - 1);
          if line.last().cloned() == Some(b'\r') { line.truncate(len - 2) }
          return Ok(line)
        } else {
          // The stream ended.
          return Err(HttpError::ConnectionClosed)
        }
      }
    }
  }

  pub fn write(&mut self, data: &[u8]) -> Result<()> {
    // Don't use the default conversion from io::Error here -- failures on
    // write are the client's fault and can't typically be reported, so it's
    // important that we indicate ConnectionClosed.
    self.output.write_all(data).map_err(|_| HttpError::ConnectionClosed)
  }

  pub fn write_to_string<T: ToString>(&mut self, value: T) -> Result<()> {
    self.write(value.to_string().as_bytes())
  }

  pub fn write_hex(&mut self, value: usize) -> Result<()> {
    let s = format!("{:x}", value);
    self.write(s.as_bytes())
  }

  pub fn write_buf(&mut self, count: usize) -> Result<()> {
    self.output.write_all(&self.buf[..count])
        .map_err(|_| HttpError::ConnectionClosed)
  }

  pub fn flush_output(&mut self) -> Result<()> {
    self.output.flush().map_err(|_| HttpError::ConnectionClosed)
  }

  pub fn log(&mut self,
             path: &[u8],
             context: Option<&'static [u8]>,
             msg: &[u8]) {
    // We do not expect writes to the log to fail, and we can't easily
    // handle them if they do, so we ignore the result and return.
    macro_rules! ignore {
      ($op: expr) => {
        match $op {
          Ok(_) => (),
          Err(_) => return ()
        }
      }
    }

    ignore!(self.error.write_all(self.remote.as_bytes()));
    ignore!(self.error.write_all(b" read "));
    if path.len() > 100 {
      ignore!(self.error.write_all(&path[..100]));
      ignore!(self.error.write_all(b"..."))
    } else {
      ignore!(self.error.write_all(path))
    }
    if let Some(c) = context {
      ignore!(self.error.write_all(b" ["));
      ignore!(self.error.write_all(c));
      ignore!(self.error.write_all(b"]"));
    }
    ignore!(self.error.write_all(b": "));
    ignore!(self.error.write_all(msg));
    ignore!(self.error.write_all(b"\n"));
    ignore!(self.error.flush());
  }
}

#[cfg(test)]
mod tests {
  use std::io;
  use std::mem;
  use super::*;
  use super::super::unix;
  use super::super::timeout;
  use super::super::error::*;

  use std::io::Write;

  #[test]
  fn test_connection_readline() {
    let (mut c, mut to_con, from_con, con_err) = {
      let pipe_to_con = unix::pipe().unwrap();
      let pipe_from_con = unix::pipe().unwrap();
      let error_from_con = unix::pipe().unwrap();
  
      let c = Connection {
        input: io::BufReader::new(timeout::SafeFile::new(pipe_to_con.input)),
        output: io::BufWriter::new(
            timeout::SafeFile::new(pipe_from_con.output)),
        error: io::BufWriter::new(error_from_con.output),
        remote: "REMOTE".to_string(),
        buf: Box::new([0; super::FILE_BUF_BYTES]),
      };
  
      (c, pipe_to_con.output, pipe_from_con.input, error_from_con.input)
    };
  
    // Note: this test relies on buffering in the pipes.  Hoping for the best.
  
    to_con.write_all(b"\r\n").unwrap();
    assert_eq!(b"", &c.readline().unwrap()[..]);
    to_con.write_all(b"abcd\r\nohai\r\n").unwrap();
    assert_eq!(b"abcd", &c.readline().unwrap()[..]);
    assert_eq!(b"ohai", &c.readline().unwrap()[..]);
  
    // Mostly for testing, but also as suggested by the spec, we also tolerate
    // pure Unix-style LF endings.
    to_con.write_all(b"also just\nnewline\n").unwrap();
    assert_eq!(b"also just", &c.readline().unwrap()[..]);
    assert_eq!(b"newline", &c.readline().unwrap()[..]);
  
    // Test what happens when the connection is dropped.
    to_con.write_all(b"truncated").unwrap();
    mem::drop(to_con);  // close our side of this pipe
    match c.readline().err() {
      Some(HttpError::ConnectionClosed) => (),
      Some(_) => panic!("Unexpected error from readline() at stream end"),
      _ => panic!("readline() must fail at stream end"),
    };

    mem::drop(from_con);  // suppress unused variable warning ;-)
  }
}
