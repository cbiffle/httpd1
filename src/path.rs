use std::iter::IntoIterator;

/// Sanitizes a path received from a client: replaces NULs, collapses duplicate
/// slashes, and replaces initial dots.  This is partially paranoia and
/// partially about log tidiness.
pub fn sanitize<I>(path: I) -> Vec<u8>
    where I: IntoIterator<Item=u8> {
  let iter = path.into_iter();
  let mut result = Vec::with_capacity(iter.size_hint().0);

  for c in iter {
    match c {
      0 => {
        result.push(b'_')
      },
      b'/' => {
        if result.last().cloned() != Some(b'/') {
          result.push(c)
        }
      },
      b'.' => {
        let c = if result.last().cloned() == Some(b'/') { b':' }
                else { c };
        result.push(c)
      },
      c => result.push(c),
    }
  }
  result
}

#[cfg(test)]
mod tests {
  use super::*;

  macro_rules! sanitize_case {
    ($input: expr, $output: expr) => {
      assert_eq!($output, &sanitize($input.iter().cloned())[..])
    };
  }

  #[test]
  fn test_sanitize_identity() {
    sanitize_case!(b"", b"");
    sanitize_case!(b"abcd", b"abcd");
    sanitize_case!(b"/foo/bar/baz", b"/foo/bar/baz");
    sanitize_case!(b"/foo.bar/baz", b"/foo.bar/baz");
  }

  #[test]
  fn test_sanitize_dotfile_rewrite() {
    sanitize_case!(b"/.foo.bar/baz", b"/:foo.bar/baz");
  }

  #[test]
  fn test_sanitize_initial_dot_preserved() {
    // This is odd but correct in our case: the server always generates
    // explicitly relative paths, so an initial dot is expected, and the
    // server will ensure that the *next* byte is a slash.
    sanitize_case!(b"./foo", b"./foo");
  }

  #[test]
  fn test_sanitize_multiple_slash_rewrite() {
    sanitize_case!(b"/foo//bar/baz", b"/foo/bar/baz");
    sanitize_case!(b"//foo//bar/baz", b"/foo/bar/baz");
  }

  #[test]
  fn test_sanitize_nul() {
    sanitize_case!(b"abc\x00d", b"abc_d");
  }
}
