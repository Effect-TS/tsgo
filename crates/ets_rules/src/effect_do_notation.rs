use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_USES_THE_EFFECT_DO_EMULATION_EFFECT_GEN_OR_EFFECT_FN_ACHIEVE_THE_SAME_RESULT_WITH_NATIVE_JS_SCOPES_EFFECT_EFFECT_DO_NOTATION;

// go: internal/rules/effect_do_notation.go:15 EffectDoNotation 1c070437d9b9f6df
// EffectDoNotation suggests using Effect.gen or Effect.fn instead of Effect.Do.
pub static EFFECT_DO_NOTATION: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "effectDoNotation",
    group: "style",
    description: "Suggests using Effect.gen or Effect.fn instead of the Effect.Do notation helpers",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_effect_do_notation(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/effect_do_notation.go:39 EffectDoNotationMatch ab789313a7ac3d81
#[derive(Clone)]
pub struct EffectDoNotationMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub node: P<Node>,
}

// go: internal/rules/effect_do_notation.go:46 AnalyzeEffectDoNotation e5e346d34497354e
// AnalyzeEffectDoNotation finds all references to Effect.Do-style helpers.
//
// Rust: Go's checker argument is unused and dropped.
pub fn analyze_effect_do_notation(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<EffectDoNotationMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        n: P<Node>,
        matches: &mut Vec<EffectDoNotationMatch>,
    ) -> bool {
        if tp.is_node_reference_to_effect_module_api(n, "Do") {
            matches.push(EffectDoNotationMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, n),
                node: n,
            });
        } else {
            n.for_each_child(&mut |child| walk(tp, sf, child, matches));
        }

        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}
