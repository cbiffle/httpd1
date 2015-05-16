// Need libc to do unbuffered stdout/stdin :-/
#![feature(libc)]
extern crate libc;

use std::process;
use std::io;
use std::fs;
use std::mem;

use std::io::BufRead;
use std::io::Write;

pub mod unix;

pub enum HttpError {
  ConnectionClosed,
  IoError(io::Error),
}

pub fn lift_io<T>(r: io::Result<T>) -> Result<T> {
  r.map_err(|e| HttpError::IoError(e))
}

pub type Result<R> = std::result::Result<R, HttpError>;

pub struct Connection {
  input: io::BufReader<fs::File>,
  output: io::BufWriter<fs::File>,
}

impl Connection {
  fn new() -> Connection {
    Connection {
      input: io::BufReader::new(unix::stdin()),
      output: io::BufWriter::new(unix::stdout()),
    }
  }

  /// Reads a CRLF-terminated line, of the sort used in HTTP requests.
  /// This function guarantees that a successful result describes an entire
  /// line -- if the input is closed before CRLF, it signals `BrokenPipe`.
  ///
  /// The delimiter is removed before the result is returned.
  fn readline(&mut self) -> Result<Vec<u8>> {
    // We can use read_until to find the next newline, and then look behind it
    // to figure out whether we're done.
    let mut line = Vec::new();
    loop {
      match try!(lift_io(self.input.read_until(b'\n', &mut line))) {
        0 => return Err(HttpError::ConnectionClosed),
        _ => {
          if line.ends_with(b"\r\n") {
            let text_len = line.len() - 2;
            line.truncate(text_len);
            return Ok(line)
          }
        }
      }
    }
  }
}

#[test]
fn test_connection_readline() {
  let (mut c, mut to_con, mut from_con) = {
    let pipe_to_con = unix::pipe().unwrap();
    let pipe_from_con = unix::pipe().unwrap();

    let c = Connection {
      input: io::BufReader::new(pipe_to_con.input),
      output: io::BufWriter::new(pipe_from_con.output),
    };

    (c, pipe_to_con.output, pipe_from_con.input)
  };

  // Note: this test relies on buffering in the pipes.  Hoping for the best.

  to_con.write_all(b"\r\n").unwrap();
  assert_eq!(b"", &c.readline().unwrap()[..]);
  to_con.write_all(b"abcd\r\nohai\r\n").unwrap();
  assert_eq!(b"abcd", &c.readline().unwrap()[..]);
  assert_eq!(b"ohai", &c.readline().unwrap()[..]);

  to_con.write_all(b"embedded\nnewline\r\n").unwrap();
  assert_eq!(b"embedded\nnewline", &c.readline().unwrap()[..]);

  // Test what happens when the connection is dropped.
  to_con.write_all(b"truncated").unwrap();
  mem::drop(to_con);  // close our side of this pipe
  if let Some(e) = c.readline().err() {
    assert_eq!(io::ErrorKind::BrokenPipe, e.kind());
  } else {
    panic!("Connection should signal an error if input is closed during
            readline()");
  }
}

pub fn serve() -> Result<()> {
  let mut c = Connection::new();

  loop {
    let start_line = try!(c.readline());
    // Tolerate and skip blank lines between requests.
    if start_line.is_empty() { continue }

    println!("Got {}", start_line.len());
  }
}
