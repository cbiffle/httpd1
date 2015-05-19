pub fn filetype(file_path: &[u8]) -> &'static [u8] {
  match file_path.rsplitn(2, |b| *b == b'.').next() {
    Some(ext) => canned_mapping(ext),

    // TODO: a path without an extension should perhaps not be served as
    // text/plain?
    _ => b"text/plain",
  }
}

fn canned_mapping(ext: &[u8]) -> &'static [u8] {
  match ext {
    b"html"           => b"text/html",
    b"gif"            => b"image/gif",
    b"jpeg" | b"jpg"  => b"image/jpeg",
    b"png"            => b"image/png",
    b"pdf"            => b"application/pdf",
    b"css"            => b"text/css",
    _ => b"text/plain",
  }
}
