/// Sanitizes a path received from a client: replaces NULs, collapses duplicate
/// slashes, and replaces initial dots.  This is partially paranoia and
/// partially about log tidiness.
///
/// The transformation happens in-place.  The vec is truncated but not shrunken.
pub fn sanitize(path: &mut Vec<u8>) {
  let mut j = 0;
  for i in 0..path.len() {
    match path[i] {
      0 => {
        path[j] = b'_';
        j += 1;
      },
      b'/' => {
        if i == 0 || path[i - 1] != b'/' {
          path[j] = b'/';
          j += 1;
        }
      },
      b'.' => {
        path[j] = if i == 0 || path[i - 1] != b'/' { b'.' }
                  else { b':' };
        j += 1;
      },
      c => {
        path[j] = c;
        j += 1;
      },
    }
  }
  path.truncate(j);
}


