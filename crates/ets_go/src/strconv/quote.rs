//! Go's `strconv/quote.go`: `Quote`, `QuoteToASCII`, `QuoteRune`, `CanBackquote`, `Unquote`, `IsPrint`.

use super::isprint::{IS_GRAPHIC, IS_NOT_PRINT_16, IS_NOT_PRINT_32, IS_PRINT_16, IS_PRINT_32};
use super::{ErrSyntax, NumErr};
use crate::Error;
use crate::utf8::{RUNE_ERROR, RUNE_SELF, append_rune, decode_rune, valid_rune};

const LOWERHEX: &[u8; 16] = b"0123456789abcdef";

fn quote_with(s: &[u8], quote: u8, ascii_only: bool, graphic_only: bool) -> String {
    let mut buf = Vec::with_capacity(3 * s.len() / 2);
    append_quoted_with(&mut buf, s, quote, ascii_only, graphic_only);
    String::from_utf8(buf).expect("quoting produces valid UTF-8")
}

fn quote_rune_with(r: u32, quote: u8, ascii_only: bool, graphic_only: bool) -> String {
    let mut buf = Vec::new();
    append_quoted_rune_with(&mut buf, r, quote, ascii_only, graphic_only);
    String::from_utf8(buf).expect("quoting produces valid UTF-8")
}

fn append_quoted_with(
    buf: &mut Vec<u8>,
    mut s: &[u8],
    quote: u8,
    ascii_only: bool,
    graphic_only: bool,
) {
    buf.push(quote);
    while !s.is_empty() {
        let (r, width) = decode_rune(s);
        if width == 1 && r == RUNE_ERROR {
            buf.extend_from_slice(b"\\x");
            buf.push(LOWERHEX[(s[0] >> 4) as usize]);
            buf.push(LOWERHEX[(s[0] & 0xF) as usize]);
            s = &s[width..];
            continue;
        }
        append_escaped_rune(buf, r, quote, ascii_only, graphic_only);
        s = &s[width..];
    }
    buf.push(quote);
}

fn append_quoted_rune_with(
    buf: &mut Vec<u8>,
    r: u32,
    quote: u8,
    ascii_only: bool,
    graphic_only: bool,
) {
    buf.push(quote);
    let r = if valid_rune(r) { r } else { RUNE_ERROR };
    append_escaped_rune(buf, r, quote, ascii_only, graphic_only);
    buf.push(quote);
}

fn append_escaped_rune(buf: &mut Vec<u8>, r: u32, quote: u8, ascii_only: bool, graphic_only: bool) {
    if r == u32::from(quote) || r == u32::from(b'\\') {
        // always backslashed
        buf.push(b'\\');
        buf.push(r as u8);
        return;
    }
    if ascii_only {
        if r < RUNE_SELF && is_print_rune(r) {
            buf.push(r as u8);
            return;
        }
    } else if is_print_rune(r) || graphic_only && is_in_graphic_list(r) {
        append_rune(buf, r);
        return;
    }
    match r {
        0x07 => buf.extend_from_slice(b"\\a"),
        0x08 => buf.extend_from_slice(b"\\b"),
        0x0C => buf.extend_from_slice(b"\\f"),
        0x0A => buf.extend_from_slice(b"\\n"),
        0x0D => buf.extend_from_slice(b"\\r"),
        0x09 => buf.extend_from_slice(b"\\t"),
        0x0B => buf.extend_from_slice(b"\\v"),
        _ => {
            if r < u32::from(b' ') || r == 0x7f {
                buf.extend_from_slice(b"\\x");
                buf.push(LOWERHEX[((r as u8) >> 4) as usize]);
                buf.push(LOWERHEX[((r as u8) & 0xF) as usize]);
            } else {
                let r = if valid_rune(r) { r } else { 0xFFFD };
                if r < 0x10000 {
                    buf.extend_from_slice(b"\\u");
                    for s in (0..=12).rev().step_by(4) {
                        buf.push(LOWERHEX[((r >> s) & 0xF) as usize]);
                    }
                } else {
                    buf.extend_from_slice(b"\\U");
                    for s in (0..=28).rev().step_by(4) {
                        buf.push(LOWERHEX[((r >> s) & 0xF) as usize]);
                    }
                }
            }
        }
    }
}

/// Go's `strconv.Quote`.
pub fn quote(s: &str) -> String {
    quote_with(s.as_bytes(), b'"', false, false)
}

/// Go's `strconv.Quote(string(b))` for bytes that may not be valid UTF-8.
pub fn quote_bytes(s: &[u8]) -> String {
    quote_with(s, b'"', false, false)
}

/// Go's `strconv.AppendQuote`.
pub fn append_quote(dst: &mut Vec<u8>, s: &[u8]) {
    append_quoted_with(dst, s, b'"', false, false);
}

/// Go's `strconv.QuoteToASCII`.
pub fn quote_to_ascii(s: &str) -> String {
    quote_with(s.as_bytes(), b'"', true, false)
}

/// Go's `strconv.AppendQuoteToASCII`.
pub fn append_quote_to_ascii(dst: &mut Vec<u8>, s: &[u8]) {
    append_quoted_with(dst, s, b'"', true, false);
}

/// Go's `strconv.QuoteToGraphic`.
pub fn quote_to_graphic(s: &str) -> String {
    quote_with(s.as_bytes(), b'"', false, true)
}

/// Go's `strconv.QuoteRune`.
pub fn quote_rune(r: u32) -> String {
    quote_rune_with(r, b'\'', false, false)
}

/// Go's `strconv.AppendQuoteRune`.
pub fn append_quote_rune(dst: &mut Vec<u8>, r: u32) {
    append_quoted_rune_with(dst, r, b'\'', false, false);
}

/// Go's `strconv.QuoteRuneToASCII`.
pub fn quote_rune_to_ascii(r: u32) -> String {
    quote_rune_with(r, b'\'', true, false)
}

/// Go's `strconv.AppendQuoteRuneToASCII`.
pub fn append_quote_rune_to_ascii(dst: &mut Vec<u8>, r: u32) {
    append_quoted_rune_with(dst, r, b'\'', true, false);
}

/// Go's `strconv.CanBackquote`.
pub fn can_backquote(s: &[u8]) -> bool {
    let mut s = s;
    while !s.is_empty() {
        let (r, wid) = decode_rune(s);
        s = &s[wid..];
        if wid > 1 {
            if r == 0xFEFF {
                return false; // BOMs are invisible and should not be quoted.
            }
            continue; // All other multibyte runes are correctly encoded and assumed printable.
        }
        if r == RUNE_ERROR {
            return false;
        }
        if (r < u32::from(b' ') && r != u32::from(b'\t')) || r == u32::from(b'`') || r == 0x7F {
            return false;
        }
    }
    true
}

fn unhex(b: u8) -> Option<u32> {
    let c = u32::from(b);
    match b {
        b'0'..=b'9' => Some(c - u32::from(b'0')),
        b'a'..=b'f' => Some(c - u32::from(b'a') + 10),
        b'A'..=b'F' => Some(c - u32::from(b'A') + 10),
        _ => None,
    }
}

/// Go's `strconv.UnquoteChar(s, quote)`: `(value, multibyte, tail)`, or `ErrSyntax`.
pub fn unquote_char(s: &[u8], quote: u8) -> Result<(u32, bool, &[u8]), Error> {
    unquote_char_internal(s, quote).map_err(|_| ErrSyntax.clone())
}

fn unquote_char_internal(s: &[u8], quote: u8) -> Result<(u32, bool, &[u8]), NumErr> {
    // easy cases
    let Some(&c) = s.first() else {
        return Err(NumErr::Syntax);
    };
    if c == quote && (quote == b'\'' || quote == b'"') {
        return Err(NumErr::Syntax);
    } else if u32::from(c) >= RUNE_SELF {
        let (r, size) = decode_rune(s);
        return Ok((r, true, &s[size..]));
    } else if c != b'\\' {
        return Ok((u32::from(s[0]), false, &s[1..]));
    }
    // hard case: c is backslash
    if s.len() <= 1 {
        return Err(NumErr::Syntax);
    }
    let c = s[1];
    let mut s = &s[2..];
    let mut multibyte = false;
    let value = match c {
        b'a' => 0x07,
        b'b' => 0x08,
        b'f' => 0x0C,
        b'n' => 0x0A,
        b'r' => 0x0D,
        b't' => 0x09,
        b'v' => 0x0B,
        b'x' | b'u' | b'U' => {
            let n = match c {
                b'x' => 2,
                b'u' => 4,
                _ => 8,
            };
            if s.len() < n {
                return Err(NumErr::Syntax);
            }
            let mut v: u32 = 0;
            for &b in &s[..n] {
                let x = unhex(b).ok_or(NumErr::Syntax)?;
                v = (v << 4) | x;
            }
            s = &s[n..];
            if c == b'x' {
                // single-byte string, possibly not UTF-8
                v
            } else {
                if !valid_rune(v) {
                    return Err(NumErr::Syntax);
                }
                multibyte = true;
                v
            }
        }
        b'0'..=b'7' => {
            let mut v = u32::from(c - b'0');
            if s.len() < 2 {
                return Err(NumErr::Syntax);
            }
            for &b in &s[..2] {
                // one digit already; two more
                let x = u32::from(b).wrapping_sub(u32::from(b'0'));
                if x > 7 {
                    return Err(NumErr::Syntax);
                }
                v = (v << 3) | x;
            }
            s = &s[2..];
            if v > 255 {
                return Err(NumErr::Syntax);
            }
            v
        }
        b'\\' => u32::from(b'\\'),
        b'\'' | b'"' => {
            if c != quote {
                return Err(NumErr::Syntax);
            }
            u32::from(c)
        }
        _ => return Err(NumErr::Syntax),
    };
    Ok((value, multibyte, s))
}

/// Go's `strconv.QuotedPrefix`: the quoted string at the start of `s`.
pub fn quoted_prefix(s: &[u8]) -> Result<&[u8], Error> {
    match unquote_internal(s, false) {
        Ok((Unquoted::Borrowed(out), _)) => Ok(out),
        Ok((Unquoted::Owned(_), _)) => unreachable!("unquote without unescape borrows"),
        Err(_) => Err(ErrSyntax.clone()),
    }
}

/// Go's `strconv.Unquote(s)`, for results that are valid UTF-8 (a `\x` escape can produce bytes that are not;
/// use [`unquote_bytes`] where that can happen). Invalid UTF-8 in the result is an `ErrSyntax` here.
pub fn unquote(s: &str) -> Result<String, Error> {
    let bytes = unquote_bytes(s.as_bytes())?;
    String::from_utf8(bytes).map_err(|_| ErrSyntax.clone())
}

/// Go's `strconv.Unquote(s)` on bytes, returning bytes (Go strings may hold any bytes).
pub fn unquote_bytes(s: &[u8]) -> Result<Vec<u8>, Error> {
    match unquote_internal(s, true) {
        Ok((out, [])) => Ok(match out {
            Unquoted::Borrowed(b) => b.to_vec(),
            Unquoted::Owned(b) => b,
        }),
        _ => Err(ErrSyntax.clone()),
    }
}

enum Unquoted<'a> {
    Borrowed(&'a [u8]),
    Owned(Vec<u8>),
}

fn unquote_internal(input: &[u8], unescape: bool) -> Result<(Unquoted<'_>, &[u8]), NumErr> {
    // Determine the quote form and optimistically find the terminating quote.
    if input.len() < 2 {
        return Err(NumErr::Syntax);
    }
    let quote = input[0];
    let Some(end) = input[1..].iter().position(|&c| c == quote) else {
        return Err(NumErr::Syntax);
    };
    let end = end + 2; // position after terminating quote; may be wrong if escape sequences are present

    match quote {
        b'`' => {
            // `` `string` `` cannot contain backquotes.
            let out = if !unescape {
                Unquoted::Borrowed(&input[..end]) // include quotes
            } else if !input[..end].contains(&b'\r') {
                Unquoted::Borrowed(&input[1..end - 1]) // exclude quotes
            } else {
                // Carriage return characters ('\r') inside raw string literals are discarded from the raw
                // string value.
                Unquoted::Owned(input[1..end - 1].iter().copied().filter(|&c| c != b'\r').collect())
            };
            // NOTE: Prior implementations did not verify that raw strings consist of valid UTF-8
            // characters and we continue to not verify it as such. The Go specification does not
            // explicitly require valid UTF-8, but only mention that it is implicitly valid for Go source
            // code (which must be valid UTF-8).
            Ok((out, &input[end..]))
        }
        b'"' | b'\'' => {
            // Handle quoted strings without any escape sequences.
            if !input[..end].contains(&b'\\') && !input[..end].contains(&b'\n') {
                let valid = if quote == b'"' {
                    std::str::from_utf8(&input[1..end - 1]).is_ok()
                } else {
                    let (r, n) = decode_rune(&input[1..end - 1]);
                    1 + n + 1 == end && (r != RUNE_ERROR || n != 1)
                };
                if valid {
                    let out = if unescape { &input[1..end - 1] } else { &input[..end] };
                    return Ok((Unquoted::Borrowed(out), &input[end..]));
                }
            }
            // Handle quoted strings with escape sequences.
            let in0 = input;
            let mut rest = &input[1..]; // skip starting quote
            let mut buf = Vec::new();
            if unescape {
                buf.reserve(3 * end / 2); // try to avoid more allocations
            }
            while !rest.is_empty() && rest[0] != quote {
                // Process the next character, rejecting any unescaped newline characters which are
                // invalid.
                let first = rest[0];
                let result = unquote_char_internal(rest, quote);
                let Ok((r, multibyte, rem)) = result else {
                    return Err(NumErr::Syntax);
                };
                if first == b'\n' {
                    return Err(NumErr::Syntax);
                }
                rest = rem;
                // Append the character if unescaping the input.
                if unescape {
                    if r < RUNE_SELF || !multibyte {
                        buf.push(r as u8);
                    } else {
                        append_rune(&mut buf, r);
                    }
                }
                // Single quoted strings must be a single character.
                if quote == b'\'' {
                    break;
                }
            }
            // Verify that the string ends with a terminating quote.
            if rest.is_empty() || rest[0] != quote {
                return Err(NumErr::Syntax);
            }
            rest = &rest[1..]; // skip terminating quote
            if unescape {
                return Ok((Unquoted::Owned(buf), rest));
            }
            Ok((Unquoted::Borrowed(&in0[..in0.len() - rest.len()]), rest))
        }
        _ => Err(NumErr::Syntax),
    }
}

fn bsearch<T: PartialOrd + Copy>(s: &[T], v: T) -> (usize, bool) {
    let n = s.len();
    let (mut i, mut j) = (0, n);
    while i < j {
        let h = i + ((j - i) >> 1);
        if s[h] < v {
            i = h + 1;
        } else {
            j = h;
        }
    }
    (i, i < n && s[i] == v)
}

/// Go's `strconv.IsPrint`: letters, marks, numbers, punctuation, symbols and the ASCII space, per Go's
/// Unicode tables.
pub fn is_print(r: char) -> bool {
    is_print_rune(r as u32)
}

pub(crate) fn is_print_rune(r: u32) -> bool {
    // Fast check for Latin-1
    if r <= 0xFF {
        if (0x20..=0x7E).contains(&r) {
            // All the ASCII is printable from space through DEL-1.
            return true;
        }
        if (0xA1..=0xFF).contains(&r) {
            // Similarly for ¡ through ÿ...
            return r != 0xAD; // ...except for the bizarre soft hyphen.
        }
        return false;
    }
    // Same algorithm, either on uint16 or uint32 value.
    // First, find first i such that isPrint[i] >= x.
    // This is the index of either the start or end of a pair that might span x.
    // The start is even (isPrint[i&^1]) and the end is odd (isPrint[i|1]).
    // If we find x in a range, make sure x is not in isNotPrint list.
    if r < 1 << 16 {
        let rr = r as u16;
        let (i, _) = bsearch(&IS_PRINT_16, rr);
        if i >= IS_PRINT_16.len() || rr < IS_PRINT_16[i & !1] || IS_PRINT_16[i | 1] < rr {
            return false;
        }
        let (_, found) = bsearch(&IS_NOT_PRINT_16, rr);
        return !found;
    }
    let (i, _) = bsearch(&IS_PRINT_32, r);
    if i >= IS_PRINT_32.len() || r < IS_PRINT_32[i & !1] || IS_PRINT_32[i | 1] < r {
        return false;
    }
    if r >= 0x20000 {
        return true;
    }
    let (_, found) = bsearch(&IS_NOT_PRINT_32, (r - 0x10000) as u16);
    !found
}

/// Go's `strconv.IsGraphic`: [`is_print`] plus the Unicode space separators.
pub fn is_graphic(r: char) -> bool {
    is_print(r) || is_in_graphic_list(r as u32)
}

fn is_in_graphic_list(r: u32) -> bool {
    // We know that the range is 16 bits.
    if r > 0xFFFF {
        return false;
    }
    bsearch(&IS_GRAPHIC, r as u16).1
}
