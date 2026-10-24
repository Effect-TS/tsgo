//! Go's `internal/strconv/ftoa.go`: `FormatFloat` / `AppendFloat`.
//!
//! The digit generation differs from Go's code but not in result: the shortest digits (`prec < 0`) come from
//! Rust's shortest round-trip formatter, and fixed precisions from Go's exact decimal (`decimal.go`), which
//! is correctly rounded (ties to even) like Go's fast path. Layout (`fmtEFG`, `fmtB`, `fmtX`) is Go's code.

use super::decimal::Decimal;

const LOWERHEX: &[u8; 16] = b"0123456789abcdef";
const UPPERHEX: &[u8; 16] = b"0123456789ABCDEF";

struct FloatInfo {
    mantbits: u32,
    expbits: u32,
    bias: i32,
}

const FLOAT32_INFO: FloatInfo = FloatInfo { mantbits: 23, expbits: 8, bias: -127 };
const FLOAT64_INFO: FloatInfo = FloatInfo { mantbits: 52, expbits: 11, bias: -1023 };

/// Go's `strconv.FormatFloat(f, fmt, prec, bitSize)`.
///
/// `fmt` is one of `b e E f g G x X` (anything else gives `%` followed by the byte, as in Go), `prec` -1
/// means the fewest digits that round-trip, `bit_size` is 32 or 64.
pub fn format_float(f: f64, fmt: u8, prec: i32, bit_size: u32) -> String {
    let mut dst = Vec::with_capacity((prec + 4).max(24) as usize);
    append_float(&mut dst, f, fmt, prec, bit_size);
    String::from_utf8(dst).expect("ASCII")
}

/// Go's `strconv.AppendFloat`.
pub fn append_float(dst: &mut Vec<u8>, f: f64, fmt: u8, prec: i32, bit_size: u32) {
    match bit_size {
        32 => genericftoa(dst, u64::from((f as f32).to_bits()), fmt, prec, &FLOAT32_INFO),
        64 => genericftoa(dst, f.to_bits(), fmt, prec, &FLOAT64_INFO),
        _ => panic!("strconv: illegal AppendFloat/FormatFloat bitSize"),
    }
}

fn genericftoa(dst: &mut Vec<u8>, bits: u64, fmt: u8, mut prec: i32, flt: &FloatInfo) {
    let neg = bits >> (flt.expbits + flt.mantbits) != 0;
    let mut exp = ((bits >> flt.mantbits) & ((1 << flt.expbits) - 1)) as i32;
    let mut mant = bits & ((1u64 << flt.mantbits) - 1);

    if exp == (1 << flt.expbits) - 1 {
        // Inf, NaN
        let s: &[u8] = if mant != 0 {
            b"NaN"
        } else if neg {
            b"-Inf"
        } else {
            b"+Inf"
        };
        dst.extend_from_slice(s);
        return;
    }
    if exp == 0 {
        // denormalized
        exp += 1;
    } else {
        // add implicit top bit
        mant |= 1u64 << flt.mantbits;
    }
    exp += flt.bias;

    // Pick off easy binary, hex formats.
    if fmt == b'b' {
        fmt_b(dst, neg, mant, exp - flt.mantbits as i32);
        return;
    }
    if fmt == b'x' || fmt == b'X' {
        fmt_x(dst, prec, fmt, neg, mant, exp, flt.mantbits);
        return;
    }

    if mant == 0 {
        fmt_efg(dst, neg, &[], 0, 0, prec, fmt, prec < 0);
        return;
    }

    if prec < 0 {
        // Shortest.
        let (digits, dp) = shortest_digits(bits, flt);
        let nd = digits.len() as i32;
        match fmt {
            b'e' | b'E' => prec = (nd - 1).max(0),
            b'f' => prec = (nd - dp).max(0),
            b'g' | b'G' => prec = nd,
            _ => {}
        }
        fmt_efg(dst, neg, &digits, dp, nd, prec, fmt, true);
        return;
    }

    let mut d = Decimal::new();
    d.assign(mant);
    d.shift(exp - flt.mantbits as i32);
    match fmt {
        b'e' | b'E' => d.round(prec + 1),
        b'f' => d.round(d.dp + prec),
        b'g' | b'G' => {
            if prec == 0 {
                prec = 1;
            }
            d.round(prec);
        }
        _ => {}
    }
    let nd = d.nd;
    fmt_efg(dst, neg, &d.d[..nd], d.dp, nd as i32, prec, fmt, false);
}

/// The shortest decimal digits that round-trip to the float (no trailing zeros) and the decimal point
/// position: the value is `0.digits * 10^dp`.
fn shortest_digits(bits: u64, flt: &FloatInfo) -> (Vec<u8>, i32) {
    let s = if flt.mantbits == FLOAT32_INFO.mantbits {
        format!("{:e}", f32::from_bits(bits as u32).abs())
    } else {
        format!("{:e}", f64::from_bits(bits).abs())
    };
    let (mantissa, exponent) = s.split_once('e').expect("Rust's exponent format has an exponent");
    let mut digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).collect();
    while digits.len() > 1 && digits.last() == Some(&b'0') {
        digits.pop();
    }
    let exponent: i32 = exponent.parse().expect("Rust's exponent is an integer");
    (digits, exponent + 1)
}

/// %e is used if the exponent from the conversion is less than -4 or greater than or equal to the
/// precision. If precision was the shortest possible, use precision 6 for this decision.
fn fmt_efg(
    dst: &mut Vec<u8>,
    neg: bool,
    s: &[u8],
    dp: i32,
    nd: i32,
    mut prec: i32,
    mut fmt: u8,
    shortest: bool,
) {
    if fmt == b'g' || fmt == b'G' {
        let mut eprec = prec;
        if eprec > nd && nd >= dp {
            eprec = nd;
        }
        // %e is used if the exponent from the conversion is less than -4 or greater than or equal to the
        // precision. if precision was the shortest possible, use precision 6 for this decision.
        if shortest {
            eprec = 6;
        }
        let exp = dp - 1;
        if exp < -4 || exp >= eprec {
            if prec > nd {
                prec = nd;
            }
            prec -= 1;
            fmt = fmt + b'e' - b'g';
        } else {
            if prec > dp {
                prec = nd;
            }
            prec = (prec - dp).max(0);
            fmt = b'f';
        }
    }

    match fmt {
        b'e' | b'E' => {
            // %e: -d.ddddde±dd
            if neg {
                dst.push(b'-');
            }
            // first digit
            let mut ch = b'0';
            if nd != 0 {
                ch = s[0];
            }
            dst.push(ch);
            // .moredigits
            if prec > 0 {
                dst.push(b'.');
                let mut i = 1;
                let m = nd.min(prec + 1);
                if i < m {
                    dst.extend_from_slice(&s[i as usize..m as usize]);
                    i = m;
                }
                for _ in 0..(prec + 1 - i) {
                    dst.push(b'0');
                }
            }
            // e±
            dst.push(fmt);
            let mut exp = dp - 1;
            if nd == 0 {
                // special case: 0 has exponent 0
                exp = 0;
            }
            if exp < 0 {
                ch = b'-';
                exp = -exp;
            } else {
                ch = b'+';
            }
            dst.push(ch);
            // dd or ddd
            if exp < 10 {
                dst.extend_from_slice(&[b'0', exp as u8 + b'0']);
            } else if exp < 100 {
                dst.extend_from_slice(&[(exp / 10) as u8 + b'0', (exp % 10) as u8 + b'0']);
            } else {
                dst.extend_from_slice(&[
                    (exp / 100) as u8 + b'0',
                    ((exp / 10) % 10) as u8 + b'0',
                    (exp % 10) as u8 + b'0',
                ]);
            }
        }
        b'f' => {
            // %f: -ddddddd.ddddd
            if neg {
                dst.push(b'-');
            }
            // integer, padded with zeros as needed.
            if dp > 0 {
                let m = nd.min(dp);
                dst.extend_from_slice(&s[..m as usize]);
                for _ in 0..(dp - m) {
                    dst.push(b'0');
                }
            } else {
                dst.push(b'0');
            }
            // fraction
            if prec > 0 {
                dst.push(b'.');
                let lz = prec.min((-dp).max(0)); // leading zeros
                let off = dp + lz;
                let m = (prec - lz).min((nd - off).max(0)); // middle digits
                let tz = (prec - lz - m).max(0); // trailing zeros
                for _ in 0..lz {
                    dst.push(b'0');
                }
                for i in 0..m {
                    dst.push(s[(off + i) as usize]);
                }
                for _ in 0..tz {
                    dst.push(b'0');
                }
            }
        }
        _ => {
            // unknown format
            dst.push(b'%');
            dst.push(fmt);
        }
    }
}

/// %b: -ddddddddp±ddd
fn fmt_b(dst: &mut Vec<u8>, neg: bool, mant: u64, exp: i32) {
    if neg {
        dst.push(b'-');
    }
    super::append_uint(dst, mant, 10);
    dst.push(b'p');
    if exp >= 0 {
        dst.push(b'+');
    }
    super::append_int(dst, i64::from(exp), 10);
}

/// %x: -0x1.yyyyyyyyp±ddd or -0x0p+0. (y is hex digit, d is decimal digit)
fn fmt_x(
    dst: &mut Vec<u8>,
    prec: i32,
    fmt: u8,
    neg: bool,
    mut mant: u64,
    mut exp: i32,
    mantbits: u32,
) {
    if mant == 0 {
        exp = 0;
    }
    // Shift digits so leading 1 (if any) is at bit 1<<60.
    mant <<= 60 - mantbits;
    while mant != 0 && mant & (1 << 60) == 0 {
        mant <<= 1;
        exp -= 1;
    }
    // Round if requested.
    if (0..15).contains(&prec) {
        let shift = (prec * 4) as u32;
        let extra = (mant << shift) & ((1 << 60) - 1);
        mant >>= 60 - shift;
        if extra | (mant & 1) > 1 << 59 {
            mant += 1;
        }
        mant <<= 60 - shift;
        if mant & (1 << 61) != 0 {
            // Wrapped around.
            mant >>= 1;
            exp += 1;
        }
    }
    let hex = if fmt == b'X' { UPPERHEX } else { LOWERHEX };
    // sign, 0x, leading digit
    if neg {
        dst.push(b'-');
    }
    dst.extend_from_slice(&[b'0', fmt, b'0' + ((mant >> 60) & 1) as u8]);
    // .fraction
    mant <<= 4; // remove leading 0 or 1
    if prec < 0 && mant != 0 {
        dst.push(b'.');
        while mant != 0 {
            dst.push(hex[((mant >> 60) & 15) as usize]);
            mant <<= 4;
        }
    } else if prec > 0 {
        dst.push(b'.');
        for _ in 0..prec {
            dst.push(hex[((mant >> 60) & 15) as usize]);
            mant <<= 4;
        }
    }
    // p±
    let mut ch = b'P';
    if fmt == super::lower(fmt) {
        ch = b'p';
    }
    dst.push(ch);
    if exp < 0 {
        ch = b'-';
        exp = -exp;
    } else {
        ch = b'+';
    }
    dst.push(ch);
    // dd or ddd or dddd
    if exp < 100 {
        dst.extend_from_slice(&[(exp / 10) as u8 + b'0', (exp % 10) as u8 + b'0']);
    } else if exp < 1000 {
        dst.extend_from_slice(&[
            (exp / 100) as u8 + b'0',
            ((exp / 10) % 10) as u8 + b'0',
            (exp % 10) as u8 + b'0',
        ]);
    } else {
        dst.extend_from_slice(&[
            (exp / 1000) as u8 + b'0',
            ((exp / 100) % 10) as u8 + b'0',
            ((exp / 10) % 10) as u8 + b'0',
            (exp % 10) as u8 + b'0',
        ]);
    }
}
