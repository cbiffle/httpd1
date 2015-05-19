use std::env;
use std::ffi::OsString;

use std::os::unix::ffi::OsStringExt;
use std::iter::FromIterator;

pub fn filetype(file_path: &[u8]) -> Vec<u8> {
  match file_path.rsplitn(2, |b| *b == b'.').next() {
    Some(ext) => env_mapping(ext)
                   .unwrap_or_else(|| canned_mapping(ext)),

    // TODO: a path without an extension should perhaps not be served as
    // text/plain?
    _ => b"text/plain".iter().cloned().collect(),
  }
}

fn canned_mapping(ext: &[u8]) -> Vec<u8> {
  let mimetype: &[u8] = match ext {
    b"html"           => b"text/html",
    b"gif"            => b"image/gif",
    b"jpeg" | b"jpg"  => b"image/jpeg",
    b"png"            => b"image/png",
    b"pdf"            => b"application/pdf",
    b"css"            => b"text/css",
    _ => b"text/plain",
  };
  mimetype.iter().cloned().collect()
}

fn env_mapping(ext: &[u8]) -> Option<Vec<u8>> {
  let key = Vec::from_iter(
      b"CT_".iter()
      .chain(ext.iter())
      .cloned());
  env::var_os(OsString::from_vec(key)).map(|s| s.into_vec())
}
