// Need libc to do unbuffered stdout/stdin :-/
#![feature(libc)]
extern crate libc;
extern crate time;

use std::io;
use std::ffi;

use std::io::Write;
use std::ascii::AsciiExt;
use std::iter::FromIterator;
use std::os::unix::ffi::OsStringExt;
use std::io::Read;

pub mod unix;
mod filetype;
mod timeout;
mod con;
mod error;
mod path;
mod percent;

use self::error::*;
use self::con::Connection;  // interesting, wildcard doesn't work for this.

pub fn serve() -> Result<()> {
  let mut c = Connection::new();

  loop {  // Process requests.
    let start_line = try!(c.readline());
    // Tolerate and skip blank lines between requests.
    if start_line.is_empty() { continue }

    let mut req = match parse_start_line(start_line) {
      Err(e) => return barf(&mut c, None, true, e),
      Ok(r) => r,
    };

    let mut hdr = Vec::new();
    loop {  // Process request headers.
      // Requests headers are slightly complicated because they can be broken
      // over multiple lines using indentation.
      let hdr_line = try!(c.readline());

      if hdr_line.is_empty() || !is_http_ws(hdr_line[0]) {
        // At an empty line or a line beginning with non-whitespace, we know
        // we have received the entirety of the *previous* header and can
        // process it.
        if starts_with_ignore_ascii_case(&hdr[..], b"content-length:")
            || starts_with_ignore_ascii_case(&hdr[..], b"transfer-encoding:") {
          return Err(HttpError::NotImplemented(b"I can't receive messages"))
        }
        if starts_with_ignore_ascii_case(&hdr[..], b"expect") {
          return Err(HttpError::SpanishInquisition)
        }
        if starts_with_ignore_ascii_case(&hdr[..], b"if-match")
            || starts_with_ignore_ascii_case(&hdr[..], b"if-unmodified-since") {
          return Err(HttpError::PreconditionFailed)
        }

        if starts_with_ignore_ascii_case(&hdr[..], b"host") {
          // Only accept a host from the request headers if none was provided
          // in the start line.
          if req.host.is_none() {
            // Just drop whitespace characters from the host header.  This
            // questionable interpretation of the spec mimics publicfile.
            let new_host = Vec::from_iter(hdr[5..].iter().cloned()
                .filter(|b| !is_http_ws(*b)));
            if !new_host.is_empty() { req.host = Some(new_host) }
          }
        }

        if starts_with_ignore_ascii_case(&hdr[..], b"if-modified-since") {
          req.if_modified_since =
              Some(Vec::from_iter(hdr[18..].iter().cloned()
                                    .skip_while(|b| is_http_ws(*b))));
        }

        // We've processed this header -- discard it.
        hdr.clear();
      }

      if hdr_line.is_empty() { break }

      hdr.extend(hdr_line);
    }

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
    try!(con.write(b"</html></body>"));
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

fn is_http_ws(c: u8) -> bool {
  c == b' ' || c == b'\t'
}

#[derive(Debug, PartialEq, Copy, Clone)]
enum Method {
  Get,
  Head,
}

#[derive(Debug, PartialEq, Copy, Clone)]
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

fn parse_start_line(line: Vec<u8>) -> Result<Request> {
  let parts: Vec<_> = line.splitn(3, |b| *b == b' ').collect();
  if parts.len() != 3 { return Err(HttpError::BadRequest) }

  let method = match parts[0] {
    b"GET" => Method::Get,
    b"HEAD" => Method::Head,
    _ => return Err(HttpError::BadMethod),
  };
  let (host, mut path) = {
    let raw = parts[1];
    // Distinguish an old-style path-only request from a HTTP/1.1-style URL
    // request by checking for the presence of an HTTP scheme.
    if raw.len() >= 7 && raw[..7].eq_ignore_ascii_case(b"http://") {
      // Split the remainder at the first slash.  The bytes to the left are the
      // host name; to the right, including the delimiter, the path.
      let (host, path) = raw[7..].split_at(indexof(&raw[7..], b'/'));
      let host = Vec::from_iter(host.into_iter().cloned());
      let path = Vec::from_iter(path.into_iter().cloned());

      if host.is_empty() {
        // The client can totally specify an "empty host" using a URL of the
        // form `http:///foo`.  We are not amused, and treat this as an absent
        // host specification.
        (None, path)
      } else {
        (Some(host), path) 
      }
    } else {
      (None, Vec::from_iter(parts[1].into_iter().cloned()))
    }
  };
  let protocol = match parts[2] {
    b"HTTP/1.0" => Protocol::Http10,
    b"HTTP/1.1" => Protocol::Http11,
    _ => return Err(HttpError::BadProtocol),
  };

  // Slap an 'index.html' onto the end of any path that, from simple textual
  // inspection, ends in a directory.
  if path.is_empty() || path.ends_with(b"/") {
    path.extend(b"index.html".into_iter().cloned());
  }

  Ok(Request {
    method: method,
    protocol: protocol,
    host: host,
    path: path,
    if_modified_since: None,  // Filled in later.
  }) 
}

#[derive(Debug)]
struct Request {
  method: Method,
  protocol: Protocol,
  host: Option<Vec<u8>>,
  path: Vec<u8>,
  if_modified_since: Option<Vec<u8>>,
}

fn starts_with_ignore_ascii_case(v: &[u8], prefix: &[u8]) -> bool {
  if v.len() < prefix.len() {
    false
  } else {
    v[..prefix.len()].eq_ignore_ascii_case(prefix)
  }
}

#[test]
fn test_starts_with_ignore_ascii_case() {
  assert_eq!(true, starts_with_ignore_ascii_case(b"", b""));
  assert_eq!(true, starts_with_ignore_ascii_case(b"foobar", b"foo"));
  assert_eq!(true, starts_with_ignore_ascii_case(b"FOOBAR", b"foo"));

  assert_eq!(false, starts_with_ignore_ascii_case(b"foo", b"foobar"));
  assert_eq!(false, starts_with_ignore_ascii_case(b"", b"foobar"));
}
