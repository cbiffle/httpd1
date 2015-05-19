//! IO operations with timeout support.

use std::io;
use std::fs;
use libc::time_t;

use std::os::unix::io::AsRawFd;

mod ffi {
  use libc::{c_int, time_t};

  #[link(name="timeout", kind="static")]
  extern {
    pub fn wait_for_data(fd: c_int,
                         seconds: time_t)
                         -> c_int;
  }
}

/// A trait for objects that can produce data, but not all the time.  This trait
/// would typically be combined with `std::io::Read`.
trait ReadTimeout {
  /// Waits until at least some data is available from this object, up to the
  /// specified number of seconds.  If time elapses, this returns an error.
  fn wait_for_data(&mut self, seconds: u32) -> io::Result<()>;
}

impl ReadTimeout for fs::File {
  fn wait_for_data(&mut self, seconds: u32) -> io::Result<()> {
    if unsafe { ffi::wait_for_data(self.as_raw_fd(), seconds as time_t) } == 0 {
      Ok(())
    } else {
      Err(io::Error::last_os_error())
    }
  }
}

impl<T> ReadTimeout for io::BufReader<T>
    where T: ReadTimeout + io::Read {
  fn wait_for_data(&mut self, seconds: u32) -> io::Result<()> {
    self.get_mut().wait_for_data(seconds)
  }
}
