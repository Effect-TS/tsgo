use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{EffectMajorVersion, TypeParser};

// go: internal/rules/multiple_effect_provide.go:17 MultipleEffectProvide
// MultipleEffectProvide detects consecutive Effect.provide calls with Layer-typed
// arguments within piping flows and warns that they should be merged into a single
// provide call to avoid service lifecycle issues.
pub static MULTIPLE_EFFECT_PROVIDE: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
        name: "multipleEffectProvide",
        group: "antipattern",
        description: "Warns against chaining Effect.provide calls which can cause service lifecycle issues",
        default_severity: Severity::Warning,
        supported_effect: vec!["v3", "v4"],
        codes: vec![tsdiag::THIS_EXPRESSION_CHAINS_MULTIPLE_EFFECT_PROVIDE_CALLS_PROVIDING_LAYERS_IN_MULTIPLE_CALLS_IN_A_CHAIN_CAN_BREAK_SERVICE_LIFECYCLE_BEHAVIOR_COMPARED_WITH_A_SINGLE_COMBINED_PROVIDE_WITH_MERGED_LAYERS_EFFECT_MULTIPLE_EFFECT_PROVIDE.code()],
        run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
            let matches = analyze_multiple_effect_provide(ctx.type_parser, ctx.source_file);
            let mut diags = Vec::with_capacity(matches.len());
            for m in &matches {
                diags.push(ctx.new_diagnostic(m.source_file, m.location, &tsdiag::THIS_EXPRESSION_CHAINS_MULTIPLE_EFFECT_PROVIDE_CALLS_PROVIDING_LAYERS_IN_MULTIPLE_CALLS_IN_A_CHAIN_CAN_BREAK_SERVICE_LIFECYCLE_BEHAVIOR_COMPARED_WITH_A_SINGLE_COMBINED_PROVIDE_WITH_MERGED_LAYERS_EFFECT_MULTIPLE_EFFECT_PROVIDE, &[], &[]));
            }
            diags
        },
    }
});

// go: internal/rules/multiple_effect_provide.go:36 MultipleEffectProvideMatch
// MultipleEffectProvideMatch holds the AST nodes needed by both the
// diagnostic rule and the quick-fix for the multipleEffectProvide pattern.
#[derive(Clone)]
pub struct MultipleEffectProvideMatch {
    pub source_file: P<SourceFile>, // The source file where the diagnostic should be reported
    pub location: TextRange, // The diagnostic span (error range on the first call in the chunk)
    pub chunk: Vec<P<Node>>, // The list of call expression nodes in the consecutive provide chain (2+ elements)
    pub layer_args: Vec<P<Node>>, // The layer argument nodes from each Effect.provide(layer) call
}

// go: internal/rules/multiple_effect_provide.go:45 AnalyzeMultipleEffectProvide
// AnalyzeMultipleEffectProvide finds all consecutive Effect.provide call chains
// with Layer-typed arguments within piping flows.
//
// Rust: Go's (ignored) checker argument is dropped.
pub fn analyze_multiple_effect_provide(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<MultipleEffectProvideMatch> {
    let mut matches = Vec::new();
    let is_effect_v4 = tp.supported_effect_version() == EffectMajorVersion::V4;

    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        // Track chunks of consecutive Effect.provide calls with Layer arguments.
        let mut current_chunk: Vec<P<Node>> = Vec::new();
        let mut current_layer_args: Vec<P<Node>> = Vec::new();

        // Rust: Go's `finalizeChunk` closure over the loop's state.
        fn finalize_chunk(
            sf: P<SourceFile>,
            matches: &mut Vec<MultipleEffectProvideMatch>,
            current_chunk: &mut Vec<P<Node>>,
            current_layer_args: &mut Vec<P<Node>>,
        ) {
            if current_chunk.len() >= 2 {
                matches.push(MultipleEffectProvideMatch {
                    source_file: sf,
                    location: ets_shim::scanner::get_error_range_for_node(sf, current_chunk[0]),
                    chunk: std::mem::take(current_chunk),
                    layer_args: std::mem::take(current_layer_args),
                });
            }
            current_chunk.clear();
            current_layer_args.clear();
        }

        for (_, transformation) in flow.transformations().all() {
            macro_rules! finalize {
                () => {
                    finalize_chunk(sf, &mut matches, &mut current_chunk, &mut current_layer_args)
                };
            }

            // Must have arguments
            if transformation.args().len() == 0 {
                finalize!();
                continue;
            }

            // Check if this is an Effect.provide call
            if !tp.is_node_reference_to_effect_module_api(transformation.callee(), "provide") {
                finalize!();
                continue;
            }

            let mut is_local_provide = false;
            if is_effect_v4 && transformation.args().len() > 1 {
                let options = *transformation.args().at(1);
                if options.kind() == Kind::ObjectLiteralExpression {
                    for &property in options.as_object_literal_expression().properties.nodes() {
                        if property.kind() != Kind::PropertyAssignment {
                            continue;
                        }
                        let name = property.name();
                        if let Some(name) = name
                            && name.kind() == Kind::Identifier
                            && name.text() == "local"
                            && property.initializer().unwrap().kind() == Kind::TrueKeyword
                        {
                            is_local_provide = true;
                            break;
                        }
                    }
                }
            }
            if is_local_provide {
                finalize!();
                continue;
            }

            // Check if the first argument is a Layer type
            let arg = *transformation.args().at(0);
            if tp.node_cannot_be_effect_library_type(arg) {
                finalize!();
                continue;
            }
            let Some(arg_type) = tp.get_type_at_location(arg) else {
                finalize!();
                continue;
            };

            if tp.layer_type(arg_type).is_none() {
                // provide call but not with a Layer argument — breaks the chain
                finalize!();
                continue;
            }

            // Find the enclosing call expression for this provide
            let Some(call_node) =
                ast::find_ancestor_kind(transformation.callee(), Kind::CallExpression)
            else {
                finalize!();
                continue;
            };

            current_chunk.push(call_node);
            current_layer_args.push(arg);
        }

        // Finalize the last chunk after the loop
        finalize_chunk(sf, &mut matches, &mut current_chunk, &mut current_layer_args);
    }

    matches
}
