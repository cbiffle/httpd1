extern crate time;

use std::ffi;

use std::ascii::AsciiExt;
use std::os::unix::ffi::OsStringExt;
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

  let content_type = filetype::from_path(&file_path[..]);

  // We take this silly round-trip through OsString because we can't ensure
  // that the path is valid UTF-8, so we can't hit str/String safely.
  // Fortunately it's merely an elaborate typecast on Unix.
  let file_path = ffi::OsString::from_vec(file_path);
  let resource = match unix::safe_open(&file_path) {
    Ok(r) => {
      let file_path = file_path.into_vec();
      con.log(&file_path[..], b"success");
      r
    },

    Err(e) => {
      let file_path = file_path.into_vec();
      con.log(&file_path[..], e.description().as_bytes());
      return Err(HttpError::IoError(e))
    },
  };

  let now = time::get_time();

  response::send(con, req.method, req.protocol, now,
                 req.if_modified_since, &content_type[..], resource)
}

fn indexof<T: PartialEq>(slice: &[T], item: T) -> usize {
  for i in 0..slice.len() {
    if slice[i] == item { return i }
  }
  slice.len()
}
