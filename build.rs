fn main() {
    gcc::Config::new()
        .file("c-src/timeout.c")
        .compile("libtimeout.a")
}
