//! A thin veneer for libc et al.

use std::fs;
use std::os::unix::io::{AsRawFd, FromRawFd};

pub fn stdin() -> fs::File {
    let s = std::io::stdin();
    let s = s.lock();
    let fd = s.as_raw_fd();
    std::mem::forget(s);
    unsafe { fs::File::from_raw_fd(fd) }
}

pub fn stdout() -> fs::File {
    let s = std::io::stdout();
    let s = s.lock();
    let fd = s.as_raw_fd();
    std::mem::forget(s);
    unsafe { fs::File::from_raw_fd(fd) }
}

pub fn stderr() -> fs::File {
    let s = std::io::stderr();
    let s = s.lock();
    let fd = s.as_raw_fd();
    std::mem::forget(s);
    unsafe { fs::File::from_raw_fd(fd) }
}

/// Wraps POSIX pipe(2).  On success, returns a pair of Files that own the
/// pipe's file descriptors.
#[cfg(test)]
pub fn pipe() -> nix::Result<Pipe> {
    let (fd0, fd1) = nix::unistd::pipe()?;
    Ok(Pipe {
        input: unsafe { fs::File::from_raw_fd(fd0) },
        output: unsafe { fs::File::from_raw_fd(fd1) },
    })
}

#[cfg(test)]
pub struct Pipe {
    pub input: fs::File,
    pub output: fs::File,
}
