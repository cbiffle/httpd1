//! A thin veneer for libc et al.

use std::fs;
use std::io;

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

pub fn chroot(path: &[u8]) -> io::Result<()> {
    cvt(unsafe { libc::chroot(path.as_ptr() as *const libc::c_char) })
        .map(|_| ())
}

pub fn setgroups(groups: &[libc::gid_t]) -> io::Result<()> {
    cvt(unsafe {
        libc::setgroups(groups.len() as libc::size_t, groups.as_ptr())
    })
    .map(|_| ())
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
