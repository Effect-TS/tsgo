//! Go's `fmt/format.go`: the raw formatter (padding, integers, floats, strings, quoting).

use crate::strconv;
use crate::utf8::{MAX_RUNE, RUNE_ERROR, append_rune, decode_rune, rune_count};

pub(super) const LDIGITS: &[u8; 17] = b"0123456789abcdefx";
pub(super) const UDIGITS: &[u8; 17] = b"0123456789ABCDEFX";

pub(super) const SIGNED: bool = true;
pub(super) const UNSIGNED: bool = false;

/// flags placed in a separate struct for easy clearing.
#[derive(Clone, Copy, Default)]
pub(super) struct FmtFlags {
    pub(super) wid_present: bool,
    pub(super) prec_present: bool,
    pub(super) minus: bool,
    pub(super) plus: bool,
    pub(super) sharp: bool,
    pub(super) space: bool,
    pub(super) zero: bool,

    // For the formats %+v %#v, we set the plusV/sharpV flags and clear the plus/sharp flags since %+v and
    // %#v are in effect different, flagless formats set at the top level.
    pub(super) plus_v: bool,
    pub(super) sharp_v: bool,
}

/// A fmt is the raw formatter used by Printf etc. It prints into a buffer that must be set up separately.
#[derive(Default)]
pub(super) struct Fmt {
    pub(super) flags: FmtFlags,
    pub(super) wid: i64,  // width
    pub(super) prec: i64, // precision
}

impl Fmt {
    pub(super) fn clearflags(&mut self) {
        self.flags = FmtFlags::default();
        self.wid = 0;
        self.prec = 0;
    }

    /// writePadding generates n bytes of padding.
    pub(super) fn write_padding(&self, buf: &mut Vec<u8>, n: i64) {
        if n <= 0 {
            // No padding bytes needed.
            return;
        }
        // Decide which byte the padding should be filled with.
        let mut pad_byte = b' ';
        // Zero padding is allowed only to the left.
        if self.flags.zero && !self.flags.minus {
            pad_byte = b'0';
        }
        buf.extend(std::iter::repeat_n(pad_byte, n as usize));
    }

    /// pad appends b to f.buf, padded on left (!f.minus) or right (f.minus).
    pub(super) fn pad(&self, buf: &mut Vec<u8>, b: &[u8]) {
        if !self.flags.wid_present || self.wid == 0 {
            buf.extend_from_slice(b);
            return;
        }
        let width = self.wid - rune_count(b) as i64;
        if !self.flags.minus {
            // left padding
            self.write_padding(buf, width);
            buf.extend_from_slice(b);
        } else {
            // right padding
            buf.extend_from_slice(b);
            self.write_padding(buf, width);
        }
    }

    /// padString appends s to f.buf, padded on left (!f.minus) or right (f.minus).
    pub(super) fn pad_string(&self, buf: &mut Vec<u8>, s: &[u8]) {
        self.pad(buf, s);
    }

    /// fmtBoolean formats a boolean.
    pub(super) fn fmt_boolean(&self, buf: &mut Vec<u8>, v: bool) {
        if v {
            self.pad_string(buf, b"true");
        } else {
            self.pad_string(buf, b"false");
        }
    }

    /// fmtUnicode formats a uint64 as "U+0078" or with f.sharp set as "U+0078 'x'".
    pub(super) fn fmt_unicode(&mut self, buf: &mut Vec<u8>, mut u: u64) {
        let orig = u;
        let mut prec: i64 = 4;
        if self.flags.prec_present && self.prec > 4 {
            prec = self.prec;
        }
        let mut digits = Vec::new();
        // Format the Unicode code point u as a hexadecimal number.
        while u >= 16 {
            digits.push(UDIGITS[(u & 0xF) as usize]);
            prec -= 1;
            u >>= 4;
        }
        digits.push(UDIGITS[u as usize]);
        prec -= 1;
        // Add zeros in front of the number until requested precision is reached.
        while prec > 0 {
            digits.push(b'0');
            prec -= 1;
        }
        digits.extend_from_slice(b"+U");
        digits.reverse();
        // For %#U we want to add a space and a quoted character at the end of the buffer.
        if self.flags.sharp
            && orig <= u64::from(MAX_RUNE)
            && char::from_u32(orig as u32).is_some_and(strconv::is_print)
        {
            digits.extend_from_slice(b" '");
            append_rune(&mut digits, orig as u32);
            digits.push(b'\'');
        }
        let old_zero = self.flags.zero;
        self.flags.zero = false;
        self.pad(buf, &digits);
        self.flags.zero = old_zero;
    }

    /// fmtInteger formats signed and unsigned integers.
    pub(super) fn fmt_integer(
        &mut self,
        buf: &mut Vec<u8>,
        mut u: u64,
        base: u32,
        is_signed: bool,
        verb: char,
        digits: &[u8; 17],
    ) {
        let negative = is_signed && (u as i64) < 0;
        if negative {
            u = u.wrapping_neg();
        }

        // Two ways to ask for extra leading zero digits: %.3d or %03d.
        // If both are specified the f.zero flag is ignored and padding with spaces is used instead.
        let mut prec: i64 = 0;
        if self.flags.prec_present {
            prec = self.prec;
            // Precision of 0 and value of 0 means "print nothing" but padding.
            if prec == 0 && u == 0 {
                let old_zero = self.flags.zero;
                self.flags.zero = false;
                self.write_padding(buf, self.wid);
                self.flags.zero = old_zero;
                return;
            }
        } else if self.flags.zero && !self.flags.minus && self.flags.wid_present {
            // Zero padding is allowed only to the left.
            prec = self.wid;
            if negative || self.flags.plus || self.flags.space {
                prec -= 1; // leave room for sign
            }
        }

        // Because printing is easier right-to-left: format u into buf, ending at buf[i].
        let mut out: Vec<u8> = Vec::with_capacity(24);
        match base {
            10 => {
                while u >= 10 {
                    let next = u / 10;
                    out.push(b'0' + (u - next * 10) as u8);
                    u = next;
                }
            }
            16 => {
                while u >= 16 {
                    out.push(digits[(u & 0xF) as usize]);
                    u >>= 4;
                }
            }
            8 => {
                while u >= 8 {
                    out.push(b'0' + (u & 7) as u8);
                    u >>= 3;
                }
            }
            2 => {
                while u >= 2 {
                    out.push(b'0' + (u & 1) as u8);
                    u >>= 1;
                }
            }
            _ => panic!("fmt: unknown base; can't happen"),
        }
        out.push(digits[u as usize]);
        while prec > out.len() as i64 {
            out.push(b'0');
        }

        // Various prefixes: 0x, -, etc.
        if self.flags.sharp {
            match base {
                2 => {
                    // Add a leading 0b.
                    out.push(b'b');
                    out.push(b'0');
                }
                8 => {
                    if out.last() != Some(&b'0') {
                        out.push(b'0');
                    }
                }
                16 => {
                    // Add a leading 0x or 0X.
                    out.push(digits[16]);
                    out.push(b'0');
                }
                _ => {}
            }
        }
        if verb == 'O' {
            out.push(b'o');
            out.push(b'0');
        }

        if negative {
            out.push(b'-');
        } else if self.flags.plus {
            out.push(b'+');
        } else if self.flags.space {
            out.push(b' ');
        }
        out.reverse();

        // Left padding with zeros has already been handled like precision earlier or the f.zero flag is
        // ignored due to an explicitly set precision.
        let old_zero = self.flags.zero;
        self.flags.zero = false;
        self.pad(buf, &out);
        self.flags.zero = old_zero;
    }

    /// truncate truncates the byte slice b as a string of the specified precision, if present.
    pub(super) fn truncate<'a>(&self, b: &'a [u8]) -> &'a [u8] {
        if self.flags.prec_present {
            let mut n = self.prec;
            let mut i = 0;
            while i < b.len() {
                n -= 1;
                if n < 0 {
                    return &b[..i];
                }
                let (_, wid) = decode_rune(&b[i..]);
                i += wid;
            }
        }
        b
    }

    /// fmtS formats a string.
    pub(super) fn fmt_s(&self, buf: &mut Vec<u8>, s: &[u8]) {
        let s = self.truncate(s);
        self.pad_string(buf, s);
    }

    /// fmtSbx formats a string or byte slice as a hexadecimal encoding of its bytes.
    pub(super) fn fmt_sbx(&self, buf: &mut Vec<u8>, b: &[u8], digits: &[u8; 17]) {
        let mut length = b.len() as i64;
        // Set length to not process more bytes than the precision demands.
        if self.flags.prec_present && self.prec < length {
            length = self.prec;
        }
        // Compute width of the encoding taking into account the f.sharp and f.space flag.
        let mut width = 2 * length;
        if width > 0 {
            if self.flags.space {
                // Each element encoded by two hexadecimals will get a leading 0x or 0X.
                if self.flags.sharp {
                    width *= 2;
                }
                // Elements will be separated by a space.
                width += length - 1;
            } else if self.flags.sharp {
                // Only a leading 0x or 0X will be added for the whole string.
                width += 2;
            }
        } else {
            // The byte slice or string that should be encoded is empty.
            if self.flags.wid_present {
                self.write_padding(buf, self.wid);
            }
            return;
        }
        // Handle padding to the left.
        if self.flags.wid_present && self.wid > width && !self.flags.minus {
            self.write_padding(buf, self.wid - width);
        }
        // Write the encoding directly into the output buffer.
        if self.flags.sharp {
            // Add leading 0x or 0X.
            buf.push(b'0');
            buf.push(digits[16]);
        }
        for (i, &c) in b[..length as usize].iter().enumerate() {
            if self.flags.space && i > 0 {
                // Separate elements with a space.
                buf.push(b' ');
                if self.flags.sharp {
                    // Add leading 0x or 0X for each element.
                    buf.push(b'0');
                    buf.push(digits[16]);
                }
            }
            // Encode each byte as two hexadecimal digits.
            buf.push(digits[(c >> 4) as usize]);
            buf.push(digits[(c & 0xF) as usize]);
        }
        // Handle padding to the right.
        if self.flags.wid_present && self.wid > width && self.flags.minus {
            self.write_padding(buf, self.wid - width);
        }
    }

    /// fmtQ formats a string as a double-quoted, escaped Go string constant. If f.sharp is set a raw
    /// (backquoted) string may be returned instead if the string does not contain any control characters
    /// other than tab.
    pub(super) fn fmt_q(&self, buf: &mut Vec<u8>, s: &[u8]) {
        let s = self.truncate(s);
        if self.flags.sharp && strconv::can_backquote(s) {
            let mut q = Vec::with_capacity(s.len() + 2);
            q.push(b'`');
            q.extend_from_slice(s);
            q.push(b'`');
            self.pad_string(buf, &q);
            return;
        }
        let mut q = Vec::new();
        if self.flags.plus {
            strconv::append_quote_to_ascii(&mut q, s);
        } else {
            strconv::append_quote(&mut q, s);
        }
        self.pad(buf, &q);
    }

    /// fmtC formats an integer as a Unicode character. If the character is not valid Unicode, it will print
    /// U+FFFD.
    pub(super) fn fmt_c(&self, buf: &mut Vec<u8>, c: u64) {
        // Explicitly check whether c exceeds utf8.MaxRune since the conversion of a uint64 to a rune may
        // lose precision that indicates an overflow.
        let r = if c > u64::from(MAX_RUNE) { RUNE_ERROR } else { c as u32 };
        let mut b = Vec::new();
        append_rune(&mut b, r);
        self.pad(buf, &b);
    }

    /// fmtQc formats an integer as a single-quoted, escaped Go character constant. If the character is not
    /// valid Unicode, it will print U+FFFD.
    pub(super) fn fmt_qc(&self, buf: &mut Vec<u8>, c: u64) {
        let r = if c > u64::from(MAX_RUNE) { RUNE_ERROR } else { c as u32 };
        let mut b = Vec::new();
        if self.flags.plus {
            strconv::append_quote_rune_to_ascii(&mut b, r);
        } else {
            strconv::append_quote_rune(&mut b, r);
        }
        self.pad(buf, &b);
    }

    /// fmtFloat formats a float64. It assumes that verb is a valid format specifier for
    /// strconv.AppendFloat and therefore fits into a byte.
    pub(super) fn fmt_float(
        &mut self,
        buf: &mut Vec<u8>,
        v: f64,
        size: u32,
        verb: char,
        mut prec: i64,
    ) {
        // Explicit precision in format specifier overrules default precision.
        if self.flags.prec_present {
            prec = self.prec;
        }
        // Format number, reserving space for leading + sign if needed.
        let mut num = vec![b'+'];
        strconv::append_float(&mut num, v, verb as u8, prec as i32, size);
        if num[1] == b'-' || num[1] == b'+' {
            num.remove(0);
        } else {
            num[0] = b'+';
        }
        // f.space means to add a leading space instead of a "+" sign unless the sign is explicitly asked for
        // by f.plus.
        if self.flags.space && num[0] == b'+' && !self.flags.plus {
            num[0] = b' ';
        }
        // Special handling for infinities and NaN, which don't look like a number so shouldn't be padded
        // with zeros.
        if num[1] == b'I' || num[1] == b'N' {
            let old_zero = self.flags.zero;
            self.flags.zero = false;
            // Remove sign before NaN if not asked for.
            if num[1] == b'N' && !self.flags.space && !self.flags.plus {
                num.remove(0);
            }
            self.pad(buf, &num);
            self.flags.zero = old_zero;
            return;
        }
        // The sharp flag forces printing a decimal point for non-binary formats and retains trailing zeros,
        // which we may need to restore.
        if self.flags.sharp && verb != 'b' {
            let mut digits: i64 = 0;
            if matches!(verb, 'v' | 'g' | 'G' | 'x') {
                digits = prec;
                // If no precision is set explicitly use a precision of 6.
                if digits == -1 {
                    digits = 6;
                }
            }

            // Buffer pre-allocated with enough room for exponent notations of the form "e+123" or "p-1023".
            let mut tail: Vec<u8> = Vec::new();

            let mut has_decimal_point = false;
            let mut saw_nonzero_digit = false;
            // Starting from i = 1 to skip sign at num[0].
            let mut i = 1;
            while i < num.len() {
                match num[i] {
                    b'.' => has_decimal_point = true,
                    b'p' | b'P' => {
                        tail.extend_from_slice(&num[i..]);
                        num.truncate(i);
                    }
                    b'e' | b'E' if verb != 'x' && verb != 'X' => {
                        tail.extend_from_slice(&num[i..]);
                        num.truncate(i);
                    }
                    c => {
                        if c != b'0' {
                            saw_nonzero_digit = true;
                        }
                        // Count significant digits after the first non-zero digit.
                        if saw_nonzero_digit {
                            digits -= 1;
                        }
                    }
                }
                i += 1;
            }
            if !has_decimal_point {
                // Leading digit 0 should contribute once to digits.
                if num.len() == 2 && num[1] == b'0' {
                    digits -= 1;
                }
                num.push(b'.');
            }
            while digits > 0 {
                num.push(b'0');
                digits -= 1;
            }
            num.extend_from_slice(&tail);
        }
        // We want a sign if asked for and if the sign is not positive.
        if self.flags.plus || num[0] != b'+' {
            // If we're zero padding to the left we want the sign before the leading zeros. Achieve this by
            // writing the sign out and then padding the unsigned number. Zero padding is allowed only to the
            // left.
            if self.flags.zero
                && !self.flags.minus
                && self.flags.wid_present
                && self.wid > num.len() as i64
            {
                buf.push(num[0]);
                self.write_padding(buf, self.wid - num.len() as i64);
                buf.extend_from_slice(&num[1..]);
                return;
            }
            self.pad(buf, &num);
            return;
        }
        // No sign to show and the number is positive; just print the unsigned number.
        self.pad(buf, &num[1..]);
    }
}
