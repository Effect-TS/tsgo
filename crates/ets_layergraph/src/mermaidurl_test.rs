use ets_go::sprintf;
use ets_go::testing::T;

use crate::*;

/// Rust-only: Go's `base64.RawURLEncoding.DecodeString` (no padding; any other byte is an error).
fn base64_raw_url_decode(s: &str) -> Result<Vec<u8>, String> {
    fn value(b: u8) -> Result<u32, String> {
        match b {
            b'A'..=b'Z' => Ok(u32::from(b - b'A')),
            b'a'..=b'z' => Ok(u32::from(b - b'a') + 26),
            b'0'..=b'9' => Ok(u32::from(b - b'0') + 52),
            b'-' => Ok(62),
            b'_' => Ok(63),
            _ => Err(format!("illegal base64 data at input byte {b}")),
        }
    }
    let bytes = s.as_bytes();
    if bytes.len() % 4 == 1 {
        return Err("illegal base64 data".to_string());
    }
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, &b) in chunk.iter().enumerate() {
            n |= value(b)? << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

/// Rust-only: Go's `json.Unmarshal` into `struct { Code string `json:"code"` }`, for the one-field payload the
/// encoder writes (`{"code":"..."}` with JSON string escapes).
fn unmarshal_code(data: &[u8]) -> Result<String, String> {
    let text = std::str::from_utf8(data).map_err(|e| e.to_string())?;
    let rest = text.strip_prefix("{\"code\":\"").ok_or("unexpected payload")?;
    let rest = rest.strip_suffix("\"}").ok_or("unexpected payload")?;
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next().ok_or("truncated escape")? {
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            '/' => out.push('/'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'u' => {
                let hex: String = chars.by_ref().take(4).collect();
                let code = u32::from_str_radix(&hex, 16).map_err(|e| e.to_string())?;
                out.push(char::from_u32(code).ok_or("invalid escape")?);
            }
            other => return Err(format!("invalid escape {other}")),
        }
    }
    Ok(out)
}

/// Rust-only: the roundtrip check of the Go test (base64url decode, zlib decompress, JSON unmarshal).
fn decode_payload(t: &T, encoded: &str) -> String {
    let compressed = match base64_raw_url_decode(encoded) {
        Ok(compressed) => compressed,
        Err(err) => t.fatal(sprintf!("base64url decode failed: %v", err)),
    };
    let mut decompressed = Vec::new();
    let read = std::io::Read::read_to_end(
        &mut flate2::read::ZlibDecoder::new(compressed.as_slice()),
        &mut decompressed,
    );
    let decompressed = match read.map(|_| decompressed) {
        Ok(decompressed) => decompressed,
        Err(err) => t.fatal(sprintf!("zlib decompress failed: %v", format!("{err:?}"))),
    };
    match unmarshal_code(&decompressed) {
        Ok(code) => code,
        Err(err) => t.fatal(sprintf!("json unmarshal failed: %v", err)),
    }
}

// go test: internal/layergraph/mermaidurl_test.go:13 TestEncodeMermaidURL 989acec172f4131c
#[test]
fn test_encode_mermaid_url() {
    ets_go::testing::run(|t| {
        t.parallel();
        for (base_url, diagram) in [
            ("https://mermaid.live/edit#", "flowchart TB\n  A --> B"),
            ("https://www.mermaidchart.com/play#", "flowchart LR\n  X --> Y"),
        ] {
            t.run(base_url, |t| {
                t.parallel();
                let result = encode_mermaid_url(base_url, diagram);

                let prefix = format!("{base_url}pako:");
                let Some(encoded) = result.strip_prefix(&prefix) else {
                    t.fatal(sprintf!("expected prefix %q, got %q", prefix, result));
                };

                let code = decode_payload(t, encoded);
                if code != diagram {
                    t.error(sprintf!("roundtrip mismatch: got %q, want %q", code, diagram));
                }
            });
        }
    });
}
