/// Rust-only: Go's `encoding/json` encoding of a string value (`encodeState.string` with HTML escaping on, as
/// `json.Marshal` uses): `"` and `\` escaped, `\b` `\f` `\n` `\r` `\t` short escapes, other control characters and
/// `<` `>` `&` as `\u00XX`, U+2028 and U+2029 as `\u2028` / `\u2029`. A Rust string is valid UTF-8, so Go's
/// U+FFFD replacement of invalid bytes never applies.
fn go_json_string(s: &str, out: &mut String) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if (c as u32) < 0x20 || c == '<' || c == '>' || c == '&' => {
                let b = c as u32 as u8;
                out.push_str("\\u00");
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0xF) as usize] as char);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Rust-only: Go's `base64.RawURLEncoding.EncodeToString` (URL alphabet, no padding).
fn base64_raw_url_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).copied().map_or(0, u32::from);
        let b2 = chunk.get(2).copied().map_or(0, u32::from);
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((n >> 6) & 63) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(n & 63) as usize] as char);
        }
    }
    out
}

// go: internal/layergraph/mermaidurl.go:13 EncodeMermaidURL 3c0b33efa6ad48b0
// EncodeMermaidURL encodes a Mermaid diagram string into a pako-compatible URL fragment.
// It JSON-serializes {"code": diagram}, compresses with zlib at max compression,
// base64url-encodes (no padding), and returns baseURL + "pako:" + encoded.
//
// Rust: Go's `compress/zlib` at `BestCompression` is `flate2`'s zlib encoder at `Compression::best()`; the two
// deflate implementations choose different (equally valid) encodings, so the encoded fragment differs from Go's
// while it decodes to the same payload (porting/DIVERGENCES.md). Go's JSON and zlib errors cannot happen (writes go to memory).
pub fn encode_mermaid_url(base_url: &str, diagram: &str) -> String {
    let mut json = String::from("{\"code\":");
    go_json_string(diagram, &mut json);
    json.push('}');

    let mut w = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    let _ = std::io::Write::write_all(&mut w, json.as_bytes());
    let compressed = w.finish().unwrap_or_default();

    let encoded = base64_raw_url_encode(&compressed);
    format!("{base_url}pako:{encoded}")
}
