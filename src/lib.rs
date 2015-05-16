// Need libc to do unbuffered stdout/stdin :-/
#![feature(libc)]
extern crate libc;

use std::process;

pub mod unix;

pub fn serve() -> ! {
  process::exit(0)
}
