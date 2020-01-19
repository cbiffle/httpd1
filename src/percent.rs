//! URL percent-encoding.

use crate::error::{HttpError, Result};

/// Decodes URL percent-escaping, in-place.  Fails if the encoding is bad.
pub fn unescape(path: &[u8], out: &mut Vec<u8>) -> Result<()> {
    fn fromhex(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'A'..=b'F' => Some(b - b'A' + 10),
            b'a'..=b'f' => Some(b - b'a' + 10),
            _ => None,
        }
    }

    // TODO this is such a C approach; mutable index variables are hard to
    // reason about.
    let mut i = 0;
    while i < path.len() {
        let c = path[i];
        i += 1;

        if c == b'%' {
            // Possible valid escape.
            if (path.len() - i) < 2 {
                return Err(HttpError::BadRequest);
            }

            if let (Some(a), Some(b)) = (fromhex(path[i]), fromhex(path[i + 1]))
            {
                out.push(a * 16 + b);
                i += 2; // skip consumed hex characters.
            } else {
                return Err(HttpError::BadRequest);
            }
        } else {
            out.push(c);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! unescape_case {
        ($input: expr, PASS, $output: expr) => {{
            let path = $input;
            let mut v = Vec::new();
            assert!(unescape(path, &mut v).is_ok());
            assert_eq!($output, &v[..])
        }};
        ($input: expr, FAIL) => {{
            let path = $input;
            let mut v = Vec::new();
            assert!(unescape(path, &mut v).is_err());
        }};
    }

    #[test]
    fn test() {
        unescape_case!(b"", PASS, b"");
        unescape_case!(b"%00%01ab%63%64", PASS, b"\x00\x01abcd");
        unescape_case!(b"foo%XY", FAIL);
        unescape_case!(b"foo%X", FAIL);
        unescape_case!(b"foo%", FAIL);
    }
}
