//! A thin veneer for libc et al.

use std::fs;

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
