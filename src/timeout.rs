//! IO operations with timeout support.
use libc::time_t;
use std::fs;
use std::io;

use std::os::unix::io::AsRawFd;

mod ffi {
    use std::mem::MaybeUninit;
    use std::ptr::null_mut;
    use libc::{c_int, time_t};

    pub fn wait_for_data(fd: c_int, seconds: time_t) -> nix::Result<()> {
        use nix::sys::select::{select, FdSet};
        use nix::sys::time::TimeVal;

        let mut tv = TimeVal::from(libc::timeval {
            tv_sec: seconds,
            tv_usec: 0,
        });

        let mut fds = FdSet::new();
        fds.insert(fd);

        select(
            fd + 1,
            Some(&mut fds),
            None,
            None,
            Some(&mut tv),
        )?;
        if !fds.contains(fd) {
            return Err(nix::errno::Errno::ETIMEDOUT.into());
        }
        Ok(())
    }

    pub unsafe fn wait_for_writeable(fd: c_int, seconds: time_t) -> c_int {
        let mut tv = libc::timeval {
            tv_sec: seconds,
            tv_usec: 0,
        };

        let mut fds = MaybeUninit::uninit();
        libc::FD_ZERO(fds.as_mut_ptr());
        let fds = fds.as_mut_ptr();
        libc::FD_SET(fd, fds);

        if libc::select(fd + 1, null_mut(), fds, null_mut(), &mut tv) == -1 {
            return -1;
        }
        if !libc::FD_ISSET(fd, fds) {
            return -1;
        }

        0
    }
}

fn cvt_err(e: nix::Error) -> io::Error {
    io::Error::new(
        io::ErrorKind::Other,
        format!("{}", e),
    )
}

/// A trait for objects that can produce data, but not all the time.  This trait
/// would typically be combined with `std::io::Read`.
trait ReadTimeout {
    /// Waits until at least some data is available from this object, up to the
    /// specified number of seconds.  If time elapses, this returns an error.
    fn wait_for_data(&mut self, seconds: u32) -> io::Result<()>;
}

/// A trait for objects that can consume data, but not all the time.  This trait
/// would typically be combined with `std::io::Write`.
trait WriteTimeout {
    /// Waits until at least some data can be written to this object, up to the
    /// specified number of seconds.  If time elapses, this returns an error.
    fn wait_for_writeable(&mut self, seconds: u32) -> io::Result<()>;
}

impl ReadTimeout for fs::File {
    fn wait_for_data(&mut self, seconds: u32) -> io::Result<()> {
        ffi::wait_for_data(self.as_raw_fd(), seconds as time_t)
            .map_err(cvt_err)
    }
}

impl<T> ReadTimeout for io::BufReader<T>
where
    T: ReadTimeout + io::Read,
{
    fn wait_for_data(&mut self, seconds: u32) -> io::Result<()> {
        self.get_mut().wait_for_data(seconds)
    }
}

impl WriteTimeout for fs::File {
    fn wait_for_writeable(&mut self, seconds: u32) -> io::Result<()> {
        let r = unsafe {
            ffi::wait_for_writeable(self.as_raw_fd(), seconds as time_t)
        };

        if r == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

impl<T> WriteTimeout for io::BufWriter<T>
where
    T: WriteTimeout + io::Write,
{
    fn wait_for_writeable(&mut self, seconds: u32) -> io::Result<()> {
        self.get_mut().wait_for_writeable(seconds)
    }
}

/// A wrapper for `File` that ensures that all read operations are done under
/// a (fixed) timeout.
pub struct SafeFile(fs::File);

impl SafeFile {
    pub fn new(inner: fs::File) -> Self {
        SafeFile(inner)
    }
}

impl io::Read for SafeFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.wait_for_data(60).and_then(|_| self.0.read(buf))
    }
}

impl io::Write for SafeFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .wait_for_writeable(60)
            .and_then(|_| self.0.write(buf))
    }

    fn flush(&mut self) -> io::Result<()> {
        // On Unix, at least, flushing a raw File is a no-op -- so no timeout
        // is required here.  Flushing a buffered writer will hit the write
        // timeout, above.
        self.0.flush()
    }
}
