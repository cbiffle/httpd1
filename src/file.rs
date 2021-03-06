//! File access operations.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path;
use std::time::SystemTime;

use crate::error;

/// Opens a file for read, but returns it only if its permissions and mode match
/// some seriously pedantic checks.  Otherwise, the file is immediately closed.
///
/// On success, returns the file along with some of the metadata retrieved
/// during the checks, as a useful side effect.
///
/// Analog of djb's `file_open` from `file.c`.
pub fn safe_open<P>(path: P) -> error::Result<FileOrDir>
where
    P: AsRef<path::Path>,
{
    let f = fs::File::open(path)?;
    let meta = f.metadata()?;

    if (meta.mode() & 0o444) != 0o444 {
        Err(error::HttpError::NotFound(b"not ugo+r"))
    } else if (meta.mode() & 0o101) == 0o001 {
        Err(error::HttpError::NotFound(b"o+x but u-x"))
    } else if meta.is_dir() {
        Ok(FileOrDir::Dir)
    } else if meta.is_file() {
        Ok(FileOrDir::File(OpenFile {
            file: f,
            mtime: meta.modified()?,
            length: meta.len(),
        }))
    } else {
        Err(error::HttpError::NotFound(b"not a regular file"))
    }
}

/// Used to represent the result of opening a path, which might have turned out
/// to be a directory.
pub enum FileOrDir {
    File(OpenFile),
    Dir,
}

/// Result type for `safe_open`.
pub struct OpenFile {
    /// The opened file.
    pub file: fs::File,
    /// The file's modification time in seconds since the epoch, at the last time
    /// we checked.
    pub mtime: SystemTime,
    /// The file's length, at the last time we checked.  Note that this may change
    /// at runtime; take care.
    pub length: u64,
}
