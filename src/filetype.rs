//! Guessing the MIME type of files in inexpensive ways.

use std::env;
use std::ffi::OsString;

use std::os::unix::ffi::OsStringExt;
use std::iter::FromIterator;

/// Takes a guess at a file's MIME type using its file extension.
///
/// The extension is the sequence of bytes after the last period, so we can't
/// ascribe unique MIME types to things like `.tar.gz`.
///
/// For a file `foo.ext`, we'll first search for an environment variable called
/// `CT_ext`.  If present, its contents will be returned as the MIME type.
///
/// If no such environment variable exists, a hardcoded mapping of common
/// extensions will be consulted.
///
/// Either way, a new Vec containing the MIME type will be allocated and
/// returned.  We could technically use static byte slices, since the contents
/// of environment variables are in RAM with static duration on Unix, but Rust
/// doesn't present them that way -- probably some Windows thing.
pub fn from_path(file_path: &[u8]) -> Vec<u8> {
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

#[cfg(test)]
mod tests {
  use super::from_path;

  macro_rules! from_path_case {
    ($name: ident, $input: expr, $output: expr) => {
      #[test]
      fn $name() {
        assert_eq!($output, &from_path($input)[..])
      }
    };
  }

  from_path_case!(test_no_extension, b"foobar", b"text/plain");
  from_path_case!(test_canned, b"foobar.css", b"text/css");
  // Deliberately *not* exercising the complete canned mapping.
}
