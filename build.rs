fn main() {
    cc::Build::new()
        .file("c-src/timeout.c")
        .compile("libtimeout.a")
}
