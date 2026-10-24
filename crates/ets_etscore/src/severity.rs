// Package etscore provides core types for Effect TypeScript integration.
// This is a leaf package with no dependencies, designed to be imported by
// both typescript-go/internal/core and effectapi without circular imports.

use std::borrow::Cow;

// go: etscore/severity.go:9 Severity
// Severity represents the configured severity level for a diagnostic rule.
//
// Rust: a newtype over Go's `int` (its `String` has a `default:` case for values outside the constants).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Severity(pub i64);

// go: etscore/severity.go:11 SeverityOff
impl Severity {
    pub const Off: Severity = Severity(0);
    pub const Warning: Severity = Severity(1);
    pub const Error: Severity = Severity(2);
    pub const Suggestion: Severity = Severity(3);
    pub const Message: Severity = Severity(4);
    pub const SkipFile: Severity = Severity(5); // Used in directives only, not in tsconfig
}

// go: etscore/severity.go:22 ParseSeverity
// ParseSeverity parses a severity string into a Severity constant.
// Valid values: "off", "warning", "warn", "error", "suggestion", "message", "skip-file"
pub fn parse_severity(s: &str) -> Severity {
    match ets_go::strings::to_lower(ets_go::strings::trim_space(s)).as_str() {
        "off" => Severity::Off,
        "warning" | "warn" => Severity::Warning,
        "error" => Severity::Error,
        "suggestion" => Severity::Suggestion,
        "message" => Severity::Message,
        "skip-file" => Severity::SkipFile,
        _ => Severity::Error, // default to error for unknown values
    }
}

impl Severity {
    // go: etscore/severity.go:42 Severity.String
    // String returns the string representation of a Severity.
    pub fn string(self) -> &'static str {
        match self {
            Severity::Off => "off",
            Severity::Warning => "warning",
            Severity::Error => "error",
            Severity::Suggestion => "suggestion",
            Severity::Message => "message",
            Severity::SkipFile => "skip-file",
            _ => "unknown",
        }
    }

    // go: etscore/severity.go:62 Severity.IsOff
    // IsOff returns true if the severity disables the diagnostic.
    pub fn is_off(self) -> bool {
        self == Severity::Off || self == Severity::SkipFile
    }

    // go: etscore/severity.go:70 Severity.visibilityRank
    // visibilityRank orders severities by how prominently they surface in output:
    // error > warning > suggestion > message > off/skip-file. This is distinct
    // from the declaration order of the constants, which is not a visibility
    // ordering.
    pub(crate) fn visibility_rank(self) -> i64 {
        match self {
            Severity::Error => 4,
            Severity::Warning => 3,
            Severity::Suggestion => 2,
            Severity::Message => 1,
            _ => 0,
        }
    }

    // go: etscore/severity.go:88 Severity.AtLeastAsVisibleAs
    // AtLeastAsVisibleAs reports whether s surfaces at least as prominently as
    // min. Off and skip-file severities are never at least as visible as any
    // enabled severity.
    pub fn at_least_as_visible_as(self, min: Severity) -> bool {
        self.visibility_rank() >= min.visibility_rank()
    }

    // go: etscore/severity.go:94 Severity.MarshalJSON
    // MarshalJSON implements json.Marshaler for Severity.
    // Serializes as the string representation (e.g., "error", "warning").
    #[expect(
        clippy::unnecessary_wraps,
        reason = "Go's json.Marshaler signature: the error is always nil"
    )]
    pub fn marshal_json(self) -> Result<Vec<u8>, ets_go::Error> {
        Ok(format!("\"{}\"", self.string()).into_bytes())
    }

    // go: etscore/severity.go:100 Severity.UnmarshalJSON
    // UnmarshalJSON implements json.Unmarshaler for Severity.
    // Parses from string representation (e.g., "error", "warning").
    #[expect(
        clippy::unnecessary_wraps,
        reason = "Go's json.Unmarshaler signature: the error is always nil"
    )]
    pub fn unmarshal_json(&mut self, data: &[u8]) -> Result<(), ets_go::Error> {
        // Remove quotes from JSON string
        let str = String::from_utf8_lossy(data);
        let str = str.trim_matches('"');
        *self = parse_severity(str);
        Ok(())
    }
}

/// Rust-only: Go's `fmt` calls `String()` for `%v` / `%s`.
impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.string())
    }
}

/// Rust-only: how Go's `fmt` sees a `Severity` (a Stringer over an `int`).
impl ets_go::fmt::Value for Severity {
    fn go_value(&self) -> ets_go::fmt::GoValue<'_> {
        ets_go::fmt::GoValue::stringer(
            self.string().to_string(),
            ets_go::fmt::GoValue::Int(self.0, Cow::Borrowed("etscore.Severity")),
        )
    }
}
