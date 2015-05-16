// Need libc to do unbuffered stdout/stdin :-/
#![feature(libc)]
extern crate libc;

use std::process;
use std::io;
use std::fs;
use std::mem;

use std::io::BufRead;
use std::io::Write;
use std::ascii::AsciiExt;
use std::iter::FromIterator;

pub mod unix;

#[derive(Debug)]
pub enum HttpError {
  ConnectionClosed,
  BadMethod,
  BadRequest,
  BadProtocol,
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
  match c.readline().err() {
    Some(HttpError::ConnectionClosed) => (),
    Some(_) => panic!("Unexpected error from readline() at stream end"),
    _ => panic!("readline() must fail at stream end"),
  };
}

pub fn serve() -> Result<()> {
  let mut c = Connection::new();

  loop {
    let start_line = try!(c.readline());
    // Tolerate and skip blank lines between requests.
    if start_line.is_empty() { continue }

    let (method, mut host, mut path, protocol) =
        try!(parse_start_line(start_line));

    println!("R: {:?} {:?} {:?} {:?}", method, host, path, protocol);
  }
}

#[derive(Debug)]
enum Method {
  Get,
  Head,
}

#[derive(Debug)]
enum Protocol {
  Http10,
  Http11,
}

fn indexof<T: PartialEq>(slice: &[T], item: T) -> usize {
  for i in 0..slice.len() {
    if slice[i] == item { return i }
  }
  slice.len()
}

fn parse_start_line(line: Vec<u8>)
    -> Result<(Method, Option<Vec<u8>>, Vec<u8>, Protocol)> {
  let parts: Vec<_> = line.splitn(3, |b| *b == b' ').collect();
  if parts.len() != 3 { return Err(HttpError::BadRequest) }

  let method = match parts[0] {
    b"GET" => Method::Get,
    b"HEAD" => Method::Head,
    _ => return Err(HttpError::BadMethod),
  };
  let (host, path) = {
    let raw = parts[1];
    if raw.len() >= 7 && raw[..7].eq_ignore_ascii_case(b"http://") {
      let no_scheme = &raw[7..];
      let slash = indexof(no_scheme, b'/');
      println!("slash = {}", slash);
      let host = Vec::from_iter(no_scheme[..slash].into_iter().cloned());
      let mut path = Vec::from_iter(no_scheme[slash..].into_iter().cloned());

      if path.is_empty() || path.ends_with(b"/") {
        path.extend(b"index.html".into_iter().cloned());
      }
      (Some(host), path) 
    } else {
      (None, Vec::from_iter(parts[1].into_iter().cloned()))
    }
  };
  let protocol = match parts[2] {
    b"HTTP/1.0" => Protocol::Http10,
    b"HTTP/1.1" => Protocol::Http11,
    _ => return Err(HttpError::BadProtocol),
  };

  Ok((method, host, path, protocol)) 
}
