//! The parts of Go's `unicode/utf8` the other modules need, on bytes (Go strings may hold invalid UTF-8).

pub(crate) const RUNE_ERROR: u32 = 0xFFFD;
pub(crate) const RUNE_SELF: u32 = 0x80;
pub(crate) const MAX_RUNE: u32 = 0x0010_FFFF;

/// Go's `utf8.DecodeRune`: the first rune of `p` and its width; `(RuneError, 1)` for an invalid encoding,
/// `(RuneError, 0)` for an empty slice.
pub(crate) fn decode_rune(p: &[u8]) -> (u32, usize) {
    let Some(&b0) = p.first() else {
        return (RUNE_ERROR, 0);
    };
    if b0 < 0x80 {
        return (u32::from(b0), 1);
    }
    let (need, min, init) = match b0 {
        0xC2..=0xDF => (1, 0x80, u32::from(b0 & 0x1F)),
        0xE0..=0xEF => (2, 0x800, u32::from(b0 & 0x0F)),
        0xF0..=0xF4 => (3, 0x1_0000, u32::from(b0 & 0x07)),
        _ => return (RUNE_ERROR, 1),
    };
    if p.len() < need + 1 {
        // Go reports width 1 for any truncated or invalid sequence.
        return (RUNE_ERROR, 1);
    }
    let mut r = init;
    for &b in &p[1..=need] {
        if b & 0xC0 != 0x80 {
            return (RUNE_ERROR, 1);
        }
        r = (r << 6) | u32::from(b & 0x3F);
    }
    if r < min || r > MAX_RUNE || (0xD800..=0xDFFF).contains(&r) {
        return (RUNE_ERROR, 1);
    }
    (r, need + 1)
}

/// Go's `utf8.RuneCount`.
pub fn rune_count(p: &[u8]) -> usize {
    let mut n = 0;
    let mut i = 0;
    while i < p.len() {
        let (_, w) = decode_rune(&p[i..]);
        i += w;
        n += 1;
    }
    n
}

/// Go's `utf8.ValidRune`.
pub(crate) fn valid_rune(r: u32) -> bool {
    r <= MAX_RUNE && !(0xD800..=0xDFFF).contains(&r)
}

/// Go's `utf8.AppendRune` (invalid runes are written as U+FFFD).
pub(crate) fn append_rune(buf: &mut Vec<u8>, r: u32) {
    let c = char::from_u32(r).unwrap_or('\u{FFFD}');
    let mut tmp = [0; 4];
    buf.extend_from_slice(c.encode_utf8(&mut tmp).as_bytes());
}

/// Whether `bytes` is UTF-8 with, at most, WTF-8 lone surrogates.
///
/// A lone surrogate is `ED A0..BF 80..BF` (U+D800 to U+DFFF in the 3-byte form). Those are the only non-UTF-8
/// strings tsrs produces: like typescript-go
/// (`stringutil.EncodeJSStringRune`), its scanner keeps a lone surrogate of a JS string (`"\uD800"`) as those
/// three bytes, in a `&str`. A Go string holds them as they are; so does a Rust `String` here (tsrs's
/// convention, `tsrs_api_codec::is_wtf8`).
pub fn is_wtf8(bytes: &[u8]) -> bool {
    let mut rest = bytes;
    loop {
        match std::str::from_utf8(rest) {
            Ok(_) => return true,
            Err(e) => {
                let tail = &rest[e.valid_up_to()..];
                if tail.len() >= 3
                    && tail[0] == 0xED
                    && (0xA0..=0xBF).contains(&tail[1])
                    && (0x80..=0xBF).contains(&tail[2])
                {
                    rest = &tail[3..];
                } else {
                    return false;
                }
            }
        }
    }
}

/// A Go `string(bytes)` as a Rust `String`, when [`is_wtf8`] holds.
///
/// The bytes are kept unchanged, as Go keeps them. `None` for any other invalid UTF-8, which a Rust `String`
/// cannot hold (`docs/DIVERGENCES.md`, "Strings that are not UTF-8").
pub fn string_from_wtf8(bytes: &[u8]) -> Option<String> {
    if !is_wtf8(bytes) {
        return None;
    }
    #[expect(
        clippy::disallowed_methods,
        reason = "not for speed: WTF-8 lone surrogates are not UTF-8, so from_utf8 rejects the strings tsrs makes \
                  (and Go keeps); is_wtf8 checked the bytes above"
    )]
    // SAFETY: UTF-8 or WTF-8 lone surrogates, the tsrs convention for JS strings (`is_wtf8`); the bytes are
    // copied into the `String`.
    let text = unsafe { std::str::from_utf8_unchecked(bytes) };
    Some(text.to_owned())
}

/// A Go `string(bytes)` as a Rust `String`: [`string_from_wtf8`], else `String::from_utf8_lossy` (U+FFFD for
/// the sequences a Rust `String` cannot hold: `docs/DIVERGENCES.md`, "Strings that are not UTF-8").
pub fn string_from_go_bytes(bytes: &[u8]) -> String {
    string_from_wtf8(bytes).unwrap_or_else(|| String::from_utf8_lossy(bytes).into_owned())
}
