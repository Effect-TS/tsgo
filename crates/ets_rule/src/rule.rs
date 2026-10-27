// Package rule defines the Rule struct for Effect diagnostic rules.

use ets_etscore::Severity;
use ets_shim::P;
use ets_shim::ast::Diagnostic;

use crate::*;

// go: internal/rule/rule.go:12 Rule 7a0e1113baca354a
// Rule defines a diagnostic rule with its metadata and check function.
//
// Rust: rules are package-level variables (`LazyLock` statics, copied into `ets_rules::ALL` as Go's `All` copies
// them), so the strings are `'static` and `Run` is a function pointer (never nil). `Codes` is computed from the
// diagnostics messages at initialization, as in Go (`Message::code` is not `const`).
#[derive(Clone, Debug)]
pub struct Rule {
    // Name is the unique identifier used in configuration (e.g., "pocRule", "floatingEffect").
    // This name is used in tsconfig.json diagnosticSeverity config and in @effect-diagnostics directives.
    pub name: &'static str,

    // Group is the diagnostic group this rule belongs to (e.g., "correctness", "antipattern", "effectNative", "style").
    pub group: &'static str,

    // Description explains what the rule checks for (used in documentation/tooling).
    pub description: &'static str,

    // DefaultSeverity is the severity used when the user has not explicitly configured this rule.
    // Every rule must set this field explicitly.
    pub default_severity: Severity,

    // SupportedEffect lists which Effect versions this rule supports (e.g., "v3", "v4").
    pub supported_effect: Vec<&'static str>,

    // Codes is the list of diagnostic codes that this rule can emit.
    // This is used to build the code-to-rule mapping for codefixes and other lookups.
    pub codes: Vec<i32>,

    // Run executes the rule against a source file and returns diagnostics.
    // It should NOT attach diagnostics to the checker - just return them.
    // The hook will handle directive processing, severity transformation, and emission.
    pub run: fn(&mut Context<'_, '_>) -> Vec<P<Diagnostic>>,
}

// go: internal/rule/rule.go:41 UnusedDirectiveName bb208445b89b6bc1
// Configurable diagnostics that run outside the source-file rule registry.
pub const UNUSED_DIRECTIVE_NAME: &str = "unusedDirective";
// go: internal/rule/rule.go:41 UnusedDirectiveName bb208445b89b6bc1
pub const UNKNOWN_RULE_NAME_NAME: &str = "unknownRuleName";

// go: internal/rule/rule.go:47 ByName 956bd056970c15db
// ByName finds a rule by name in a slice. Returns nil if not found.
pub fn by_name<'a>(rules: &'a [Rule], name: &str) -> Option<&'a Rule> {
    rules.iter().find(|r| r.name == name)
}

// go: internal/rule/rule.go:57 AllCodes be7801259f23c9f3
// AllCodes returns all diagnostic codes from the given rules.
pub fn all_codes(rules: &[Rule]) -> Vec<i32> {
    let mut codes = Vec::new();
    for r in rules {
        codes.extend_from_slice(&r.codes);
    }
    codes
}

// go: internal/rule/rule.go:67 CodeToRuleName 15b52b1b564368ca
// CodeToRuleName returns the rule name for a diagnostic code.
// Returns empty string if the code is not found in any rule.
pub fn code_to_rule_name(rules: &[Rule], code: i32) -> &'static str {
    for r in rules {
        if r.codes.contains(&code) {
            return r.name;
        }
    }
    ""
}

// go: internal/rule/rule.go:77 IsEffectCode e705d9000ba76326
// IsEffectCode returns true if code is in the Effect diagnostic range (377000-377999).
pub fn is_effect_code(code: i32) -> bool {
    (377_000..=377_999).contains(&code)
}
