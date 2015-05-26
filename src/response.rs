//! HTTP response support.

extern crate time;

use std::io;
use std::io::BufRead;

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
  let (code, message) = match error.status() {
    None => return Ok(()),
    Some(pair) => pair,
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

/// Sends a permanent redirect to the client.  The connection stays open.
pub fn redirect(con: &mut Connection,
                protocol: Protocol,
                send_content: bool,
                location: &[u8])
                -> Result<()> {
  let body = b"<html><body>moved permanently</body></html>";

  let now = time::get_time();
  try!(start_response(con, protocol, &now, b"301", b"moved permanently"));
  try!(con.write(b"Content-Length: "));
  try!(con.write_to_string(body.len()));
  try!(con.write(b"\r\nLocation: "));
  try!(con.write(location));
  try!(con.write(b"\r\n"));

  try!(con.write(b"Content-Type: text/html\r\n\r\n"));

  if send_content {
    try!(con.write(body));
  }

  try!(con.flush_output());

  match protocol {
    Protocol::Http10 => Err(HttpError::ConnectionClosed),
    Protocol::Http11 => Ok(()),
  }
}

fn send_unencoded(con: &mut Connection,
                  send_content: bool,
                  resource: OpenFile) -> Result<()> {
  try!(con.write(b"Content-Length: "));
  try!(con.write_to_string(resource.length));
  try!(con.write(b"\r\n\r\n"));

  if send_content {
    let mut input = io::BufReader::with_capacity(1024, resource.file);
    loop {
      let count = {
        let chunk = try!(input.fill_buf());
        if chunk.is_empty() { break }
        try!(con.write(chunk));
        chunk.len()
      };
      input.consume(count);
    }
  }
  
  // We use unencoded responses for HTTP/1.0 clients, and we assume that
  // they don't use persistent connections.  This merits reconsideration (TODO).
  Err(HttpError::ConnectionClosed)
}

fn send_chunked(con: &mut Connection,
                send_content: bool,
                resource: OpenFile) -> Result<()> {
  try!(con.write(b"Transfer-Encoding: chunked\r\n\r\n"));

  if send_content {
    let mut input = io::BufReader::with_capacity(1024, resource.file);
    loop {
      let count = {
        let chunk = try!(input.fill_buf());
        try!(con.write_hex(chunk.len()));
        try!(con.write(b"\r\n"));
        try!(con.write(chunk));
        try!(con.write(b"\r\n"));

        chunk.len()
      };
      if count == 0 { break }  // End of transfer.

      input.consume(count)
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


