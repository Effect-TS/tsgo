//! Go's `path` package (slash-separated paths): `Clean`, `Join`, `Base` and `Ext`. On Unix, Go's `path/filepath`
//! functions of the same names behave identically (the separator is `/`).

/// Go's `path.Clean`: the shortest lexically equivalent path.
///
/// Multiple slashes become one, `.` elements are removed, `..` elements remove the preceding non-`..` element, a
/// rooted path drops leading `..`s; `"."` for an empty result.
pub fn clean(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let p = path.as_bytes();
    let rooted = p[0] == b'/';
    let n = p.len();

    // Invariants (as in Go): reading from p at r, writing to out; dotdot is the index in out where `..` must stop
    // (the root, or after a leading run of `..`s).
    let mut out: Vec<u8> = Vec::with_capacity(n);
    let mut r = 0;
    let mut dotdot = 0;
    if rooted {
        out.push(b'/');
        r = 1;
        dotdot = 1;
    }

    while r < n {
        if p[r] == b'/' {
            // empty path element
            r += 1;
        } else if p[r] == b'.' && (r + 1 == n || p[r + 1] == b'/') {
            // . element
            r += 1;
        } else if p[r] == b'.' && p[r + 1] == b'.' && (r + 2 == n || p[r + 2] == b'/') {
            // .. element: remove to last /
            r += 2;
            if out.len() > dotdot {
                // can backtrack
                let mut w = out.len() - 1;
                while w > dotdot && out[w] != b'/' {
                    w -= 1;
                }
                out.truncate(w);
            } else if !rooted {
                // cannot backtrack, but not rooted, so append .. element.
                if !out.is_empty() {
                    out.push(b'/');
                }
                out.extend_from_slice(b"..");
                dotdot = out.len();
            }
        } else {
            // real path element.
            // add slash if needed
            if (rooted && out.len() != 1) || (!rooted && !out.is_empty()) {
                out.push(b'/');
            }
            // copy element
            while r < n && p[r] != b'/' {
                out.push(p[r]);
                r += 1;
            }
        }
    }

    // Turn empty string into "."
    if out.is_empty() {
        return ".".to_string();
    }
    // Only ASCII bytes were removed or inserted between whole elements, so the result is valid UTF-8.
    String::from_utf8(out).expect("clean keeps whole path elements")
}

/// Go's `path.Join`: the non-empty elements joined with `/`, then cleaned; `""` when every element is empty.
pub fn join(elem: &[&str]) -> String {
    let non_empty: Vec<&str> = elem.iter().copied().filter(|e| !e.is_empty()).collect();
    if non_empty.is_empty() {
        return String::new();
    }
    clean(&non_empty.join("/"))
}

/// Go's `path.Base` (and `path/filepath.Base` on Unix): the last element of `path`, after trailing slashes are
/// removed; `"."` for an empty path and `"/"` for a path of only slashes.
pub fn base(path: &str) -> &str {
    if path.is_empty() {
        return ".";
    }
    // Strip trailing slashes.
    let path = path.trim_end_matches('/');
    // Find the last element
    let path = match path.rfind('/') {
        Some(i) => &path[i + 1..],
        None => path,
    };
    // If empty now, it had only slashes.
    if path.is_empty() {
        return "/";
    }
    path
}

/// Go's `path.Ext` (and `path/filepath.Ext` on Unix): the suffix beginning at the final dot in the final
/// slash-separated element of `path`; empty if there is no dot.
pub fn ext(path: &str) -> &str {
    for (i, b) in path.bytes().enumerate().rev() {
        if b == b'/' {
            break;
        }
        if b == b'.' {
            return &path[i..];
        }
    }
    ""
}

#[cfg(test)]
mod tests {
    use super::{base, clean, ext, join};

    // Go's path_test.go cleantests.
    #[test]
    fn test_clean() {
        let cases = [
            ("", "."),
            ("abc", "abc"),
            ("abc/def", "abc/def"),
            ("a/b/c", "a/b/c"),
            (".", "."),
            ("..", ".."),
            ("../..", "../.."),
            ("../../abc", "../../abc"),
            ("/abc", "/abc"),
            ("/", "/"),
            ("abc/", "abc"),
            ("abc/def/", "abc/def"),
            ("/abc/", "/abc"),
            ("//abc", "/abc"),
            ("///abc", "/abc"),
            ("//abc//", "/abc"),
            ("abc//", "abc"),
            ("abc//def//ghi", "abc/def/ghi"),
            ("//abc//def//ghi//", "/abc/def/ghi"),
            ("abc/./def", "abc/def"),
            ("/./abc/def", "/abc/def"),
            ("abc/.", "abc"),
            ("abc/def/ghi/../jkl", "abc/def/jkl"),
            ("abc/def/../ghi/../jkl", "abc/jkl"),
            ("abc/def/..", "abc"),
            ("abc/def/../..", "."),
            ("/abc/def/../..", "/"),
            ("abc/def/../../..", ".."),
            ("/abc/def/../../..", "/"),
            ("abc/def/../../../ghi/jkl/../../../mno", "../../mno"),
            ("abc/./../def", "def"),
            ("abc//./../def", "def"),
            ("abc/../../././../def", "../../def"),
        ];
        for (path, want) in cases {
            assert_eq!(clean(path), want, "Clean({path:?})");
        }
    }

    // Go's path_test.go jointests.
    #[test]
    fn test_join() {
        let cases: [(&[&str], &str); 11] = [
            (&[], ""),
            (&[""], ""),
            (&["a"], "a"),
            (&["a", "b"], "a/b"),
            (&["a", ""], "a"),
            (&["", "b"], "b"),
            (&["/", "a"], "/a"),
            (&["/", ""], "/"),
            (&["a/", "b"], "a/b"),
            (&["a/", ""], "a"),
            (&["", ""], ""),
        ];
        for (elem, want) in cases {
            assert_eq!(join(elem), want, "Join({elem:?})");
        }
    }

    // Go's path_test.go basetests.
    #[test]
    fn test_base() {
        let cases = [
            ("", "."),
            (".", "."),
            ("/.", "."),
            ("/", "/"),
            ("////", "/"),
            ("x/", "x"),
            ("abc", "abc"),
            ("abc/def", "def"),
            ("a/b/.x", ".x"),
            ("a/b/c.", "c."),
            ("a/b/c.x", "c.x"),
        ];
        for (path, want) in cases {
            assert_eq!(base(path), want, "Base({path:?})");
        }
    }

    // Go's path_test.go exttests.
    #[test]
    fn test_ext() {
        let cases = [
            ("path.go", ".go"),
            ("path.pb.go", ".go"),
            ("a.dir/b", ""),
            ("a.dir/b.go", ".go"),
            ("a.dir/", ""),
        ];
        for (path, want) in cases {
            assert_eq!(ext(path), want, "Ext({path:?})");
        }
    }
}
