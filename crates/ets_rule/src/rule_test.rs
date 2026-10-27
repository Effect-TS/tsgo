// Package rule defines the Rule struct for Effect diagnostic rules.

use ets_etscore::Severity;
use ets_shim::P;
use ets_shim::ast::Diagnostic;

use crate::*;

fn dummy_run(_: &mut Context<'_, '_>) -> Vec<P<Diagnostic>> {
    Vec::new()
}

/// Rust-only: Go's `Rule{Name: ..., Description: ..., Run: dummyRun}` (the other fields at their zero values).
fn rule(name: &'static str, description: &'static str) -> Rule {
    Rule {
        name,
        group: "",
        description,
        default_severity: Severity::Off,
        supported_effect: Vec::new(),
        codes: Vec::new(),
        run: dummy_run,
    }
}

// go test: internal/rule/rule_test.go:10 TestRuleStruct bc64def8037515d3
// Rust: Go's `r.Run == nil` check has no counterpart (`run` is a function pointer, never null).
#[test]
fn test_rule_struct() {
    // Test that a Rule can be created with all fields
    let r = rule("testRule", "A test rule");

    assert_eq!(r.name, "testRule", "expected Name to be 'testRule', got {:?}", r.name);
    assert_eq!(
        r.description, "A test rule",
        "expected Description to be 'A test rule', got {:?}",
        r.description
    );
}

// go test: internal/rule/rule_test.go:34 TestByName df89b770fece8375
#[test]
fn test_by_name() {
    let rules = vec![rule("rule1", "First rule"), rule("rule2", "Second rule")];

    // finds existing rule
    let found = by_name(&rules, "rule1").expect("expected to find rule1");
    assert_eq!(found.name, "rule1", "expected Name to be 'rule1', got {:?}", found.name);

    // returns nil for non-existent rule
    let found = by_name(&rules, "nonexistent");
    assert!(found.is_none(), "expected nil for non-existent rule, got {found:?}");

    // returns nil for empty slice
    let found = by_name(&[], "rule1");
    assert!(found.is_none(), "expected nil for empty slice, got {found:?}");

    // returns pointer to actual rule in slice
    let found = by_name(&rules, "rule1");
    assert!(
        found.is_some_and(|found| std::ptr::eq(found, &raw const rules[0])),
        "expected ByName to return pointer to actual rule in slice"
    );

    // finds second rule correctly
    let found = by_name(&rules, "rule2").expect("expected to find rule2");
    assert_eq!(found.name, "rule2", "expected Name to be 'rule2', got {:?}", found.name);
    assert!(
        std::ptr::eq(found, &raw const rules[1]),
        "expected ByName to return pointer to second rule in slice"
    );
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
