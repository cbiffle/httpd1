//! A thin veneer for libc et al.

extern crate libc;

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::mem;

use std::os::unix::io::AsRawFd;
use std::os::unix::io::FromRawFd;

pub fn stdin()  -> fs::File { unsafe { fs::File::from_raw_fd(0) } }
pub fn stdout() -> fs::File { unsafe { fs::File::from_raw_fd(1) } }
pub fn stderr() -> fs::File { unsafe { fs::File::from_raw_fd(2) } }

/// Converts the Unix syscall convention of "-1 means error" to a Result.
/// Also corrects the range of the result, excluding negative values.
///
/// Implementation derived from code in std::sys::unix, which is unfortunately
/// private.  We'd really like to use Zero/One but they're unstable; unlike
/// libc, which seems pretty stable for being unstable, they may be a source of
/// churn.
fn cvt<T: Default + PartialOrd>(t: T) -> io::Result<T> {
  if t < T::default() {
    Err(io::Error::last_os_error())
  } else {
    Ok(t)
  }
}

/// Opens a file for read after somewhat pedantically verifying its permissions.
/// Returns the file, along with some of the metadata discovered during checks.
///
/// Analog of djb's `file_open` from `file.c`.  Performs essentially the same
/// tests, but assumes that files are never opened for write.
pub fn safe_open(path: &OsStr) -> io::Result<OpenFile> {
  // OsStr::from_bytes is specified as a no-cost cast operation on Unix.
  let f = try!(fs::File::open(path));
  let s = try!(fstat(&f));

  if (s.st_mode & 0o444) != 0o444 {
    Err(io::Error::new(io::ErrorKind::PermissionDenied, "not ugo+_r"))
  } else if (s.st_mode & 0o101) == 0o001 {
    Err(io::Error::new(io::ErrorKind::PermissionDenied, "o+x but u-x"))
  } else if (s.st_mode & libc::S_IFMT) != libc::S_IFREG {
    Err(io::Error::new(io::ErrorKind::PermissionDenied, "not a regular file"))
  } else {
    Ok(OpenFile {
      file: f,
      mtime: s.st_mtime,
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
  pub mtime: i64,
  /// The file's length, at the last time we checked.  Note that this may change
  /// at runtime; take care.
  pub length: u64,
}

/// Bring in some features not exposed in Rust's libc crate.
mod ffi {
  extern {
    pub fn chroot(path: *const ::libc::c_char) -> ::libc::c_int;
    pub fn setgroups(size: ::libc::size_t,
                     list: *const ::libc::gid_t) -> ::libc::c_int;
  }
}

pub fn chroot(path: &[u8]) -> io::Result<()> {
  cvt(unsafe { ffi::chroot(path.as_ptr() as *const libc::c_char) })
    .map(|_| ())
}

pub fn setgroups(groups: &[libc::gid_t]) -> io::Result<()> {
  cvt(unsafe { ffi::setgroups(groups.len() as libc::size_t, groups.as_ptr()) })
    .map(|_| ())
}

fn fstat(f: &fs::File) -> io::Result<libc::stat> {
  let fd = f.as_raw_fd();
  let mut s = unsafe { mem::zeroed() };
  cvt(unsafe { libc::fstat(fd, &mut s) }).map(|_| s)
}

pub fn setuid(uid: libc::uid_t) -> io::Result<()> {
  cvt(unsafe { libc::setuid(uid) }).map(|_| ())
}

pub fn setgid(gid: libc::gid_t) -> io::Result<()> {
  cvt(unsafe { libc::setgid(gid) }).map(|_| ())
}

/// Wraps POSIX pipe(2).  On success, returns a pair of Files that own the
/// pipe's file descriptors.
pub fn pipe() -> io::Result<Pipe> {
  let mut fds = [0; 2];
  cvt(unsafe { libc::pipe(fds.as_mut_ptr()) })
    .map(|_| Pipe {
      input: unsafe { fs::File::from_raw_fd(fds[0]) },
      output: unsafe { fs::File::from_raw_fd(fds[1]) },
    })
}

pub struct Pipe {
  pub input: fs::File,
  pub output: fs::File,
}
