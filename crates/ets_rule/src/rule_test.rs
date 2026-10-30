// Package rule defines the Rule struct for Effect diagnostic rules.

use ets_etscore::Severity;
use ets_shim::P;
use ets_shim::ast::Diagnostic;

use crate::*;

fn dummy_run(_: &mut Context<'_, '_>) -> Vec<P<Diagnostic>> {
    Vec::new()
}

/// Rust-only: Go's `Rule{Name: ...}` (the other fields at their zero values).
fn rule(name: &'static str) -> Rule {
    Rule {
        name,
        group: "",
        description: "",
        default_severity: Severity::Off,
        supported_effect: Vec::new(),
        codes: Vec::new(),
        replaced_by: vec![],
        run: dummy_run,
    }
}

// go test: internal/rule/rule_test.go:5 TestByName 03e8f85c8a8b1ec7
#[test]
fn test_by_name() {
    let rules = vec![rule("rule1"), rule("rule2")];

    let found = by_name(&rules, "rule2");
    assert!(
        found.is_some_and(|found| std::ptr::eq(found, &raw const rules[1])),
        "expected pointer to rules[1], got {found:?}"
    );
    let found = by_name(&rules, "nonexistent");
    assert!(found.is_none(), "expected nil for non-existent rule, got {found:?}");
    let found = by_name(&[], "rule1");
    assert!(found.is_none(), "expected nil for empty slice, got {found:?}");
}

/// Rust-only: regression test for `_patches/tsrs/008-diagnostics-generate.patch`: the Effect messages are tsrs
/// diagnostics messages, with Go's identifiers, codes, categories and keys, and the key lookup finds them.
#[test]
fn effect_diagnostic_messages_are_generated() {
    let message = &ets_shim::diagnostics::X_EFFECT_DIAGNOSTICS_DIRECTIVE_HAS_NO_EFFECT;
    assert_eq!(message.code(), 377_000);
    assert_eq!(message.category(), ets_shim::diagnostics::Category::Warning);
    assert_eq!(message.key().as_str(), "effect_diagnostics_directive_has_no_effect_377000");
    assert_eq!(message.text(), "@effect-diagnostics directive has no effect.");
    let found = ets_shim::diagnostics::key_to_message(message.key().as_str())
        .expect("the key lookup knows the Effect messages");
    assert!(std::ptr::eq(found, message));
    assert!(is_effect_code(message.code()));
}
