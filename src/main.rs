#![feature(libc)]
extern crate libc;
extern crate httpd;

use std::env;
use std::process;
use std::io;
use httpd::unix;

use std::str::FromStr;

/// Discards undesirable authority and calls through to the connection handler.
/// In this case, "undesirable authority" means:
/// - The global filesystem root (shed via `chroot`)
/// - The calling uid/gid and supplementary groups.
fn main() {
  // Only chroot if a root directory is provided.  This behavior mimics
  // publicfile, but seems suspicious to me.  (TODO)
  if let Some(root) = env::args().nth(1) {
    if env::set_current_dir(&root).is_err() { process::exit(20) }
    if unix::chroot(root.as_bytes()).is_err() { process::exit(30) }
  }

  with_env_var("GID", set_all_groups);
  with_env_var("UID", unix::setuid);

  let _ = httpd::serve();
}

/// Paranoically extended version of setgid which also nukes the supplemental
/// groups.  This is "unsafe" and "extern" because I can't figure out how to
/// make env_to_libc, below, generic over safety and calling convention.
fn set_all_groups(gid: libc::gid_t) -> io::Result<()> {
  unix::setgroups(&[gid]).and_then(|_| unix::setgid(gid))
}

fn with_env_var<V: FromStr>(var: &str, f: fn(V) -> io::Result<()>) {
  if let Ok(val_str) = env::var(var) {
    println!("{} = {}", var, val_str);
    if let Ok(val) = FromStr::from_str(&val_str) {
      if f(val).is_err() { process::exit(30) }
    } else {
      process::exit(30)
    }
  }
}
