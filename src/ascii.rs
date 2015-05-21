use std::ascii::AsciiExt;

pub trait AsciiPrefix {
  fn starts_with_ignore_ascii_case(&self, prefix: &[u8]) -> bool;
}

impl AsciiPrefix for Vec<u8> {
  fn starts_with_ignore_ascii_case(&self, prefix: &[u8]) -> bool {
    (&self[..]).starts_with_ignore_ascii_case(prefix)
  }
}

impl<'a> AsciiPrefix for &'a [u8] {
  fn starts_with_ignore_ascii_case(&self, prefix: &[u8]) -> bool {
    if self.len() < prefix.len() {
      false
    } else {
      (&self[..prefix.len()]).eq_ignore_ascii_case(prefix)
    }
  }
}

#[cfg(test)]
mod tests {
  use super::AsciiPrefix;

  #[test]
  fn test_starts_with_ignore_ascii_case() {
    assert_eq!(true, b"".as_ref().starts_with_ignore_ascii_case(b""));
    assert_eq!(true, b"foobar".as_ref().starts_with_ignore_ascii_case(b""));
    assert_eq!(true, b"foobar".as_ref().starts_with_ignore_ascii_case(b"foo"));
    assert_eq!(true, b"FOOBAR".as_ref().starts_with_ignore_ascii_case(b"foo"));

    assert_eq!(false, b"foo".as_ref().starts_with_ignore_ascii_case(b"foobar"));
    assert_eq!(false, b"".as_ref().starts_with_ignore_ascii_case(b"foobar"));
  }
}
