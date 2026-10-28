use ets_etsoxlintrunner::{DiagnosticAdapter, ReportedDiagnostic};
use ets_shim::P;
use ets_shim::ast::Diagnostic as TsDiagnostic;
use ets_shim::core::TextRange;
use ets_shim::core::context::Context;
use tsrs_lint::protocol::{Diagnostic, Fix, LabeledRange, Range, RuleMessage, Suggestion};
use tsrs_lint::rule::{Listener, Rule, RuleContext};

use crate::names::EFFECT_RULE_NAMES;

/// Rust-only: Go's `newEffectRule(name, effectName)` in `effect_rules_generated.go`.
struct EffectRule {
    effect_name: &'static str,
}

/// Rust-only: the factory registered with `tsrs_lint::registry::register_external_rules`.
pub(crate) fn create_effect_rule(name: &str) -> Option<Box<dyn Rule>> {
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
