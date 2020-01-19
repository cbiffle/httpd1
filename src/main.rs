use std::str::FromStr;
use std::{env, process};

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
pub fn main() {
    // Only chroot if a root directory is provided.  This allows for testing (most
    // of the) the daemon as an unprivileged user.
    if let Some(root) = env::args().nth(1) {
        env::set_current_dir(&root)
            .map_err(|_| 20)
            .and_then(|_| nix::unistd::chroot(root.as_bytes()).map_err(|_| 30))
            .unwrap_or_else(|n| process::exit(n));
    }

    with_env_var("UID", |uid| {
        let uid = nix::unistd::Uid::from_raw(uid);
        nix::unistd::setuid(uid)
    });
    with_env_var("GID", |gid| {
        let gid = nix::unistd::Gid::from_raw(gid);
        nix::unistd::setgroups(&[gid])?;
        nix::unistd::setgid(gid)
    });

    let remote = env::var("TCPREMOTEIP").unwrap_or_else(|_| "0".to_string());

    server::serve(remote).unwrap_or_else(|_| process::exit(40))
}

fn with_env_var<V: FromStr, E>(var: &str, f: impl FnOnce(V) -> Result<(), E>) {
    if let Ok(val_str) = env::var(var) {
        V::from_str(&val_str)
            .map_err(|_| 30)
            .and_then(|val| f(val).map_err(|_| 30))
            .unwrap_or_else(|n| process::exit(n))
    }
}
