use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_PROVIDE_ACCEPTS_AN_ARRAY_OF_LAYERS_AND_MERGES_THEM_ITSELF_PASS_THE_LAYERS_AS_AN_ARRAY_INSTEAD_OF_WRAPPING_THEM_IN_LAYER_MERGE_ALL_EFFECT_PROVIDE_MERGE_ALL_TO_ARRAY;

// go: internal/rules/provide_merge_all_to_array.go:15 ProvideMergeAllToArray 61eec093678dafd3
// ProvideMergeAllToArray suggests passing layers to Effect.provide as an array,
// since Effect.provide already merges an array of layers with Layer.mergeAll.
pub static PROVIDE_MERGE_ALL_TO_ARRAY: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "provideMergeAllToArray",
    group: "style",
    description: "Suggests passing an array of layers to Effect.provide instead of wrapping them in Layer.mergeAll",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_provide_merge_all_to_array(ctx.type_parser, Some(source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diagnostics
    },
});

// go: internal/rules/provide_merge_all_to_array.go:40 ProvideMergeAllToArrayMatch 6080eefd718b2700
// ProvideMergeAllToArrayMatch holds the nodes shared by the diagnostic and its quick fix.
#[derive(Clone, Copy)]
pub struct ProvideMergeAllToArrayMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange, // Error range of the Layer.mergeAll(...) call
    pub argument: P<Node>,   // The provide argument as written, possibly parenthesized
    pub merge_all: P<Node>,  // The Layer.mergeAll(...) call inside Argument
}

// go: internal/rules/provide_merge_all_to_array.go:49 AnalyzeProvideMergeAllToArray d41687e6a0ce09c8
// AnalyzeProvideMergeAllToArray finds Effect.provide calls whose layer argument
// is written directly as a Layer.mergeAll(...) call.
//
// Rust: the parser is never nil, so only the source file's nil check remains.
pub fn analyze_provide_merge_all_to_array(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<ProvideMergeAllToArrayMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        for (index, _) in flow.transformations().all() {
            let provide = flow.transformations().at(index);
            if provide.args().len() == 0 {
                continue;
            }
            let argument = *provide.args().at(0);
            let merge_all = ast::skip_parentheses(argument);
            if !is_plain_merge_all_call_shape(merge_all)
                || provide.type_arguments().is_some_and(|args| !args.nodes().is_empty())
                || !tp.is_node_reference_to_effect_module_api(provide.callee(), "provide")
                || !tp.is_node_reference_to_effect_layer_module_api(
                    merge_all.as_call_expression().expression(),
                    "mergeAll",
                )
            {
                continue;
            }

            matches.push(ProvideMergeAllToArrayMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, merge_all),
                argument,
                merge_all,
            });
        }
    }
    matches
}

// go: internal/rules/provide_merge_all_to_array.go:83 isPlainMergeAllCallShape 39f0c84f7b038e03
// isPlainMergeAllCallShape checks the syntax a quick fix can rewrite to an array:
// a non-optional call without explicit type arguments and with at least one argument.
//
// Rust: the node (a skipped-parentheses argument) and a call's argument list are never nil, so Go's nil checks drop.
pub(crate) fn is_plain_merge_all_call_shape(node: P<Node>) -> bool {
    if node.kind() != Kind::CallExpression {
        return false;
    }
    let call = node.as_call_expression();
    call.question_dot_token().is_none()
        && call.type_arguments().is_none_or(|args| args.nodes().is_empty())
        && !call.arguments().nodes().is_empty()
}
