use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{PipingFlow, PipingFlowTransformation, TransformationKind, TypeParser};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_MAP_EFFECT_FLATTEN_IS_THE_SAME_AS_EFFECT_FLAT_MAP_THAT_EXPRESSES_THE_SAME_STEPS_MORE_DIRECTLY_EFFECT_EFFECT_MAP_FLATTEN;

// go: internal/rules/effect_map_flatten.go:16 EffectMapFlatten 6f29b4722d813982
// EffectMapFlatten suggests using Effect.flatMap instead of Effect.map followed
// by Effect.flatten in piping flows.
pub static EFFECT_MAP_FLATTEN: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "effectMapFlatten",
    group: "style",
    description: "Suggests using Effect.flatMap instead of Effect.map followed by Effect.flatten in piping flows",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_effect_map_flatten(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/effect_map_flatten.go:40 EffectMapFlattenMatch 2b343d0f9910a900
#[derive(Clone)]
pub struct EffectMapFlattenMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub node: P<Node>,
}

// go: internal/rules/effect_map_flatten.go:48 AnalyzeEffectMapFlatten 820b305db4c04c92
// AnalyzeEffectMapFlatten finds adjacent Effect.map(...), Effect.flatten pairs
// in pipe/pipeable flows.
//
// Rust: Go's checker argument is unused and dropped.
pub fn analyze_effect_map_flatten(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<EffectMapFlattenMatch> {
    let mut matches = Vec::new();

    let flows = tp.piping_flows(sf, false);
    for (_, flow) in ets_readonly::new(flows.as_deref()).all() {
        let sequences = PipingFlow::find_transformation_sequences(
            Some(flow),
            tp,
            &[
                &|tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| {
                    transformation.args().len() > 0
                        && tp.is_node_reference_to_effect_module_api(transformation.callee(), "map")
                },
                &|tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| {
                    transformation.args().len() == 0
                        && tp.is_node_reference_to_effect_module_api(
                            transformation.callee(),
                            "flatten",
                        )
                },
            ],
        );
        for (_, sequence) in ets_readonly::new(sequences.as_deref()).all() {
            let map_transformation = flow.transformations().at(sequence.start());
            let flatten_transformation = flow.transformations().at(sequence.start() + 1);
            if (map_transformation.kind() != TransformationKind::Pipe
                && map_transformation.kind() != TransformationKind::Pipeable)
                || flatten_transformation.kind() != map_transformation.kind()
            {
                continue;
            }

            matches.push(EffectMapFlattenMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(
                    sf,
                    flatten_transformation.callee(),
                ),
                node: flatten_transformation.callee(),
            });
        }
    }

    matches
}
