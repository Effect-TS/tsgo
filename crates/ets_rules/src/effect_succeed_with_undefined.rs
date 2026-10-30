use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, SourceFile};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectMajorVersion, TypeParser};

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_UNDEFINED_REPRESENTS_THE_SAME_OUTCOME_AS_EFFECT_SUCCEED_UNDEFINED_OR_EFFECT_SUCCEED_VOID_0_EFFECT_EFFECT_SUCCEED_WITH_UNDEFINED;

// go: internal/rules/effect_succeed_with_undefined.go:13 EffectSucceedWithUndefined a139d6ed82d2402e
// EffectSucceedWithUndefined suggests using Effect.undefined instead of Effect.succeed(undefined) or Effect.succeed(void 0).
pub static EFFECT_SUCCEED_WITH_UNDEFINED: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "effectSucceedWithUndefined",
    group: "style",
    description: "Suggests using Effect.undefined instead of Effect.succeed(undefined) or Effect.succeed(void 0)",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_effect_succeed_with_undefined(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/effect_succeed_with_undefined.go:33 AnalyzeEffectSucceedWithUndefined 2edc9233b2a40cb9
// AnalyzeEffectSucceedWithUndefined finds Effect.succeed(undefined) and Effect.succeed(void 0)
// steps whose success type is exactly undefined, which Effect.undefined replaces. Effect.undefined
// only exists in Effect v4.
//
// Rust: Go's checker argument is unused and dropped; the parser is never nil, so only the version check of Go's
// guard remains.
pub fn analyze_effect_succeed_with_undefined(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<EffectSucceedConstantMatch> {
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }
    filter_effect_succeed_constant_matches(
        analyze_effect_succeed_constant(tp, Some(sf)),
        "undefined",
    )
}
