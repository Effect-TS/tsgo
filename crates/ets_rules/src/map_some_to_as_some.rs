use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{PipingFlowTransformation, TypeParser};

// go: internal/rules/map_some_to_as_some.go:16 MapSomeToAsSome 02b9f6ec5769ec28
// MapSomeToAsSome suggests Effect.asSome when Effect.map only wraps the
// success value with Option.some.
pub static MAP_SOME_TO_AS_SOME: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "mapSomeToAsSome",
    group: "style",
    description: "Suggests using Effect.asSome instead of Effect.map when the mapper only wraps the success value with Option.some",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![
        tsdiag::EFFECT_AS_SOME_EXPRESSES_WRAPPING_THE_SUCCESS_VALUE_IN_OPTION_SOME_DIRECTLY_EFFECT_MAP_SOME_TO_AS_SOME.code(),
    ],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let matches = analyze_map_some_to_as_some(ctx.type_parser, ctx.source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                &tsdiag::EFFECT_AS_SOME_EXPRESSES_WRAPPING_THE_SUCCESS_VALUE_IN_OPTION_SOME_DIRECTLY_EFFECT_MAP_SOME_TO_AS_SOME,
                &[],
                &[],
            ));
        }
        diags
    },
}
});

// go: internal/rules/map_some_to_as_some.go:41 MapSomeToAsSomeMatch 68c16d0bc1c9e542
// MapSomeToAsSomeMatch holds the nodes needed by the diagnostic and quick fix.
#[derive(Clone)]
pub struct MapSomeToAsSomeMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub transformation: PipingFlowTransformation,
}

// go: internal/rules/map_some_to_as_some.go:49 AnalyzeMapSomeToAsSome 378ddb0d3664d8ba
// AnalyzeMapSomeToAsSome finds data-last and data-first Effect.map calls whose
// mapper is Option.some or an identity forwarder such as value => Option.some(value).
//
// Rust: Go's unused checker argument is dropped (the parser holds the checker).
pub fn analyze_map_some_to_as_some(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<MapSomeToAsSomeMatch> {
    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        for (index, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(index);
            if transformation.args().len() != 1
                || !tp.is_node_reference_to_effect_module_api(transformation.callee(), "map")
            {
                continue;
            }

            if transformation
                .type_arguments()
                .is_some_and(|type_arguments| !type_arguments.nodes().is_empty())
                || !is_option_some_mapper(tp, *transformation.args().at(0))
            {
                continue;
            }
            matches.push(MapSomeToAsSomeMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, transformation.callee()),
                transformation: transformation.clone(),
            });
        }
    }
    matches
}

// go: internal/rules/map_some_to_as_some.go:74 isOptionSomeMapper d4bcb2b70ac7dd5a
pub(crate) fn is_option_some_mapper(tp: &mut TypeParser<'_>, node: P<Node>) -> bool {
    let (mapper, type_arguments, parameter) = tp.unwrap_identity_forwarder(node);
    if type_arguments.is_some_and(|type_arguments| !type_arguments.nodes().is_empty()) {
        return false;
    }
    if let Some(parameter) = parameter {
        let parameter_declaration = parameter.as_parameter_declaration();
        if parameter_declaration.question_token().is_some()
            || parameter_declaration.type_().is_some()
        {
            return false;
        }
    }
    // Rust: Go passes a nil mapper through; the module-export check answers false for it.
    let Some(mapper) = mapper else {
        return false;
    };
    tp.is_node_reference_to_effect_option_module_api(mapper, "some")
}
