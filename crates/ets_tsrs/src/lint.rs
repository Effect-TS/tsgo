//! Rust-only: the Effect rules of tsrs's type-aware linter (`tsrs --lint`), the counterpart of the patched tsgolint
//! (`_patches/tsgolint/001-effect-rules.patch` and the `effect_rules_generated.go` that `repoctl codegen tsgolint`
//! writes).
//!
//! Go appends one `rule.Rule` per Effect rule, named `effecttsgo/<kebab-case name>`, to tsgolint's `allRules`. Each
//! runs `etsoxlintrunner.RunRuleAndReport` once per file and listens to no node. Here the same adapters are built on
//! demand by the factory that `register` hands to tsrs's linter (`tsrs_lint::registry::register_external_rules`,
//! added by `_patches/tsrs/170-lint-effect-rules.patch`).

use std::sync::LazyLock;

use rustc_hash::FxHashMap;

/// Rust-only: `toKebabCase` of `_tools/repoctl/src/codegen.ts`, which names the tsgolint Effect rules.
fn to_kebab_case(value: &str) -> String {
    let characters: Vec<char> = value.chars().collect();
    let mut result = String::new();
    for (index, &current) in characters.iter().enumerate() {
        if current.is_ascii_uppercase() {
            let previous_is_lower = index > 0 && characters[index - 1].is_ascii_lowercase();
            let next_is_lower = characters.get(index + 1).is_some_and(char::is_ascii_lowercase);
            if index > 0 && (previous_is_lower || next_is_lower) {
                result.push('-');
            }
            result.push(current.to_ascii_lowercase());
        } else {
            result.push(current);
        }
    }
    result
}

/// Rust-only: the `newEffectRule(qualifiedName, effectName)` registrations of `renderTsgolintEffectRules`.
#[cfg_attr(
    not(any(feature = "cli", test)),
    expect(dead_code, reason = "only the linter looks rules up")
)]
static EFFECT_RULE_NAMES: LazyLock<FxHashMap<String, &'static str>> = LazyLock::new(|| {
    ets_rules::ALL
        .iter()
        .map(|rule| (format!("effecttsgo/{}", to_kebab_case(rule.name)), rule.name))
        .collect()
});

/// Rust-only: tsgolint's `append(allRules, effectRules...)`, plus the `init()` of `etsoxlintrunner` that the generated
/// rules import.
#[cfg(feature = "cli")]
pub(crate) fn register() {
    ets_etsoxlintrunner::init();
    tsrs_lint::registry::register_external_rules(adapter::create_effect_rule);
}

#[cfg(feature = "cli")]
mod adapter {
    use ets_etsoxlintrunner::{DiagnosticAdapter, ReportedDiagnostic};
    use ets_shim::P;
    use ets_shim::ast::Diagnostic as TsDiagnostic;
    use ets_shim::core::TextRange;
    use ets_shim::core::context::Context;
    use tsrs_lint::protocol::{Diagnostic, Fix, LabeledRange, Range, RuleMessage, Suggestion};
    use tsrs_lint::rule::{Listener, Rule, RuleContext};

    use super::EFFECT_RULE_NAMES;

    /// Rust-only: Go's `newEffectRule(name, effectName)` in `effect_rules_generated.go`.
    struct EffectRule {
        effect_name: &'static str,
    }

    /// Rust-only: the factory registered with `tsrs_lint::registry::register_external_rules`.
    pub(super) fn create_effect_rule(name: &str) -> Option<Box<dyn Rule>> {
        let effect_name = *EFFECT_RULE_NAMES.get(name)?;
        Some(Box::new(EffectRule { effect_name }))
    }

    fn lint_range(range: TextRange) -> Range {
        Range {
            pos: range.source_pos().unwrap_or_default(),
            end: range.source_end().unwrap_or_default(),
        }
    }

    impl Rule for EffectRule {
        /// Go's `Run`: the rule runs once per file and returns no listeners.
        fn initialize(&mut self, context: &mut RuleContext<'_>) {
            let program = context.program;
            let source_file = context.source_file;
            let fix_suggestions = context.options.fix_suggestions;
            let rule_name = context.rule_name.to_string();
            let options = program.options().effect.get();
            let mut reported = Vec::new();
            // Go's `utils.GetDiagnosticMessage`: the localized message text.
            let message = |diagnostic: P<TsDiagnostic>| diagnostic.localize();
            let mut report = |diagnostic: ReportedDiagnostic<'_>| {
                let labeled_ranges = diagnostic
                    .related_information
                    .iter()
                    .filter(|related| related.file_name == source_file.file_name())
                    .map(|related| LabeledRange {
                        label: related.description.clone(),
                        range: lint_range(related.range),
                    })
                    .collect();
                // tsgolint's ReportDiagnosticWithSuggestions computes suggestions only when they are requested.
                let suggestions = if fix_suggestions {
                    (diagnostic.suggestions)()
                        .into_iter()
                        .map(|suggestion| Suggestion {
                            message: RuleMessage {
                                description: suggestion.description,
                                ..Default::default()
                            },
                            fixes: suggestion
                                .edits
                                .into_iter()
                                .map(|edit| Fix { text: edit.text, range: lint_range(edit.range) })
                                .collect(),
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                reported.push(Diagnostic {
                    range: Some(lint_range(diagnostic.range)),
                    message: RuleMessage {
                        id: diagnostic.message_id,
                        description: diagnostic.description,
                        ..Default::default()
                    },
                    labeled_ranges,
                    suggestions,
                    ..Default::default()
                });
            };
            if let Err(err) = ets_etsoxlintrunner::run_rule_and_report(
                &Context::background(),
                Some(program),
                Some(&mut *context.checker),
                Some(source_file),
                options.as_deref(),
                self.effect_name,
                DiagnosticAdapter { message: Some(&message), report: Some(&mut report) },
            ) {
                panic!("running {rule_name}: {err}");
            }
            for diagnostic in reported {
                context.report_diagnostic(diagnostic);
            }
        }

        fn listens_to(&self, _listener: Listener) -> bool {
            false
        }

        fn visit(
            &mut self,
            _listener: Listener,
            _node: P<ets_shim::ast::Node>,
            _context: &mut RuleContext<'_>,
        ) {
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kebab_case_matches_codegen() {
        for (input, expected) in [
            ("floatingEffect", "floating-effect"),
            ("missingEffectContext", "missing-effect-context"),
            ("globalJSON", "global-json"),
            ("globalJsonInEffect", "global-json-in-effect"),
            ("anyUnknownInErrorContext", "any-unknown-in-error-context"),
        ] {
            assert_eq!(to_kebab_case(input), expected, "{input}");
        }
    }

    #[test]
    fn every_effect_rule_has_a_distinct_name() {
        assert_eq!(EFFECT_RULE_NAMES.len(), ets_rules::ALL.len());
        assert_eq!(EFFECT_RULE_NAMES.get("effecttsgo/floating-effect"), Some(&"floatingEffect"));
    }
}
