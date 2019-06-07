extern crate libc;

use std::env;
use std::io;
use std::process;

use std::str::FromStr;

mod ascii;
mod con;
mod error;
mod file;
mod filetype;
mod path;
mod percent;
mod request;
mod response;
mod server;
mod timeout;
mod unix;

/// Discards undesirable authority and calls through to the connection handler.
/// In this case, "undesirable authority" means:
/// - The global filesystem root (shed via `chroot`)
/// - The calling uid/gid and supplementary groups.
#[cfg_attr(test, allow(dead_code))]
fn main() {
    // Only chroot if a root directory is provided.  This allows for testing (most
    // of the) the daemon as an unprivileged user.
    if let Some(root) = env::args().nth(1) {
        if env::set_current_dir(&root).is_err() {
            process::exit(20)
        }
        if unix::chroot(root.as_bytes()).is_err() {
            process::exit(30)
        }
    }

    with_env_var("UID", unix::setuid);
    with_env_var("GID", |gid| {
        unix::setgroups(&[gid])?;
        unix::setgid(gid)
    });

    let remote = env::var("TCPREMOTEIP").unwrap_or_else(|_| "0".to_string());

    if server::serve(remote).is_err() {
        process::exit(40)
    }
}

fn with_env_var<V: FromStr, F>(var: &str, f: F)
where
    F: FnOnce(V) -> io::Result<()>,
{
    if let Ok(val_str) = env::var(var) {
        if let Ok(val) = FromStr::from_str(&val_str) {
            if f(val).is_err() {
                process::exit(30)
            }
        } else {
            process::exit(30)
        }
    }
}
