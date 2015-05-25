//! HTTP response support.

extern crate time;

use std::io::Read;

use super::request::{Method, Protocol};
use super::con::Connection;
use super::file::OpenFile;
use super::error::{HttpError, Result};

pub enum ContentEncoding {
  Gzip,
}

pub fn send(con: &mut Connection,
            method: Method,
            protocol: Protocol,
            now: time::Timespec,
            encoding: Option<ContentEncoding>,
            if_modified_since: Option<Vec<u8>>,
            content_type: &[u8],
            resource: OpenFile)
            -> Result<()> {
  let mtime = format!("{}", time::at_utc(resource.mtime).rfc822());

  let unmodified = if let Some(ref ims) = if_modified_since {
    &ims[..] == mtime.as_bytes()
  } else {
    false
  };

  if unmodified {
    con.log_other(b"note: not modified");
    try!(start_response(con, protocol, &now, b"304", b"not modified"))
  } else {
    try!(start_response(con, protocol, &now, b"200", b"OK"))
  }
  try!(con.write(b"Content-Type: "));
  try!(con.write(content_type));
  try!(con.write(b"\r\n"));

  try!(con.write(b"Last-Modified: "));
  try!(con.write(mtime.as_bytes()));
  try!(con.write(b"\r\n"));

  match encoding {
    None => (),
    Some(ContentEncoding::Gzip) => {
      try!(con.write(b"Content-Encoding: gzip\r\n"));
    },
  }

  let send_content = method == Method::Get && !unmodified;

  let r = match protocol {
    Protocol::Http10 => send_unencoded(con, send_content, resource),
    Protocol::Http11 => send_chunked(con, send_content, resource),
  };

  try!(con.flush_output());
  r
}

/// Signals the given error to the client.
///
/// Currently, this also closes the connection, though this seems like a
/// decision better left to the caller (TODO).
pub fn barf(con: &mut Connection,
            protocol: Option<Protocol>,
            send_content: bool,
            error: HttpError)
            -> Result<()> {
  let (code, message): (&[u8], &[u8]) = match error {
    HttpError::IoError(_) =>(b"500", b"error"),

    HttpError::ConnectionClosed => return Ok(()),

    HttpError::BadMethod => (b"501", b"method not implemented"),
    HttpError::BadRequest => (b"400", b"bad request"),
    HttpError::BadProtocol => (b"505", b"HTTP version not supported"),
    HttpError::SpanishInquisition => (b"417", b"expectations not supported"),
    HttpError::PreconditionFailed => (b"412", b"bad precondition"),
    HttpError::NotFound => (b"404", b"not found"),
    
    HttpError::NotImplemented(m) => (b"501", m),
  };

  let now = time::get_time();
  try!(start_response(con, protocol.unwrap_or(Protocol::Http10), &now,
                      code, message));
  try!(con.write(b"Content-Length: "));
  try!(con.write_to_string(message.len() + 28));  // length of HTML wrapper
  try!(con.write(b"\r\n"));

  if protocol == Some(Protocol::Http11) {
    try!(con.write(b"Connection: close\r\n"));
  }

  try!(con.write(b"Content-Type: text/html\r\n\r\n"));

  if send_content {
    try!(con.write(b"<html><body>"));
    try!(con.write(message));
    try!(con.write(b"</body></html>\r\n"));
  }

  con.flush_output()
}

fn send_unencoded(con: &mut Connection,
                  send_content: bool,
                  mut resource: OpenFile) -> Result<()> {
  try!(con.write(b"Content-Length: "));
  try!(con.write_to_string(resource.length));
  try!(con.write(b"\r\n\r\n"));

  if send_content {
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

fn send_chunked(con: &mut Connection,
                send_content: bool,
                mut resource: OpenFile) -> Result<()> {
  try!(con.write(b"Transfer-Encoding: chunked\r\n\r\n"));

  if send_content {
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


