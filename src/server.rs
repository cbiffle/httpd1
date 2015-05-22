//! The core HTTP server, which ties the other modules together.

extern crate time;

use std::ffi;

use std::ascii::AsciiExt;
use std::os::unix::ffi::OsStrExt;
use std::io::Read;
use std::error::Error;

use super::error::*;
use super::con::Connection;  // interesting, wildcard doesn't work for this.
use super::request::{Method, Protocol, Request};
use super::{request,response,percent,path,filetype,unix};

pub fn serve(remote: String) -> Result<()> {
  let mut c = Connection::new(remote);

  loop {  // Process requests.
    let req = match request::read(&mut c) {
      Ok(r) => r,
      Err(e) => return response::barf(&mut c, None, true, e),
    };

    // Back up two pieces before we consume the request.
    let protocol = req.protocol;
    let method = req.method;

    if let Some(error) = serve_request(&mut c, req).err() {
      // Try to report this to the client.  Error reporting is best-effort.
      let _ = response::barf(&mut c, Some(protocol), (method == Method::Get),
                             error);
      return Ok(())
    }

    // Otherwise, carry on accepting requests.
  }
}

fn serve_request(con: &mut Connection, req: Request) -> Result<()> {
  // The request may not have included a Host, but we need to use it to
  // generate a file path.  Tolerate Host's absence for HTTP/1.0 requests
  // by replacing it with the simulated host "0".
  let host = match req.host {
    None => match req.protocol {
      Protocol::Http10 => vec![b'0'],

      // HTTP 1.1 requests must include a host, one way or another.
      Protocol::Http11 => return Err(HttpError::BadRequest),
    },
    Some(mut h) => {
      normalize_host(&mut h);
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

  let content_type = filetype::from_path(&file_path[..]);
  let resource = try!(open_resource(con, &file_path[..]));
  let now = time::get_time();

  response::send(con, req.method, req.protocol, now,
                 req.if_modified_since, &content_type[..], resource)
}

fn open_resource(con: &mut Connection, path: &[u8]) -> Result<unix::OpenFile> {
  match unix::safe_open(ffi::OsStr::from_bytes(path)) {
    Ok(r) => {
      con.log(path, b"success");
      Ok(r)
    },

    Err(e) => {
      con.log(path, e.description().as_bytes());
      Err(HttpError::IoError(e))
    },
  }
}

// If the client provided a host, we must normalize it for use as a directory
// name: downcase it and strip off the port, if any.
fn normalize_host(host: &mut Vec<u8>) {
  for i in 0..host.len() {
    let c = host[i];
    if c == b':' {
      host.truncate(i);
      return
    } else {
      host[i] = c.to_ascii_lowercase()
    }
  }
}
