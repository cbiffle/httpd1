//! IO operations with timeout support.
use libc::time_t;
use std::fs;
use std::io;

use std::os::unix::io::AsRawFd;

mod ffi {
    use libc::{c_int, time_t};

    extern "C" {
        pub fn wait_for_data(fd: c_int, seconds: time_t) -> c_int;

        pub fn wait_for_writeable(fd: c_int, seconds: time_t) -> c_int;
    }
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
        if unsafe { ffi::wait_for_data(self.as_raw_fd(), seconds as time_t) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
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
        let r = unsafe { ffi::wait_for_writeable(self.as_raw_fd(), seconds as time_t) };

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
