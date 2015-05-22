//! File access operations.

extern crate libc;
extern crate time;

use std::fs;
use std::path;
use std::io;

use super::unix;

/// Opens a file for read, but returns it only if its permissions and mode match
/// some seriously pedantic checks.  Otherwise, the file is immediately closed.
///
/// On success, returns the file along with some of the metadata retrieved
/// during the checks, as a useful side effect.
///
/// Analog of djb's `file_open` from `file.c`.
#[cfg_attr(test, allow(dead_code))]  // Uses filesystem, hard to unit test.
pub fn safe_open<P>(path: P) -> io::Result<OpenFile>
    where P: AsRef<path::Path> {
  let f = try!(fs::File::open(path));
  let s = try!(unix::fstat(&f));

  if (s.st_mode & 0o444) != 0o444 {
    Err(io::Error::new(io::ErrorKind::PermissionDenied, "not ugo+_r"))
  } else if (s.st_mode & 0o101) == 0o001 {
    Err(io::Error::new(io::ErrorKind::PermissionDenied, "o+x but u-x"))
  } else if (s.st_mode & libc::S_IFMT) != libc::S_IFREG {
    Err(io::Error::new(io::ErrorKind::PermissionDenied, "not a regular file"))
  } else {
    Ok(OpenFile {
      file: f,
      mtime: time::Timespec { sec: s.st_mtime, nsec: 0 },
      length: s.st_size as u64,
    })
  }
}

/// Result type for `safe_open`.
pub struct OpenFile {
  /// The opened file.
  pub file: fs::File,
  /// The file's modification time in seconds since the epoch, at the last time
  /// we checked.
  pub mtime: time::Timespec,
  /// The file's length, at the last time we checked.  Note that this may change
  /// at runtime; take care.
  pub length: u64,
}
