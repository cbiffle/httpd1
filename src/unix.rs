//! A thin veneer for libc et al.

extern crate libc;
extern crate time;

use std::fs;
use std::io;
use std::mem;

use std::os::unix::io::AsRawFd;
use std::os::unix::io::FromRawFd;

pub fn stdin() -> fs::File {
    unsafe { fs::File::from_raw_fd(0) }
}
pub fn stdout() -> fs::File {
    unsafe { fs::File::from_raw_fd(1) }
}
pub fn stderr() -> fs::File {
    unsafe { fs::File::from_raw_fd(2) }
}

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

/// Bring in some features not exposed in Rust's libc crate.
mod ffi {
    extern "C" {
        pub fn chroot(path: *const ::libc::c_char) -> ::libc::c_int;
        pub fn setgroups(size: ::libc::size_t, list: *const ::libc::gid_t) -> ::libc::c_int;
    }
}

pub fn chroot(path: &[u8]) -> io::Result<()> {
    cvt(unsafe { ffi::chroot(path.as_ptr() as *const libc::c_char) }).map(|_| ())
}

pub fn setgroups(groups: &[libc::gid_t]) -> io::Result<()> {
    cvt(unsafe { ffi::setgroups(groups.len() as libc::size_t, groups.as_ptr()) }).map(|_| ())
}

pub fn fstat(f: &fs::File) -> io::Result<libc::stat> {
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
#[cfg(test)]
pub fn pipe() -> io::Result<Pipe> {
    let mut fds = [0; 2];
    cvt(unsafe { libc::pipe(fds.as_mut_ptr()) }).map(|_| Pipe {
        input: unsafe { fs::File::from_raw_fd(fds[0]) },
        output: unsafe { fs::File::from_raw_fd(fds[1]) },
    })
}

#[cfg(test)]
pub struct Pipe {
    pub input: fs::File,
    pub output: fs::File,
}
