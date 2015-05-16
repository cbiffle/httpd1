// Need libc to do unbuffered stdout/stdin :-/
#![feature(libc)]
extern crate libc;

use std::process;
use std::io;
use std::fs;

use std::io::BufRead;

pub mod unix;

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

  fn readline(&mut self) -> io::Result<Vec<u8>> {
    // We can use read_until to find the next newline, and then look behind it
    // to figure out whether we're done.
    let mut line = Vec::new();
    loop {
      match try!(self.input.read_until(b'\n', &mut line)) {
        0 => return Err(io::Error::new(io::ErrorKind::BrokenPipe,
                                       "request line not terminated")),
        _ => {
          if line.ends_with(b"\r\n") { return Ok(line) }
        }
      }
    }
  }
}

pub fn serve() -> io::Result<()> {
  let mut c = Connection::new();

  loop {
    let start_line = try!(c.readline());
    println!("Got {}", start_line.len());
  }
}
