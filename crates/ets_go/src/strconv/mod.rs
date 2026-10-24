//! Go's `strconv`: integer and float parsing and formatting with Go's syntax, results and `*NumError`
//! texts, and Go's quoting (`Quote`, `Unquote`, `IsPrint` with Go's Unicode tables).
//!
//! Ported from Go's `strconv` and `internal/strconv` (the Go toolchain this repository builds with). The
//! shortest float digits come from Rust's formatter (the shortest round-tripping digits closest to the value
//! are unique, which is what Go's algorithm computes too); everything around them is Go's code.

mod atof;
mod decimal;
mod ftoa;
mod isprint;
mod quote;

use std::borrow::Cow;

pub use atof::{parse_float, parse_float_value};
pub use ftoa::{append_float, format_float};
pub use quote::{
    append_quote, append_quote_rune, append_quote_rune_to_ascii, append_quote_to_ascii,
    can_backquote, is_graphic, is_print, quote, quote_bytes, quote_rune, quote_rune_to_ascii,
    quote_to_ascii, quote_to_graphic, quoted_prefix, unquote, unquote_bytes, unquote_char,
};

use crate::Error;

crate::sentinel! {
    /// Go's `strconv.ErrRange`.
    pub static ErrRange = "value out of range";
    /// Go's `strconv.ErrSyntax`.
    pub static ErrSyntax = "invalid syntax";
}

/// Go's `strconv.IntSize`.
pub const INT_SIZE: u32 = 64;

/// Go's `*strconv.NumError`.
pub struct NumError {
    /// The failing function (`ParseBool`, `ParseInt`, `ParseUint`, `ParseFloat`, `Atoi`).
    pub func: &'static str,
    /// The input.
    pub num: String,
    /// The reason: [`ErrRange`], [`ErrSyntax`], or an "invalid base" / "invalid bit size" error.
    pub err: Error,
}

impl crate::errors::GoError for NumError {
    fn error(&self) -> String {
        format!("strconv.{}: parsing {}: {}", self.func, quote(&self.num), self.err.error())
    }

    fn type_name(&self) -> Cow<'static, str> {
        Cow::Borrowed("*strconv.NumError")
    }

    fn unwrap(&self) -> crate::errors::Unwrapped<'_> {
        crate::errors::Unwrapped::One(&self.err)
    }
}

/// The reasons `internal/strconv` reports, before `toError` turns them into a `*NumError`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum NumErr {
    Range,
    Syntax,
    Base,
    BitSize,
}

fn to_error(func: &'static str, s: &str, base: u32, bit_size: u32, err: NumErr) -> Error {
    let err = match err {
        NumErr::Syntax => ErrSyntax.clone(),
        NumErr::Range => ErrRange.clone(),
        NumErr::Base => crate::errors::new(format!("invalid base {base}")),
        NumErr::BitSize => crate::errors::new(format!("invalid bit size {bit_size}")),
    };
    Error::new(NumError { func, num: s.to_string(), err })
}

fn lower(c: u8) -> u8 {
    c | (b'x' - b'X')
}

/// Go's `strconv.ParseBool`.
pub fn parse_bool(str: &str) -> Result<bool, Error> {
    match str {
        "1" | "t" | "T" | "true" | "TRUE" | "True" => Ok(true),
        "0" | "f" | "F" | "false" | "FALSE" | "False" => Ok(false),
        _ => Err(to_error("ParseBool", str, 0, 0, NumErr::Syntax)),
    }
}

/// Go's `strconv.FormatBool`.
pub fn format_bool(b: bool) -> &'static str {
    if b { "true" } else { "false" }
}

/// Go's `strconv.ParseUint(s, base, bitSize)`. On a range error Go also returns the maximum value;
/// [`parse_uint_value`] returns it with the error.
pub fn parse_uint(s: &str, base: u32, bit_size: u32) -> Result<u64, Error> {
    parse_uint_internal(s, base, bit_size)
        .map_err(|(_, err)| to_error("ParseUint", s, base, bit_size, err))
}

/// Go's `strconv.ParseUint` returning the value Go returns even on error (the clamped maximum on a range
/// error, 0 otherwise).
pub fn parse_uint_value(s: &str, base: u32, bit_size: u32) -> (u64, Option<Error>) {
    match parse_uint_internal(s, base, bit_size) {
        Ok(v) => (v, None),
        Err((v, err)) => (v, Some(to_error("ParseUint", s, base, bit_size, err))),
    }
}

pub(crate) fn parse_uint_internal(s: &str, base: u32, bit_size: u32) -> Result<u64, (u64, NumErr)> {
    if s.is_empty() {
        return Err((0, NumErr::Syntax));
    }
    let base0 = base == 0;
    let s0 = s;
    let mut s = s.as_bytes();
    let mut base = base;
    match base {
        2..=36 => {}
        0 => {
            // Look for octal, hex prefix.
            base = 10;
            if s[0] == b'0' {
                if s.len() >= 3 && lower(s[1]) == b'b' {
                    base = 2;
                    s = &s[2..];
                } else if s.len() >= 3 && lower(s[1]) == b'o' {
                    base = 8;
                    s = &s[2..];
                } else if s.len() >= 3 && lower(s[1]) == b'x' {
                    base = 16;
                    s = &s[2..];
                } else {
                    base = 8;
                    s = &s[1..];
                }
            }
        }
        _ => return Err((0, NumErr::Base)),
    }
    let bit_size = if bit_size == 0 {
        INT_SIZE
    } else if bit_size > 64 {
        return Err((0, NumErr::BitSize));
    } else {
        bit_size
    };
    // Cutoff is the smallest number such that cutoff*base > maxUint64.
    let cutoff = u64::MAX / u64::from(base) + 1;
    let max_val = if bit_size == 64 { u64::MAX } else { (1u64 << bit_size) - 1 };
    let mut underscores = false;
    let mut n: u64 = 0;
    for &c in s {
        let d = if c == b'_' && base0 {
            underscores = true;
            continue;
        } else if c.is_ascii_digit() {
            c - b'0'
        } else if lower(c).is_ascii_lowercase() {
            lower(c) - b'a' + 10
        } else {
            return Err((0, NumErr::Syntax));
        };
        if u32::from(d) >= base {
            return Err((0, NumErr::Syntax));
        }
        if n >= cutoff {
            // n*base overflows
            return Err((max_val, NumErr::Range));
        }
        n *= u64::from(base);
        let n1 = n.wrapping_add(u64::from(d));
        if n1 < n || n1 > max_val {
            // n+d overflows
            return Err((max_val, NumErr::Range));
        }
        n = n1;
    }
    if underscores && !underscore_ok(s0.as_bytes()) {
        return Err((0, NumErr::Syntax));
    }
    Ok(n)
}

/// Go's `strconv.ParseInt(s, base, bitSize)`.
pub fn parse_int(s: &str, base: u32, bit_size: u32) -> Result<i64, Error> {
    parse_int_internal(s, base, bit_size)
        .map_err(|(_, err)| to_error("ParseInt", s, base, bit_size, err))
}

/// Go's `strconv.ParseInt` returning the value Go returns even on error (the clamped extreme on a range
/// error, 0 otherwise).
pub fn parse_int_value(s: &str, base: u32, bit_size: u32) -> (i64, Option<Error>) {
    match parse_int_internal(s, base, bit_size) {
        Ok(v) => (v, None),
        Err((v, err)) => (v, Some(to_error("ParseInt", s, base, bit_size, err))),
    }
}

fn parse_int_internal(s: &str, base: u32, bit_size: u32) -> Result<i64, (i64, NumErr)> {
    if s.is_empty() {
        return Err((0, NumErr::Syntax));
    }
    // Pick off leading sign.
    let mut neg = false;
    let mut digits = s;
    if let Some(rest) = s.strip_prefix('+') {
        digits = rest;
    } else if let Some(rest) = s.strip_prefix('-') {
        digits = rest;
        neg = true;
    }
    // Convert unsigned and check range.
    let un = match parse_uint_internal(digits, base, bit_size) {
        Ok(un) => un,
        Err((un, NumErr::Range)) => un,
        Err((_, err)) => return Err((0, err)),
    };
    let bit_size = if bit_size == 0 { INT_SIZE } else { bit_size };
    let cutoff = 1u64 << (bit_size - 1);
    if !neg && un >= cutoff {
        return Err(((cutoff - 1) as i64, NumErr::Range));
    }
    if neg && un > cutoff {
        return Err(((cutoff as i64).wrapping_neg(), NumErr::Range));
    }
    let mut n = un as i64;
    if neg {
        n = n.wrapping_neg();
    }
    // The range error from ParseUint was only about the unsigned bound; the signed checks above decide.
    Ok(n)
}

/// Go's `strconv.Atoi(s)` (`ParseInt(s, 10, 0)`, reported as `Atoi`).
pub fn atoi(s: &str) -> Result<i64, Error> {
    match atoi_value(s) {
        (v, None) => Ok(v),
        (_, Some(err)) => Err(err),
    }
}

/// Go's `strconv.Atoi` returning the value Go returns even on error (the clamped extreme on a range
/// error, 0 otherwise).
pub fn atoi_value(s: &str) -> (i64, Option<Error>) {
    let s_len = s.len();
    if 0 < s_len && s_len < 19 {
        // Fast path for small integers that fit int type.
        let mut b = s.as_bytes();
        if b[0] == b'-' || b[0] == b'+' {
            b = &b[1..];
            if b.is_empty() {
                return (0, Some(to_error("Atoi", s, 0, 0, NumErr::Syntax)));
            }
        }
        let mut n: i64 = 0;
        for &ch in b {
            let ch = ch.wrapping_sub(b'0');
            if ch > 9 {
                return (0, Some(to_error("Atoi", s, 0, 0, NumErr::Syntax)));
            }
            n = n * 10 + i64::from(ch);
        }
        if s.as_bytes()[0] == b'-' {
            n = -n;
        }
        return (n, None);
    }
    // Slow path for invalid, big, or underscored integers.
    match parse_int_internal(s, 10, 0) {
        Ok(v) => (v, None),
        Err((v, err)) => (v, Some(to_error("Atoi", s, 0, 0, err))),
    }
}

/// Go's `underscoreOK`: whether underscores in `s` are used as Go digit separators.
pub(crate) fn underscore_ok(s: &[u8]) -> bool {
    // saw tracks the last character (class) we saw: ^ for beginning of number, 0 for a digit or base
    // prefix, _ for an underscore, ! for none of the above.
    let mut saw = b'^';
    let mut i = 0;
    let mut s = s;
    // Optional sign.
    if !s.is_empty() && (s[0] == b'-' || s[0] == b'+') {
        s = &s[1..];
    }
    // Optional base prefix.
    let mut hex = false;
    if s.len() >= 2
        && s[0] == b'0'
        && (lower(s[1]) == b'b' || lower(s[1]) == b'o' || lower(s[1]) == b'x')
    {
        i = 2;
        saw = b'0'; // base prefix counts as a digit for "underscore as digit separator"
        hex = lower(s[1]) == b'x';
    }
    // Number proper.
    while i < s.len() {
        // Digits are always okay.
        if s[i].is_ascii_digit() || hex && (b'a'..=b'f').contains(&lower(s[i])) {
            saw = b'0';
            i += 1;
            continue;
        }
        // Underscore must follow digit.
        if s[i] == b'_' {
            if saw != b'0' {
                return false;
            }
            saw = b'_';
            i += 1;
            continue;
        }
        // Underscore must also be followed by digit.
        if saw == b'_' {
            return false;
        }
        // Saw non-digit, non-underscore.
        saw = b'!';
        i += 1;
    }
    saw != b'_'
}

/// Go's `strconv.FormatInt(i, base)` (base 2 to 36, lowercase letters).
pub fn format_int(i: i64, base: u32) -> String {
    let mut b = Vec::new();
    append_int(&mut b, i, base);
    String::from_utf8(b).expect("ASCII digits")
}

/// Go's `strconv.FormatUint(i, base)`.
pub fn format_uint(i: u64, base: u32) -> String {
    let mut b = Vec::new();
    append_uint(&mut b, i, base);
    String::from_utf8(b).expect("ASCII digits")
}

/// Go's `strconv.Itoa`.
pub fn itoa(i: i64) -> String {
    i.to_string()
}

/// Go's `strconv.AppendInt`.
pub fn append_int(dst: &mut Vec<u8>, i: i64, base: u32) {
    if i < 0 {
        dst.push(b'-');
    }
    append_uint(dst, i.unsigned_abs(), base);
}

/// Go's `strconv.AppendUint`.
pub fn append_uint(dst: &mut Vec<u8>, mut u: u64, base: u32) {
    assert!((2..=36).contains(&base), "strconv: illegal AppendInt/FormatInt base");
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut buf = [0u8; 64];
    let mut i = buf.len();
    let b = u64::from(base);
    while u >= b {
        i -= 1;
        buf[i] = DIGITS[(u % b) as usize];
        u /= b;
    }
    i -= 1;
    buf[i] = DIGITS[u as usize];
    dst.extend_from_slice(&buf[i..]);
}
