// Need libc to do unbuffered stdout/stdin :-/
#![feature(libc)]
extern crate libc;
extern crate time;

use std::io;
use std::ffi;

use std::io::Write;
use std::ascii::AsciiExt;
use std::os::unix::ffi::OsStringExt;
use std::io::Read;

pub mod unix;
mod ascii;
mod filetype;
mod timeout;
mod con;
mod error;
mod path;
mod percent;
mod request;

use self::error::*;
use self::con::Connection;  // interesting, wildcard doesn't work for this.
use self::request::{Method, Protocol, Request};

pub fn serve() -> Result<()> {
  let mut c = Connection::new();

  loop {  // Process requests.
    let req = try!(request::take_request(&mut c));

    // Back up two pieces before we consume the request.
    let protocol = req.protocol;
    let method = req.method;

    if let Some(error) = serve_request(&mut c, req).err() {
      // Try to report this to the client.  Error reporting is best-effort.
      let _ = barf(&mut c, Some(protocol), (method == Method::Get), error);
      return Ok(())
    }

    // Otherwise, carry on accepting requests.
  }
}

/// Signals the given error to the client.
///
/// Currently, this also closes the connection, though this seems like a
/// decision better left to the caller (TODO).
fn barf(con: &mut Connection,
        protocol: Option<Protocol>,
        send_content: bool,
        error: HttpError)
        -> Result<()> {
  let (code, message): (&[u8], &[u8]) = match error {
    HttpError::IoError(ioe) => match ioe.kind() {
      io::ErrorKind::NotFound
        | io::ErrorKind::PermissionDenied
          => (b"404", b"not found"),
      _ => (b"500", b"error"),
    },

    HttpError::ConnectionClosed => return Ok(()),

    HttpError::BadMethod => (b"501", b"method not implemented"),
    HttpError::BadRequest => (b"400", b"bad request"),
    HttpError::BadProtocol => (b"505", b"HTTP version not supported"),
    HttpError::SpanishInquisition => (b"417", b"expectations not supported"),
    HttpError::PreconditionFailed => (b"412", b"bad precondition"),
    
    HttpError::NotImplemented(m) => (b"501", m),
  };

  let now = time::get_time();
  try!(start_response(con, protocol.unwrap_or(Protocol::Http10), &now,
                      code, message));
  try!(con.write(b"Content-Length: "));
  try!(con.write_to_string(message.len() + 26));  // length of HTML below
  try!(con.write(b"\r\n"));

  if protocol == Some(Protocol::Http11) {
    try!(con.write(b"Connection: close\r\n"));
  }

  try!(con.write(b"Content-Type: text/html\r\n\r\n"));

  if send_content {
    try!(con.write(b"<html><body>"));
    try!(con.write(message));
    try!(con.write(b"</body></html>"));
  }

  con.flush_output()
}

fn serve_request(con: &mut Connection, req: Request) -> Result<()> {
  let host = match req.host {
    None => match req.protocol {
      // HTTP 1.1 requests must include a host, one way or another.
      Protocol::Http11 => return Err(HttpError::BadRequest),
      // For HTTP/1.0 without a host, substitute the name "0".
      Protocol::Http10 => vec![b'0'],
    },
    Some(mut h) => {
      for c in h.iter_mut() {
        *c = (*c).to_ascii_lowercase()
      }
      // TODO: host should be parsed during request processing.
      let n = indexof(&h, b':');
      h.truncate(n);
      h
    },
  };

  let mut path = req.path;
  try!(percent::unescape(&mut path));

  let file_path = path::sanitize(
    b"./".iter()
      .chain(host.iter())
      .chain(b"/".iter())
      .chain(path.iter())
      .cloned());

  let content_type = filetype::filetype(&file_path[..]);

  let file_path = ffi::OsString::from_vec(file_path);
  let resource = try!(unix::safe_open(&file_path));

  let now = time::get_time();

  try!(start_response(con, req.protocol, &now, b"200", b"OK"));
  try!(con.write(b"Content-Type: "));
  try!(con.write(&content_type[..]));
  try!(con.write(b"\r\n"));

  let mtime = format!("{}", time::at_utc(resource.mtime).rfc822());
  try!(con.write(b"Last-Modified: "));
  try!(con.write(mtime.as_bytes()));
  try!(con.write(b"\r\n"));

  let r = match req.protocol {
    Protocol::Http10 => serve_request_unencoded(con, req.method, resource),
    Protocol::Http11 => serve_request_chunked(con, req.method, resource),
  };

  try!(con.flush_output());
  r
}

fn serve_request_unencoded(con: &mut Connection,
                           method: Method,
                           mut resource: unix::OpenFile) -> Result<()> {
  try!(con.write(b"Content-Length: "));
  try!(con.write_to_string(resource.length));
  try!(con.write(b"\r\n\r\n"));

  if method == Method::Get {
    loop {
      let count = try!(resource.file.read(&mut con.buf[..]));
      if count == 0 { break }
      try!(con.write_buf(count))
    }
  }
  
  // We use unencoded responses for HTTP/1.0 clients, and we assume that
  // they don't use persistent connections.  This merits reconsideration (TODO).
  Err(HttpError::ConnectionClosed)
}

fn serve_request_chunked(con: &mut Connection,
                         method: Method,
                         mut resource: unix::OpenFile) -> Result<()> {
  try!(con.write(b"Transfer-Encoding: chunked\r\n\r\n"));

  if method == Method::Get {
    loop {
      let count = try!(resource.file.read(&mut con.buf[..]));
      try!(con.write_hex(count));
      try!(con.write(b"\r\n"));
      try!(con.write_buf(count));
      try!(con.write(b"\r\n"));

      if count == 0 { break }  // End of transfer.
    }
  }

  // Leave the connection open for more requests.
  Ok(())
}

/// Begins a response, printing the status line and a set of common headers.
/// The caller should follow up by adding any desired headers and then writing
/// a CRLF.
fn start_response(con: &mut Connection,
                  prot: Protocol,
                  now: &time::Timespec,
                  code: &[u8],
                  msg: &[u8])
                  -> Result<()> {

  let now = format!("{}", time::at_utc(*now).rfc822());

  try!(con.write(match prot {
    Protocol::Http10 => b"HTTP/1.0 ",
    Protocol::Http11 => b"HTTP/1.1 ",
  }));
  try!(con.write(code));
  try!(con.write(b" "));
  try!(con.write(msg));
  try!(con.write(b"\r\nServer: abstract screaming\r\nDate: "));
  try!(con.write(now.as_bytes()));
  try!(con.write(b"\r\n"));
  // TODO date
  Ok(())
}

fn indexof<T: PartialEq>(slice: &[T], item: T) -> usize {
  for i in 0..slice.len() {
    if slice[i] == item { return i }
  }
  slice.len()
}
