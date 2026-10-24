//! Go's `internal/strconv/atof.go`: `ParseFloat`.
//!
//! The syntax (`readFloat`, `special`, underscores, hex mantissas) and the hex rounding are Go's code. A
//! decimal mantissa is converted by Rust's correctly rounded parser from the same digits and exponent,
//! which gives Go's (also correctly rounded) result.

use super::{NumErr, lower, to_error, underscore_ok};
use crate::Error;

fn common_prefix_len_ignore_case(s: &[u8], prefix: &[u8]) -> usize {
    let n = prefix.len().min(s.len());
    for i in 0..n {
        let mut c = s[i];
        if c.is_ascii_uppercase() {
            c += b'a' - b'A';
        }
        if c != prefix[i] {
            return i;
        }
    }
    n
}

/// Go's `special`: a possibly signed "inf" / "infinity" (any case) or "nan".
fn special(s: &[u8]) -> Option<(f64, usize)> {
    if s.is_empty() {
        return None;
    }
    let mut sign = 1.0;
    let mut nsign = 0;
    let mut s = s;
    match s[0] {
        b'+' | b'-' | b'i' | b'I' => {
            if s[0] == b'+' || s[0] == b'-' {
                if s[0] == b'-' {
                    sign = -1.0;
                }
                nsign = 1;
                s = &s[1..];
            }
            let mut n = common_prefix_len_ignore_case(s, b"infinity");
            // Anything longer than "inf" is ok, but if we dont have "infinity", only consume "inf".
            if 3 < n && n < 8 {
                n = 3;
            }
            if n == 3 || n == 8 {
                return Some((sign * f64::INFINITY, nsign + n));
            }
        }
        b'n' | b'N' if common_prefix_len_ignore_case(s, b"nan") == 3 => {
            return Some((f64::from_bits(0x7FF8_0000_0000_0001), 3)); // Go's math.NaN()
        }
        _ => {}
    }
    None
}

struct ReadFloat {
    mantissa: u64,
    exp: i32,
    neg: bool,
    trunc: bool,
    hex: bool,
    i: usize,
    ok: bool,
}

/// Go's `readFloat`: reads a decimal or hexadecimal mantissa and exponent from a float string
/// representation in s; the number may be followed by other characters. `i` is the number of bytes read.
fn read_float(s: &[u8]) -> ReadFloat {
    let mut r =
        ReadFloat { mantissa: 0, exp: 0, neg: false, trunc: false, hex: false, i: 0, ok: false };
    let mut underscores = false;
    let mut i = 0;

    // optional sign
    if i >= s.len() {
        return r;
    }
    if s[i] == b'+' {
        i += 1;
    } else if s[i] == b'-' {
        i += 1;
        r.neg = true;
    }

    // digits
    let mut base: u64 = 10;
    let mut max_mant_digits = 19; // 10^19 fits in uint64
    let mut exp_char = b'e';
    if i + 2 < s.len() && s[i] == b'0' && lower(s[i + 1]) == b'x' {
        base = 16;
        max_mant_digits = 16; // 16^16 fits in uint64
        i += 2;
        exp_char = b'p';
        r.hex = true;
    }
    let mut sawdot = false;
    let mut sawdigits = false;
    let mut nd: i32 = 0;
    let mut nd_mant: i32 = 0;
    let mut dp: i32 = 0;
    while i < s.len() {
        let c = s[i];
        if c == b'_' {
            underscores = true;
            i += 1;
            continue;
        } else if c == b'.' {
            if sawdot {
                break;
            }
            sawdot = true;
            dp = nd;
            i += 1;
            continue;
        } else if c.is_ascii_digit() {
            sawdigits = true;
            if c == b'0' && nd == 0 {
                // ignore leading zeros
                dp -= 1;
                i += 1;
                continue;
            }
            nd += 1;
            if nd_mant < max_mant_digits {
                r.mantissa *= base;
                r.mantissa += u64::from(c - b'0');
                nd_mant += 1;
            } else if c != b'0' {
                r.trunc = true;
            }
            i += 1;
            continue;
        } else if base == 16 && (b'a'..=b'f').contains(&lower(c)) {
            sawdigits = true;
            nd += 1;
            if nd_mant < max_mant_digits {
                r.mantissa *= 16;
                r.mantissa += u64::from(lower(c) - b'a' + 10);
                nd_mant += 1;
            } else {
                r.trunc = true;
            }
            i += 1;
            continue;
        }
        break;
    }
    if !sawdigits {
        r.i = i;
        return r;
    }
    if !sawdot {
        dp = nd;
    }

    if base == 16 {
        dp *= 4;
        nd_mant *= 4;
    }

    // optional exponent moves decimal point. if we read a very large, very long number, just be sure to
    // move the decimal point by a lot (say, 100000). it doesn't matter if it's not the exact number.
    if i < s.len() && lower(s[i]) == exp_char {
        i += 1;
        if i >= s.len() {
            r.i = i;
            return r;
        }
        let mut esign = 1;
        if s[i] == b'+' {
            i += 1;
        } else if s[i] == b'-' {
            i += 1;
            esign = -1;
        }
        if i >= s.len() || !s[i].is_ascii_digit() {
            r.i = i;
            return r;
        }
        let mut e: i32 = 0;
        while i < s.len() && (s[i].is_ascii_digit() || s[i] == b'_') {
            if s[i] == b'_' {
                underscores = true;
                i += 1;
                continue;
            }
            if e < 10000 {
                e = e * 10 + i32::from(s[i] - b'0');
            }
            i += 1;
        }
        dp += e * esign;
    } else if base == 16 {
        // Must have exponent.
        r.i = i;
        return r;
    }

    if r.mantissa != 0 {
        r.exp = dp - nd_mant;
    }

    if underscores && !underscore_ok(&s[..i]) {
        r.i = i;
        return r;
    }

    r.i = i;
    r.ok = true;
    r
}

/// Go's `atofHex`: converts the hex floating-point string s to a rounded float64 or float32 value
/// (depending on `float32`). s is known to be a syntactically valid number.
fn atof_hex(
    float32: bool,
    mut mantissa: u64,
    mut exp: i32,
    neg: bool,
    trunc: bool,
) -> (f64, Option<NumErr>) {
    let (mantbits, expbits, bias): (u32, u32, i32) =
        if float32 { (23, 8, -127) } else { (52, 11, -1023) };
    let max_exp = (1 << expbits) + bias - 2;
    let min_exp = bias + 1;
    exp += mantbits as i32; // mantissa now implicitly divided by 2^mantbits.

    // Shift mantissa and exponent to bring representation into float range.
    // Eventually we want a mantissa with a leading 1-bit followed by mantbits other bits.
    // For rounding, we need two more, where the bottom bit represents whether that bit or any later bit was
    // non-zero. (If the mantissa has already lost non-zero bits, trunc is true, and we OR in a 1 below after
    // shifting left appropriately.)
    while mantissa != 0 && mantissa >> (mantbits + 2) == 0 {
        mantissa <<= 1;
        exp -= 1;
    }
    if trunc {
        mantissa |= 1;
    }
    while mantissa >> (1 + mantbits + 2) != 0 {
        mantissa = (mantissa >> 1) | (mantissa & 1);
        exp += 1;
    }

    // If exponent is too negative, denormalize in hopes of making it representable. (The -2 is for the
    // rounding bits.)
    while mantissa > 1 && exp < min_exp - 2 {
        mantissa = (mantissa >> 1) | (mantissa & 1);
        exp += 1;
    }

    // Round using two bottom bits.
    let mut round = mantissa & 3;
    mantissa >>= 2;
    round |= mantissa & 1; // round to even (round up if mantissa is odd)
    exp += 2;
    if round == 3 {
        mantissa += 1;
        if mantissa == 1 << (1 + mantbits) {
            mantissa >>= 1;
            exp += 1;
        }
    }

    if mantissa >> mantbits == 0 {
        // Denormal or zero.
        exp = bias;
    }
    let mut err = None;
    if exp > max_exp {
        // infinity and range error
        mantissa = 1 << mantbits;
        exp = max_exp + 1;
        err = Some(NumErr::Range);
    }

    let mut bits = mantissa & ((1 << mantbits) - 1);
    bits |= (((exp - bias) & ((1 << expbits) - 1)) as u64) << mantbits;
    if neg {
        bits |= 1 << mantbits << expbits;
    }
    if float32 {
        return (f64::from(f32::from_bits(bits as u32)), err);
    }
    (f64::from_bits(bits), err)
}

/// The decimal number `s[..n]` (already validated by `read_float`) as the nearest float.
fn atof_decimal(s: &[u8], float32: bool) -> (f64, Option<NumErr>) {
    let mut canonical = String::with_capacity(s.len() + 8);
    let mut i = 0;
    if s[0] == b'+' || s[0] == b'-' {
        if s[0] == b'-' {
            canonical.push('-');
        }
        i = 1;
    }
    let mut frac_digits: i64 = 0;
    let mut sawdot = false;
    let mut any = false;
    while i < s.len() {
        let c = s[i];
        if c == b'_' {
        } else if c == b'.' {
            sawdot = true;
        } else if c.is_ascii_digit() {
            canonical.push(char::from(c));
            any = true;
            if sawdot {
                frac_digits += 1;
            }
        } else {
            break;
        }
        i += 1;
    }
    if !any {
        canonical.push('0');
    }
    let mut e: i64 = 0;
    if i < s.len() && lower(s[i]) == b'e' {
        i += 1;
        let mut esign = 1;
        if s[i] == b'+' {
            i += 1;
        } else if s[i] == b'-' {
            esign = -1;
            i += 1;
        }
        let mut v: i64 = 0;
        while i < s.len() {
            if s[i].is_ascii_digit() && v < 10000 {
                v = v * 10 + i64::from(s[i] - b'0');
            }
            i += 1;
        }
        e = v * esign;
    }
    canonical.push('e');
    canonical.push_str(&(e - frac_digits).to_string());
    let f = if float32 {
        f64::from(canonical.parse::<f32>().expect("canonical decimal"))
    } else {
        canonical.parse::<f64>().expect("canonical decimal")
    };
    if f.is_infinite() { (f, Some(NumErr::Range)) } else { (f, None) }
}

fn atof(s: &[u8], float32: bool) -> (f64, usize, Option<NumErr>) {
    if let Some((val, n)) = special(s) {
        return (if float32 { f64::from(val as f32) } else { val }, n, None);
    }
    let r = read_float(s);
    if !r.ok {
        return (0.0, r.i, Some(NumErr::Syntax));
    }
    if r.hex {
        let (f, err) = atof_hex(float32, r.mantissa, r.exp, r.neg, r.trunc);
        return (f, r.i, err);
    }
    let (f, err) = atof_decimal(&s[..r.i], float32);
    (f, r.i, err)
}

/// Go's `strconv.ParseFloat(s, bitSize)`.
///
/// Go's float syntax (decimal or `0x` hex mantissa, `_` separators as in Go literals, "Inf" / "Infinity"
/// / "NaN" in any case), the nearest float, `*NumError` with `ErrSyntax` or `ErrRange`. On a range error
/// Go also returns ±Inf; [`parse_float_value`] returns that value with the error.
pub fn parse_float(s: &str, bit_size: u32) -> Result<f64, Error> {
    let (f, err) = parse_float_value(s, bit_size);
    match err {
        Some(err) => Err(err),
        None => Ok(f),
    }
}

/// Go's `strconv.ParseFloat`, returning the value Go returns even with an error.
pub fn parse_float_value(s: &str, bit_size: u32) -> (f64, Option<Error>) {
    let (f, n, err) = atof(s.as_bytes(), bit_size == 32);
    if n != s.len() {
        return (0.0, Some(to_error("ParseFloat", s, 0, bit_size, NumErr::Syntax)));
    }
    match err {
        Some(err) => (f, Some(to_error("ParseFloat", s, 0, bit_size, err))),
        None => (f, None),
    }
}
