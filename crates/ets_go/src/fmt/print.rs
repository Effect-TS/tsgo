//! Go's `fmt/print.go` (and `fmt/errors.go`, `internal/fmtsort`) over [`GoValue`].

use std::borrow::Cow;
use std::cmp::Ordering;

use super::format::{Fmt, LDIGITS, SIGNED, UDIGITS, UNSIGNED};
use super::{GoValue, Kind, Value};
use crate::Error;
use crate::errors::{GoError, Unwrapped};
use crate::utf8::decode_rune;

// Strings for use with buffer.WriteString. This is less overhead than using buffer.Write with byte arrays.
const COMMA_SPACE_STRING: &[u8] = b", ";
const NIL_ANGLE_STRING: &[u8] = b"<nil>";
const NIL_PAREN_STRING: &[u8] = b"(nil)";
const NIL_STRING: &[u8] = b"nil";
const MAP_STRING: &[u8] = b"map[";
const PERCENT_BANG_STRING: &[u8] = b"%!";
const MISSING_STRING: &[u8] = b"(MISSING)";
const BAD_INDEX_STRING: &[u8] = b"(BADINDEX)";
const EXTRA_STRING: &[u8] = b"%!(EXTRA ";
const BAD_WIDTH_STRING: &[u8] = b"%!(BADWIDTH)";
const BAD_PREC_STRING: &[u8] = b"%!(BADPREC)";
const NO_VERB_STRING: &[u8] = b"%!(NOVERB)";
const INV_REFLECT_STRING: &[u8] = b"<invalid reflect.Value>";

/// pp is used to store a printer's state.
#[derive(Default)]
struct Pp {
    buf: Vec<u8>,
    // arg holds the current item.
    fmt: Fmt,
    // reordered records whether the format string used argument reordering.
    reordered: bool,
    // goodArgNum records whether the most recent reordering directive was valid.
    good_arg_num: bool,
    // erroring is set when printing an error string to guard against calling handleMethods.
    erroring: bool,
    // wrapErrs is set when the format string may contain a %w verb.
    wrap_errs: bool,
    // wrappedErrs records the targets of the %w verb.
    wrapped_errs: Vec<usize>,
}

fn into_string(buf: Vec<u8>) -> String {
    // Go strings are bytes; the only way to get invalid UTF-8 here is a `%s` of bytes that are not UTF-8,
    // which Rust strings cannot hold.
    match String::from_utf8(buf) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    }
}

fn values<'a>(a: &'a [&'a dyn Value]) -> Vec<GoValue<'a>> {
    a.iter().map(|v| v.go_value()).collect()
}

/// Go's `fmt.Fprintf(w, format, a...)`.
pub fn fprintf(
    w: &mut dyn std::io::Write,
    format: &str,
    a: &[&dyn Value],
) -> std::io::Result<usize> {
    let mut p = Pp::default();
    p.do_printf(format, &values(a));
    w.write_all(&p.buf)?;
    Ok(p.buf.len())
}

/// Go's `fmt.Sprintf(format, a...)`.
pub fn sprintf(format: &str, a: &[&dyn Value]) -> String {
    let mut p = Pp::default();
    p.do_printf(format, &values(a));
    into_string(p.buf)
}

/// Go's `fmt.Appendf(b, format, a...)`.
pub fn append_f(b: &mut Vec<u8>, format: &str, a: &[&dyn Value]) {
    let mut p = Pp::default();
    p.do_printf(format, &values(a));
    b.extend_from_slice(&p.buf);
}

/// Go's `fmt.Fprint(w, a...)`.
pub fn fprint(w: &mut dyn std::io::Write, a: &[&dyn Value]) -> std::io::Result<usize> {
    let mut p = Pp::default();
    p.do_print(&values(a));
    w.write_all(&p.buf)?;
    Ok(p.buf.len())
}

/// Go's `fmt.Sprint(a...)`: spaces are added between operands when neither is a string.
pub fn sprint(a: &[&dyn Value]) -> String {
    let mut p = Pp::default();
    p.do_print(&values(a));
    into_string(p.buf)
}

/// Go's `fmt.Fprintln(w, a...)`.
pub fn fprintln(w: &mut dyn std::io::Write, a: &[&dyn Value]) -> std::io::Result<usize> {
    let mut p = Pp::default();
    p.do_println(&values(a));
    w.write_all(&p.buf)?;
    Ok(p.buf.len())
}

/// Go's `fmt.Sprintln(a...)`: spaces between operands and a newline.
pub fn sprintln(a: &[&dyn Value]) -> String {
    let mut p = Pp::default();
    p.do_println(&values(a));
    into_string(p.buf)
}

/// Go's `fmt.Errorf(format, a...)`.
///
/// If the format specifier includes a `%w` verb with an error operand, the returned error wraps it
/// (`errors.Unwrap` returns it); with several `%w` verbs it wraps all of them, in argument order. A `%w`
/// operand that is not an error is an error in the text (`%!w(int=1)`), as in Go.
pub fn errorf(format: &str, a: &[&dyn Value]) -> Error {
    if a.is_empty() && !format.contains('%') {
        // No formatting was needed.
        return crate::errors::new(format);
    }
    let mut p = Pp { wrap_errs: true, ..Pp::default() };
    p.do_printf(format, &values(a));
    let s = into_string(p.buf);
    match p.wrapped_errs.len() {
        0 => crate::errors::new(s),
        1 => Error::new(WrapError { msg: s, err: a[p.wrapped_errs[0]].as_error() }),
        _ => {
            if p.reordered {
                p.wrapped_errs.sort_unstable();
            }
            let mut errs = Vec::new();
            for (i, &arg_num) in p.wrapped_errs.iter().enumerate() {
                if i > 0 && p.wrapped_errs[i - 1] == arg_num {
                    continue;
                }
                if let Some(e) = a[arg_num].as_error() {
                    errs.push(e);
                }
            }
            Error::new(WrapErrors { msg: s, errs })
        }
    }
}

/// Go's `*fmt.wrapError`.
pub struct WrapError {
    msg: String,
    err: Option<Error>,
}

impl GoError for WrapError {
    fn error(&self) -> String {
        self.msg.clone()
    }

    fn type_name(&self) -> Cow<'static, str> {
        Cow::Borrowed("*fmt.wrapError")
    }

    fn unwrap(&self) -> Unwrapped<'_> {
        match &self.err {
            Some(e) => Unwrapped::One(e),
            None => Unwrapped::None,
        }
    }
}

/// Go's `*fmt.wrapErrors`.
pub struct WrapErrors {
    msg: String,
    errs: Vec<Error>,
}

impl GoError for WrapErrors {
    fn error(&self) -> String {
        self.msg.clone()
    }

    fn type_name(&self) -> Cow<'static, str> {
        Cow::Borrowed("*fmt.wrapErrors")
    }

    fn unwrap(&self) -> Unwrapped<'_> {
        Unwrapped::Many(&self.errs)
    }
}

/// tooLarge reports whether the magnitude of the integer is too large to be used as a formatting width or
/// precision.
fn too_large(x: i64) -> bool {
    const MAX: i64 = 1_000_000;
    !(-MAX..=MAX).contains(&x)
}

/// parsenum converts ASCII to integer. num is 0 (and isnum is false) if no number present.
fn parsenum(s: &[u8], start: usize, end: usize) -> (i64, bool, usize) {
    if start >= end {
        return (0, false, end);
    }
    let mut num: i64 = 0;
    let mut isnum = false;
    let mut newi = start;
    while newi < end && s[newi].is_ascii_digit() {
        if too_large(num) {
            return (0, false, end); // Overflow; crazy long number most likely.
        }
        num = num * 10 + i64::from(s[newi] - b'0');
        isnum = true;
        newi += 1;
    }
    (num, isnum, newi)
}

/// The integer value of a width or precision operand (Go's `intFromArg` accepts any integer kind).
fn int_from_arg(a: &[GoValue<'_>], arg_num: usize) -> (i64, bool, usize) {
    let mut new_arg_num = arg_num;
    let mut num = 0;
    let mut is_int = false;
    if arg_num < a.len() {
        match a[arg_num].underlying() {
            GoValue::Int(n, _) => {
                num = *n;
                is_int = true;
            }
            GoValue::Uint(n, _, _) if i64::try_from(*n).is_ok() => {
                num = *n as i64;
                is_int = true;
            }
            _ => {}
        }
        new_arg_num = arg_num + 1;
        if too_large(num) {
            num = 0;
            is_int = false; // Argument too large.
        }
    }
    (num, is_int, new_arg_num)
}

/// parseArgNumber returns the value of the bracketed number, minus 1 (explicit argument numbers are
/// one-indexed but we want zero-indexed). The opening bracket is known to be present at format[0]. The
/// returned values are the index, the number of bytes to consume up to the closing paren, if present, and
/// whether the number parsed ok. The bytes to consume will be 1 if no closing paren is present.
fn parse_arg_number(format: &[u8]) -> (i64, usize, bool) {
    // There must be at least 3 bytes: [n].
    if format.len() < 3 {
        return (0, 1, false);
    }
    // Find closing bracket.
    for i in 1..format.len() {
        if format[i] == b']' {
            let (width, ok, newi) = parsenum(format, 1, i);
            if !ok || newi != i {
                return (0, i + 1, false);
            }
            return (width - 1, i + 1, true); // arg numbers are one-indexed and skip paren.
        }
    }
    (0, 1, false)
}

/// fmtsort's `compare`: the order `fmt` prints map keys in.
fn compare(a: &GoValue<'_>, b: &GoValue<'_>) -> Ordering {
    let (a, b) = (a.underlying(), b.underlying());
    if a.type_name() != b.type_name() {
        return Ordering::Less; // No good answer possible, but don't return 0: they're not equal.
    }
    match (a, b) {
        (GoValue::Int(x, _), GoValue::Int(y, _)) => x.cmp(y),
        (GoValue::Uint(x, _, _), GoValue::Uint(y, _, _)) => x.cmp(y),
        (GoValue::Str(x, _), GoValue::Str(y, _)) => x.cmp(y),
        // cmp.Compare: NaN sorts before any other value, NaNs are equal.
        (GoValue::Float(x, _, _), GoValue::Float(y, _, _)) => match (x.is_nan(), y.is_nan()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        },
        (GoValue::Bool(x, _), GoValue::Bool(y, _)) => x.cmp(y),
        (GoValue::Pointer { addr: x, .. }, GoValue::Pointer { addr: y, .. })
        | (GoValue::Opaque { addr: x, .. }, GoValue::Opaque { addr: y, .. }) => x.cmp(y),
        (GoValue::Struct { fields: x, .. }, GoValue::Struct { fields: y, .. }) => {
            for (fx, fy) in x.iter().zip(y) {
                let c = compare(&fx.value, &fy.value);
                if c != Ordering::Equal {
                    return c;
                }
            }
            Ordering::Equal
        }
        (GoValue::Array { elems: x, .. }, GoValue::Array { elems: y, .. }) => {
            for (ex, ey) in x.iter().zip(y) {
                let c = compare(ex, ey);
                if c != Ordering::Equal {
                    return c;
                }
            }
            Ordering::Equal
        }
        (GoValue::Interface { elem: x, .. }, GoValue::Interface { elem: y, .. }) => match (x, y) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(x), Some(y)) => compare(x, y),
        },
        _ => panic!("bad type in compare: {}", a.type_name()),
    }
}

impl Pp {
    fn write_rune(&mut self, r: char) {
        let mut tmp = [0; 4];
        self.buf.extend_from_slice(r.encode_utf8(&mut tmp).as_bytes());
    }

    fn bad_verb(&mut self, value: Option<&GoValue<'_>>, verb: char) {
        self.erroring = true;
        self.buf.extend_from_slice(PERCENT_BANG_STRING);
        self.write_rune(verb);
        self.buf.push(b'(');
        match value {
            Some(v) if !matches!(v, GoValue::Nil) => {
                self.buf.extend_from_slice(v.type_name().as_bytes());
                self.buf.push(b'=');
                self.print_value(v, 'v', 0, true);
            }
            _ => self.buf.extend_from_slice(NIL_ANGLE_STRING),
        }
        self.buf.push(b')');
        self.erroring = false;
    }

    fn fmt_bool(&mut self, value: &GoValue<'_>, v: bool, verb: char) {
        match verb {
            't' | 'v' => self.fmt.fmt_boolean(&mut self.buf, v),
            _ => self.bad_verb(Some(value), verb),
        }
    }

    /// fmt0x64 formats a uint64 in hexadecimal and prefixes it with 0x or leaves it alone, as requested,
    /// by temporarily setting the sharp flag.
    fn fmt_0x64(&mut self, v: u64, leading0x: bool) {
        let sharp = self.fmt.flags.sharp;
        self.fmt.flags.sharp = leading0x;
        self.fmt.fmt_integer(&mut self.buf, v, 16, UNSIGNED, 'v', LDIGITS);
        self.fmt.flags.sharp = sharp;
    }

    /// fmtInteger formats a signed or unsigned integer.
    fn fmt_integer(&mut self, value: &GoValue<'_>, v: u64, is_signed: bool, verb: char) {
        match verb {
            'v' => {
                if self.fmt.flags.sharp_v && !is_signed {
                    self.fmt_0x64(v, true);
                } else {
                    self.fmt.fmt_integer(&mut self.buf, v, 10, is_signed, verb, LDIGITS);
                }
            }
            'd' => self.fmt.fmt_integer(&mut self.buf, v, 10, is_signed, verb, LDIGITS),
            'b' => self.fmt.fmt_integer(&mut self.buf, v, 2, is_signed, verb, LDIGITS),
            'o' | 'O' => self.fmt.fmt_integer(&mut self.buf, v, 8, is_signed, verb, LDIGITS),
            'x' => self.fmt.fmt_integer(&mut self.buf, v, 16, is_signed, verb, LDIGITS),
            'X' => self.fmt.fmt_integer(&mut self.buf, v, 16, is_signed, verb, UDIGITS),
            'c' => self.fmt.fmt_c(&mut self.buf, v),
            'q' => self.fmt.fmt_qc(&mut self.buf, v),
            'U' => self.fmt.fmt_unicode(&mut self.buf, v),
            _ => self.bad_verb(Some(value), verb),
        }
    }

    /// fmtFloat formats a float. The default precision for each verb is specified as last argument in the
    /// call to `fmt_float`.
    fn fmt_float(&mut self, value: &GoValue<'_>, v: f64, size: u32, verb: char) {
        match verb {
            'v' => self.fmt.fmt_float(&mut self.buf, v, size, 'g', -1),
            'b' | 'g' | 'G' | 'x' | 'X' => self.fmt.fmt_float(&mut self.buf, v, size, verb, -1),
            'f' | 'e' | 'E' => self.fmt.fmt_float(&mut self.buf, v, size, verb, 6),
            'F' => self.fmt.fmt_float(&mut self.buf, v, size, 'f', 6),
            _ => self.bad_verb(Some(value), verb),
        }
    }

    fn fmt_string(&mut self, value: Option<&GoValue<'_>>, v: &[u8], verb: char) {
        match verb {
            'v' => {
                if self.fmt.flags.sharp_v {
                    self.fmt.fmt_q(&mut self.buf, v);
                } else {
                    self.fmt.fmt_s(&mut self.buf, v);
                }
            }
            's' => self.fmt.fmt_s(&mut self.buf, v),
            'x' => self.fmt.fmt_sbx(&mut self.buf, v, LDIGITS),
            'X' => self.fmt.fmt_sbx(&mut self.buf, v, UDIGITS),
            'q' => self.fmt.fmt_q(&mut self.buf, v),
            _ => self.bad_verb(value, verb),
        }
    }

    fn fmt_bytes(&mut self, v: Option<&[u8]>, verb: char, type_string: &str, value: &GoValue<'_>) {
        match verb {
            'v' | 'd' => {
                if self.fmt.flags.sharp_v {
                    self.buf.extend_from_slice(type_string.as_bytes());
                    let Some(v) = v else {
                        self.buf.extend_from_slice(NIL_PAREN_STRING);
                        return;
                    };
                    self.buf.push(b'{');
                    for (i, &c) in v.iter().enumerate() {
                        if i > 0 {
                            self.buf.extend_from_slice(COMMA_SPACE_STRING);
                        }
                        self.fmt_0x64(u64::from(c), true);
                    }
                    self.buf.push(b'}');
                } else {
                    self.buf.push(b'[');
                    for (i, &c) in v.unwrap_or_default().iter().enumerate() {
                        if i > 0 {
                            self.buf.push(b' ');
                        }
                        self.fmt.fmt_integer(
                            &mut self.buf,
                            u64::from(c),
                            10,
                            UNSIGNED,
                            verb,
                            LDIGITS,
                        );
                    }
                    self.buf.push(b']');
                }
            }
            's' => {
                let b = v.unwrap_or_default();
                let b = self.fmt.truncate(b);
                self.fmt.pad(&mut self.buf, b);
            }
            'x' => self.fmt.fmt_sbx(&mut self.buf, v.unwrap_or_default(), LDIGITS),
            'X' => self.fmt.fmt_sbx(&mut self.buf, v.unwrap_or_default(), UDIGITS),
            'q' => self.fmt.fmt_q(&mut self.buf, v.unwrap_or_default()),
            _ => self.print_value(value, verb, 0, true),
        }
    }

    fn fmt_pointer(&mut self, value: &GoValue<'_>, verb: char) {
        let u = match value.underlying() {
            GoValue::Pointer { addr, .. } | GoValue::Opaque { addr, .. } => *addr as u64,
            // Maps and slices are reference kinds too; there is no address to print for them here.
            GoValue::Map { entries, .. } => u64::from(entries.is_some()),
            GoValue::Slice { elems, .. } => u64::from(elems.is_some()),
            _ => {
                self.bad_verb(Some(value), verb);
                return;
            }
        };
        match verb {
            'v' => {
                if self.fmt.flags.sharp_v {
                    self.buf.push(b'(');
                    self.buf.extend_from_slice(value.type_name().as_bytes());
                    self.buf.extend_from_slice(b")(");
                    if u == 0 {
                        self.buf.extend_from_slice(NIL_STRING);
                    } else {
                        self.fmt_0x64(u, true);
                    }
                    self.buf.push(b')');
                } else if u == 0 {
                    self.fmt.pad_string(&mut self.buf, NIL_ANGLE_STRING);
                } else {
                    self.fmt_0x64(u, !self.fmt.flags.sharp);
                }
            }
            'p' => self.fmt_0x64(u, !self.fmt.flags.sharp),
            'b' | 'o' | 'd' | 'x' | 'X' => self.fmt_integer(value, u, UNSIGNED, verb),
            _ => self.bad_verb(Some(value), verb),
        }
    }

    fn handle_methods(&mut self, arg: &GoValue<'_>, verb: char) -> bool {
        if self.erroring {
            return false;
        }
        let GoValue::Methods { error, string, go_string, .. } = arg else {
            if verb == 'w' {
                // It is invalid to use %w other than with Errorf or with a non-error arg.
                self.bad_verb(Some(arg), verb);
                return true;
            }
            return false;
        };
        let mut verb = verb;
        if verb == 'w' {
            // It is invalid to use %w other than with Errorf or with a non-error arg.
            if error.is_none() || !self.wrap_errs {
                self.bad_verb(Some(arg), verb);
                return true;
            }
            // If the arg is a Formatter, pass 'v' as the verb to it.
            verb = 'v';
        }
        if self.fmt.flags.sharp_v {
            if let Some(s) = go_string {
                // Print the result of GoString unadorned.
                self.fmt.fmt_s(&mut self.buf, s.as_bytes());
                return true;
            }
        } else if matches!(verb, 'v' | 's' | 'x' | 'X' | 'q') {
            // Is it an error or Stringer? The duplication in the bodies is necessary: setting handled and
            // deferring catchPanic must happen before calling the method.
            if let Some(e) = error {
                self.fmt_string(Some(arg), e.as_bytes(), verb);
                return true;
            }
            if let Some(s) = string {
                self.fmt_string(Some(arg), s.as_bytes(), verb);
                return true;
            }
        }
        false
    }

    fn print_arg(&mut self, arg: &GoValue<'_>, verb: char) {
        let arg = match arg {
            GoValue::Interface { elem: Some(e), .. } => &**e,
            a => a,
        };
        if matches!(arg, GoValue::Nil | GoValue::Interface { elem: None, .. }) {
            match verb {
                'T' | 'v' => self.fmt.pad_string(&mut self.buf, NIL_ANGLE_STRING),
                _ => self.bad_verb(None, verb),
            }
            return;
        }

        // Special processing considerations.
        // %T (the value's type) and %p (its address) are special; we always do them first.
        match verb {
            'T' => {
                let t = arg.type_name();
                self.fmt.fmt_s(&mut self.buf, t.as_bytes());
                return;
            }
            'p' => {
                self.fmt_pointer(arg, 'p');
                return;
            }
            _ => {}
        }

        // Some types can be done without reflection.
        match arg {
            GoValue::Bool(b, ty) if ty == "bool" => self.fmt_bool(arg, *b, verb),
            GoValue::Float(f, size, ty) if ty == "float64" || ty == "float32" => {
                self.fmt_float(arg, *f, *size, verb);
            }
            GoValue::Int(i, ty) if is_predeclared_int(ty) => {
                self.fmt_integer(arg, *i as u64, SIGNED, verb);
            }
            GoValue::Uint(u, ty, _) if is_predeclared_uint(ty) => {
                self.fmt_integer(arg, *u, UNSIGNED, verb);
            }
            GoValue::Str(s, ty) if ty == "string" => self.fmt_string(Some(arg), s, verb),
            GoValue::Slice { ty, elems, .. } if ty == "[]uint8" => {
                let bytes = elems.as_ref().map(|e| byte_values(e));
                self.fmt_bytes(bytes.as_deref(), verb, "[]byte", arg);
            }
            _ => {
                // If the type is not simple, it might have methods.
                if !self.handle_methods(arg, verb) {
                    // Need to use reflection, since the type had no interface methods that could be used
                    // for formatting.
                    self.print_value(arg, verb, 0, true);
                }
            }
        }
    }

    /// printValue is similar to printArg but starts with a reflect value, not an interface{} value. It
    /// does not handle 'p' and 'T' verbs because these should have been already handled by printArg.
    fn print_value(&mut self, value: &GoValue<'_>, verb: char, depth: usize, can_interface: bool) {
        // Handle values with special methods if not already handled by printArg (depth == 0).
        if depth > 0
            && !matches!(value, GoValue::Nil)
            && can_interface
            && self.handle_methods(value, verb)
        {
            return;
        }
        let f = value.underlying();
        match f {
            GoValue::Nil => {
                if depth == 0 {
                    self.buf.extend_from_slice(INV_REFLECT_STRING);
                } else {
                    match verb {
                        'v' => self.buf.extend_from_slice(NIL_ANGLE_STRING),
                        _ => self.bad_verb(None, verb),
                    }
                }
            }
            GoValue::Bool(b, _) => self.fmt_bool(f, *b, verb),
            GoValue::Int(i, _) => self.fmt_integer(f, *i as u64, SIGNED, verb),
            GoValue::Uint(u, _, _) => self.fmt_integer(f, *u, UNSIGNED, verb),
            GoValue::Float(x, size, _) => self.fmt_float(f, *x, *size, verb),
            GoValue::Str(s, _) => self.fmt_string(Some(f), s, verb),
            GoValue::Map { ty, entries } => {
                if self.fmt.flags.sharp_v {
                    self.buf.extend_from_slice(ty.as_bytes());
                    if entries.is_none() {
                        self.buf.extend_from_slice(NIL_PAREN_STRING);
                        return;
                    }
                    self.buf.push(b'{');
                } else {
                    self.buf.extend_from_slice(MAP_STRING);
                }
                let mut sorted: Vec<&(GoValue<'_>, GoValue<'_>)> =
                    entries.iter().flatten().collect();
                sorted.sort_by(|a, b| compare(&a.0, &b.0));
                for (i, (k, v)) in sorted.into_iter().enumerate() {
                    if i > 0 {
                        if self.fmt.flags.sharp_v {
                            self.buf.extend_from_slice(COMMA_SPACE_STRING);
                        } else {
                            self.buf.push(b' ');
                        }
                    }
                    self.print_value(k, verb, depth + 1, can_interface);
                    self.buf.push(b':');
                    self.print_value(v, verb, depth + 1, can_interface);
                }
                if self.fmt.flags.sharp_v {
                    self.buf.push(b'}');
                } else {
                    self.buf.push(b']');
                }
            }
            GoValue::Struct { ty, fields } => {
                if self.fmt.flags.sharp_v {
                    self.buf.extend_from_slice(ty.as_bytes());
                }
                self.buf.push(b'{');
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        if self.fmt.flags.sharp_v {
                            self.buf.extend_from_slice(COMMA_SPACE_STRING);
                        } else {
                            self.buf.push(b' ');
                        }
                    }
                    if (self.fmt.flags.plus_v || self.fmt.flags.sharp_v) && !field.name.is_empty() {
                        self.buf.extend_from_slice(field.name.as_bytes());
                        self.buf.push(b':');
                    }
                    // getField: a non-nil interface field is printed as its dynamic value.
                    let v = match &field.value {
                        GoValue::Interface { elem: Some(e), .. } => &**e,
                        v => v,
                    };
                    self.print_value(v, verb, depth + 1, can_interface && field.exported());
                }
                self.buf.push(b'}');
            }
            GoValue::Interface { ty, elem } => match elem {
                None => {
                    if self.fmt.flags.sharp_v {
                        self.buf.extend_from_slice(ty.as_bytes());
                        self.buf.extend_from_slice(NIL_PAREN_STRING);
                    } else {
                        self.buf.extend_from_slice(NIL_ANGLE_STRING);
                    }
                }
                Some(e) => self.print_value(e, verb, depth + 1, can_interface),
            },
            GoValue::Slice { ty, elem_kind, elems } => {
                self.print_list(
                    f,
                    ty,
                    *elem_kind,
                    elems.as_deref(),
                    true,
                    verb,
                    depth,
                    can_interface,
                );
            }
            GoValue::Array { ty, elem_kind, elems } => {
                self.print_list(f, ty, *elem_kind, Some(elems), false, verb, depth, can_interface);
            }
            GoValue::Pointer { addr, elem, .. } => {
                // pointer to array or slice or struct? ok at top level but not embedded (avoid loops)
                if depth == 0 && *addr != 0 {
                    if let Some(a) = elem {
                        if matches!(
                            a.underlying(),
                            GoValue::Array { .. }
                                | GoValue::Slice { .. }
                                | GoValue::Struct { .. }
                                | GoValue::Map { .. }
                        ) {
                            self.buf.push(b'&');
                            self.print_value(a, verb, depth + 1, can_interface);
                            return;
                        }
                    }
                }
                self.fmt_pointer(f, verb);
            }
            GoValue::Opaque { .. } => self.fmt_pointer(f, verb),
            GoValue::Methods { .. } => unreachable!("underlying() strips methods"),
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the arguments of Go's reflect.Array/reflect.Slice case"
    )]
    fn print_list(
        &mut self,
        f: &GoValue<'_>,
        ty: &str,
        elem_kind: Kind,
        elems: Option<&[GoValue<'_>]>,
        is_slice: bool,
        verb: char,
        depth: usize,
        can_interface: bool,
    ) {
        match verb {
            's' | 'q' | 'x' | 'X' if elem_kind == Kind::Uint8 => {
                // Handle byte and uint8 slices and arrays special for the above verbs.
                let bytes = elems.map(byte_values);
                self.fmt_bytes(bytes.as_deref(), verb, ty, f);
                return;
            }
            _ => {}
        }
        if self.fmt.flags.sharp_v {
            self.buf.extend_from_slice(ty.as_bytes());
            if is_slice && elems.is_none() {
                self.buf.extend_from_slice(NIL_PAREN_STRING);
                return;
            }
            self.buf.push(b'{');
            for (i, e) in elems.unwrap_or_default().iter().enumerate() {
                if i > 0 {
                    self.buf.extend_from_slice(COMMA_SPACE_STRING);
                }
                self.print_value(e, verb, depth + 1, can_interface);
            }
            self.buf.push(b'}');
        } else {
            self.buf.push(b'[');
            for (i, e) in elems.unwrap_or_default().iter().enumerate() {
                if i > 0 {
                    self.buf.push(b' ');
                }
                self.print_value(e, verb, depth + 1, can_interface);
            }
            self.buf.push(b']');
        }
    }

    /// argNumber returns the next argument to evaluate, which is either the value of the passed-in argNum
    /// or the value of the bracketed integer that begins format[i:]. It also returns the new value of i,
    /// that is, the index of the next byte of the format to process.
    fn arg_number(
        &mut self,
        arg_num: usize,
        format: &[u8],
        i: usize,
        num_args: usize,
    ) -> (usize, usize, bool) {
        if format.len() <= i || format[i] != b'[' {
            return (arg_num, i, false);
        }
        self.reordered = true;
        let (index, wid, ok) = parse_arg_number(&format[i..]);
        if ok && 0 <= index && (index as usize) < num_args {
            return (index as usize, i + wid, true);
        }
        self.good_arg_num = false;
        (arg_num, i + wid, ok)
    }

    fn bad_arg_num(&mut self, verb: char) {
        self.buf.extend_from_slice(PERCENT_BANG_STRING);
        self.write_rune(verb);
        self.buf.extend_from_slice(BAD_INDEX_STRING);
    }

    fn missing_arg(&mut self, verb: char) {
        self.buf.extend_from_slice(PERCENT_BANG_STRING);
        self.write_rune(verb);
        self.buf.extend_from_slice(MISSING_STRING);
    }

    fn do_printf(&mut self, format: &str, a: &[GoValue<'_>]) {
        let format = format.as_bytes();
        let end = format.len();
        let mut arg_num = 0; // we process one argument per non-trivial format
        let mut after_index; // previous item in format was an index like [3].
        self.reordered = false;
        let mut i = 0;
        'format_loop: while i < end {
            self.good_arg_num = true;
            let lasti = i;
            while i < end && format[i] != b'%' {
                i += 1;
            }
            if i > lasti {
                self.buf.extend_from_slice(&format[lasti..i]);
            }
            if i >= end {
                // done processing format string
                break;
            }

            // Process one verb
            i += 1;

            // Do we have flags?
            self.fmt.clearflags();
            while i < end {
                let c = format[i];
                match c {
                    b'#' => self.fmt.flags.sharp = true,
                    b'0' => self.fmt.flags.zero = true,
                    b'+' => self.fmt.flags.plus = true,
                    b'-' => self.fmt.flags.minus = true,
                    b' ' => self.fmt.flags.space = true,
                    _ => {
                        // Fast path for common case of ascii lower case simple verbs without precision or
                        // width or argument indices.
                        if c.is_ascii_lowercase() && arg_num < a.len() {
                            if c == b'w' {
                                self.wrapped_errs.push(arg_num);
                            }
                            if c == b'w' || c == b'v' {
                                // Go syntax
                                self.fmt.flags.sharp_v = self.fmt.flags.sharp;
                                self.fmt.flags.sharp = false;
                                // Struct-field syntax
                                self.fmt.flags.plus_v = self.fmt.flags.plus;
                                self.fmt.flags.plus = false;
                            }
                            self.print_arg(&a[arg_num], char::from(c));
                            arg_num += 1;
                            i += 1;
                            continue 'format_loop;
                        }
                        // Format is more complex than simple flags and a verb or is malformed.
                        break;
                    }
                }
                i += 1;
            }

            // Do we have an explicit argument index?
            (arg_num, i, after_index) = self.arg_number(arg_num, format, i, a.len());

            // Do we have width?
            if i < end && format[i] == b'*' {
                i += 1;
                (self.fmt.wid, self.fmt.flags.wid_present, arg_num) = int_from_arg(a, arg_num);

                if !self.fmt.flags.wid_present {
                    self.buf.extend_from_slice(BAD_WIDTH_STRING);
                }

                // We have a negative width, so take its value and ensure that the minus flag is set
                if self.fmt.wid < 0 {
                    self.fmt.wid = -self.fmt.wid;
                    self.fmt.flags.minus = true;
                    self.fmt.flags.zero = false; // Do not pad with zeros to the right.
                }
                after_index = false;
            } else {
                (self.fmt.wid, self.fmt.flags.wid_present, i) = parsenum(format, i, end);
                if after_index && self.fmt.flags.wid_present {
                    // "%[3]2d"
                    self.good_arg_num = false;
                }
            }

            // Do we have precision?
            if i + 1 < end && format[i] == b'.' {
                i += 1;
                if after_index {
                    // "%[3].2d"
                    self.good_arg_num = false;
                }
                (arg_num, i, after_index) = self.arg_number(arg_num, format, i, a.len());
                if i < end && format[i] == b'*' {
                    i += 1;
                    (self.fmt.prec, self.fmt.flags.prec_present, arg_num) =
                        int_from_arg(a, arg_num);
                    // Negative precision arguments don't make sense
                    if self.fmt.prec < 0 {
                        self.fmt.prec = 0;
                        self.fmt.flags.prec_present = false;
                    }
                    if !self.fmt.flags.prec_present {
                        self.buf.extend_from_slice(BAD_PREC_STRING);
                    }
                    after_index = false;
                } else {
                    (self.fmt.prec, self.fmt.flags.prec_present, i) = parsenum(format, i, end);
                    if !self.fmt.flags.prec_present {
                        self.fmt.prec = 0;
                        self.fmt.flags.prec_present = true;
                    }
                }
            }

            if !after_index {
                (arg_num, i, _) = self.arg_number(arg_num, format, i, a.len());
            }

            if i >= end {
                self.buf.extend_from_slice(NO_VERB_STRING);
                break;
            }

            let (verb, size) = decode_rune(&format[i..]);
            let verb = char::from_u32(verb).unwrap_or('\u{FFFD}');
            i += size;

            if verb == '%' {
                // Percent does not absorb operands and ignores f.wid and f.prec.
                self.buf.push(b'%');
            } else if !self.good_arg_num {
                self.bad_arg_num(verb);
            } else if arg_num >= a.len() {
                // No argument left over to print for the current verb.
                self.missing_arg(verb);
            } else {
                if verb == 'w' {
                    self.wrapped_errs.push(arg_num);
                }
                if verb == 'w' || verb == 'v' {
                    // Go syntax
                    self.fmt.flags.sharp_v = self.fmt.flags.sharp;
                    self.fmt.flags.sharp = false;
                    // Struct-field syntax
                    self.fmt.flags.plus_v = self.fmt.flags.plus;
                    self.fmt.flags.plus = false;
                }
                self.print_arg(&a[arg_num], verb);
                arg_num += 1;
            }
        }

        // Check for extra arguments unless the call accessed the arguments out of order, in which case it's
        // too expensive to detect if they've all been used and arguably OK if they're not.
        if !self.reordered && arg_num < a.len() {
            self.fmt.clearflags();
            self.buf.extend_from_slice(EXTRA_STRING);
            for (i, arg) in a[arg_num..].iter().enumerate() {
                if i > 0 {
                    self.buf.extend_from_slice(COMMA_SPACE_STRING);
                }
                if matches!(arg, GoValue::Nil | GoValue::Interface { elem: None, .. }) {
                    self.buf.extend_from_slice(NIL_ANGLE_STRING);
                } else {
                    self.buf.extend_from_slice(arg.type_name().as_bytes());
                    self.buf.push(b'=');
                    self.print_arg(arg, 'v');
                }
            }
            self.buf.push(b')');
        }
    }

    fn do_print(&mut self, a: &[GoValue<'_>]) {
        let mut prev_string = false;
        for (arg_num, arg) in a.iter().enumerate() {
            let is_string = matches!(arg.underlying(), GoValue::Str(..));
            // Add a space between two non-string arguments.
            if arg_num > 0 && !is_string && !prev_string {
                self.buf.push(b' ');
            }
            self.print_arg(arg, 'v');
            prev_string = is_string;
        }
    }

    /// doPrintln is like doPrint but always adds a space between arguments and a newline after the last
    /// argument.
    fn do_println(&mut self, a: &[GoValue<'_>]) {
        for (arg_num, arg) in a.iter().enumerate() {
            if arg_num > 0 {
                self.buf.push(b' ');
            }
            self.print_arg(arg, 'v');
        }
        self.buf.push(b'\n');
    }
}

fn is_predeclared_int(ty: &str) -> bool {
    matches!(ty, "int" | "int8" | "int16" | "int32" | "int64")
}

fn is_predeclared_uint(ty: &str) -> bool {
    matches!(ty, "uint" | "uint8" | "uint16" | "uint32" | "uint64" | "uintptr")
}

fn byte_values(elems: &[GoValue<'_>]) -> Vec<u8> {
    elems
        .iter()
        .map(|e| match e.underlying() {
            GoValue::Uint(u, _, _) => *u as u8,
            GoValue::Int(i, _) => *i as u8,
            _ => 0,
        })
        .collect()
}
