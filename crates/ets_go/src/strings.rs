//! The parts of Go's `strings` package whose behavior differs from Rust's `str` methods.
//!
//! For crates that cannot reach tsrs's `stringutil` (`ets_etscore` is a leaf crate: tsrs's `CompilerOptions`
//! holds its types). Crates that depend on `ets_shim` use `ets_shim::core::stringutil::{go_strings_to_lower, equal_fold}`, which
//! carry Go's own Unicode tables.

/// Go's `strings.ToLower`: rune by rune with Go's simple lowercase mapping (`unicode.ToLower`).
///
/// Rust's `char::to_lowercase` is the same mapping except for U+0130 (Latin capital I with dot), which Go maps to a
/// plain `i` and Rust to `i` plus a combining dot; `str::to_lowercase` also maps a word-final sigma contextually,
/// which Go never does.
pub fn to_lower(s: &str) -> String {
    s.chars()
        .map(|r| if r == '\u{130}' { 'i' } else { r.to_lowercase().next().unwrap_or(r) })
        .collect()
}

/// Go's `strings.TrimSpace`.
///
/// It trims `unicode.IsSpace` runes, which are Unicode's `White_Space` property, the same set `str::trim` uses.
pub fn trim_space(s: &str) -> &str {
    s.trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_lower_matches_go() {
        // Go: strings.ToLower("ÀBİKΣΣ") == "àbikσσ"
        assert_eq!(to_lower("ÀB\u{130}\u{212A}ΣΣ"), "àbikσσ");
        assert_eq!(to_lower("SKIP-FILE"), "skip-file");
    }

    #[test]
    fn trim_space_matches_go() {
        // Go: strings.TrimSpace("\u0085  \t x \v\f\r\n　") == "x"
        assert_eq!(trim_space("\u{85}\u{a0} \t x \u{b}\u{c}\r\n\u{3000}"), "x");
        assert_eq!(trim_space("\u{200b}x"), "\u{200b}x");
    }
}
