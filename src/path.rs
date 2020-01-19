//! Operations on paths.

/// Sanitizes a path received from a client: replaces NULs, collapses duplicate
/// slashes, and replaces initial dots.  This is partially paranoia and
/// partially about log tidiness.
pub fn sanitize(path: &mut Vec<u8>) {
    let mut last = None;
    filter_map_in_place(path, |&c| {
        let r = match c {
            0 => Some(b'_'),
            b'/' if last == Some(b'/') => None,
            b'.' if last == Some(b'/') => Some(b':'),
            _ => Some(c),
        };
        last = Some(c);
        r
    });
}

fn filter_map_in_place<T>(
    vec: &mut Vec<T>,
    mut f: impl FnMut(&T) -> Option<T>,
) {
    let mut used = 0;
    for i in 0..vec.len() {
        if let Some(repl) = f(&vec[i]) {
            vec[used] = repl;
            used += 1;
        }
    }
    vec.truncate(used);
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! sanitize_case {
        ($input: expr, $output: expr) => {
            {
                let mut fixture = $input.to_vec();
                sanitize(&mut fixture);
                assert_eq!(&fixture[..], $output);
            }
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
