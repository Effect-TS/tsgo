// Package rules contains all Effect diagnostic rule implementations.

use std::fmt::Write as _;
use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_layergraph::{ExtractLayerGraphOptions, LayerOutlineGraphNodeInfo};
use ets_readonly::Slice;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Node, SourceFile};
use ets_shim::checker::{Checker, Type};
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{PipingFlow, TypeParser};
use rustc_hash::FxHashSet;

// go: internal/rules/missing_effect_context.go:23 MissingEffectContext b21dc0936153e35b
// MissingEffectContext detects when an Effect has context requirements that are not
// handled by the expected type. This happens when assigning an Effect with requirements
// to a variable/parameter expecting an Effect with fewer or no requirements.
pub static MISSING_EFFECT_CONTEXT: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "missingEffectContext",
    group: "correctness",
    description: "Detects Effect values with unhandled context requirements",
    default_severity: Severity::Error,
    supported_effect: vec!["v3", "v4"],
    codes: vec![tsdiag::THIS_EFFECT_REQUIRES_A_SERVICE_THAT_IS_MISSING_FROM_THE_EXPECTED_EFFECT_CONTEXT_COLON_0_EFFECT_MISSING_EFFECT_CONTEXT.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();

        let source_file = ctx.source_file;
        for re in ctx.checker().get_relation_errors(source_file) {
            // Parse both types as Effects
            let src_effect = ctx.type_parser.effect_type(re.source);
            let tgt_effect = ctx.type_parser.effect_type(re.target);

            // Both must be Effect types
            let (Some(src_effect), Some(tgt_effect)) = (src_effect, tgt_effect) else {
                continue;
            };

            // Find unhandled context types by checking each source requirement member
            // against the target requirement type
            let unhandled_contexts = find_unhandled_contexts(ctx.type_parser, src_effect.r(), tgt_effect.r());
            if !unhandled_contexts.is_empty() {
                let context_type_str = format_context_types(ctx.checker(), &unhandled_contexts);
                let related = missing_effect_context_related_information(ctx, Some(re.error_node), &unhandled_contexts);
                let diag = ctx.new_diagnostic(
                    ctx.source_file,
                    ctx.get_error_range(re.error_node),
                    &tsdiag::THIS_EFFECT_REQUIRES_A_SERVICE_THAT_IS_MISSING_FROM_THE_EXPECTED_EFFECT_CONTEXT_COLON_0_EFFECT_MISSING_EFFECT_CONTEXT,
                    &related,
                    &[&context_type_str],
                );
                diags.push(diag);
            }
        }

        diags
    },
}
});

// go: internal/rules/missing_effect_context.go:64 findUnhandledContexts 9cfaf360f42bcf04
// findUnhandledContexts returns the source context types that are not assignable to the target context type.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub(crate) fn find_unhandled_contexts(
    tp: &mut TypeParser<'_>,
    src_r: P<Type>,
    tgt_r: P<Type>,
) -> Vec<P<Type>> {
    // Unroll source context union into individual members
    let src_members = tp.unroll_union_members(src_r).unwrap_or_default();

    let mut unhandled = Vec::new();
    for member in src_members {
        // Check if this specific member is assignable to target
        if !tp.checker().is_type_assignable_to(member, tgt_r) {
            unhandled.push(member);
        }
    }
    unhandled
}

// go: internal/rules/missing_effect_context.go:79 formatContextTypes c94c46b19f91da2d
// formatContextTypes formats a slice of context types as a union string (e.g., "EnvA | EnvB").
pub(crate) fn format_context_types(c: &mut Checker, types: &[P<Type>]) -> String {
    if types.is_empty() {
        return String::new();
    }
    if types.len() == 1 {
        return c.type_to_string_exported(types[0]);
    }
    let mut result = String::new();
    result.push_str(&c.type_to_string_exported(types[0]));
    for &t in &types[1..] {
        result.push_str(" | ");
        result.push_str(&c.type_to_string_exported(t));
    }
    result
}

// go: internal/rules/missing_effect_context.go:95 missingEffectContextRelatedInformation b1d2a1eb6e4f37b4
//
// Rust: Go's checker argument is the context's checker (`ctx.checker()`).
pub(crate) fn missing_effect_context_related_information(
    ctx: &mut Context<'_, '_>,
    error_node: Option<P<Node>>,
    missing_types: &[P<Type>],
) -> Vec<P<Diagnostic>> {
    let provide_location =
        find_related_provide_location(ctx.type_parser, Some(ctx.source_file), error_node);
    let Some(layer_node) = provide_location.layer_node else {
        return Vec::new();
    };
    let layer_diagnostics =
        find_related_layer_provider_diagnostics(ctx, Some(layer_node), missing_types);
    if layer_diagnostics.is_empty() {
        return Vec::new();
    }
    let provide_diagnostic = ctx.new_diagnostic(
        ctx.source_file,
        provide_location.location,
        &tsdiag::ADJUSTING_THIS_LAYER_COMPOSITION_COULD_PROVIDE_THE_MISSING_SERVICE_EFFECT_MISSING_EFFECT_CONTEXT,
        &[],
        &[],
    );
    let mut related_information = Vec::with_capacity(1 + layer_diagnostics.len());
    related_information.push(provide_diagnostic);
    related_information.extend(layer_diagnostics);
    related_information
}

// go: internal/rules/missing_effect_context.go:116 provideLocation 5cbfa9b76d1acb6d
#[derive(Clone, Copy)]
pub(crate) struct ProvideLocation {
    pub(crate) layer_node: Option<P<Node>>,
    pub(crate) location: TextRange,
}

impl Default for ProvideLocation {
    /// Rust-only: Go's zero `provideLocation{}` (a nil node and the zero range).
    fn default() -> Self {
        ProvideLocation { layer_node: None, location: TextRange::new(0, 0) }
    }
}

// go: internal/rules/missing_effect_context.go:121 findRelatedProvideLocation 603e6b102a16f69f
//
// Rust: Go's checker argument is the parser's checker; Go's nil checker check is the type system's.
pub(crate) fn find_related_provide_location(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
    error_node: Option<P<Node>>,
) -> ProvideLocation {
    let (Some(sf), Some(error_node)) = (sf, error_node) else {
        return ProvideLocation::default();
    };
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        if !node_contains(Some(flow.node()), Some(error_node)) {
            continue;
        }
        let error_transformation_index =
            find_transformation_index_containing_node(Some(flow), Some(error_node));
        if error_transformation_index < 0 {
            continue;
        }
        let mut i = error_transformation_index - 1;
        while i >= 0 {
            let transformation = flow.transformations().at(i);
            if !tp.is_node_reference_to_effect_module_api(transformation.callee(), "provide") {
                i -= 1;
                continue;
            }
            return ProvideLocation {
                layer_node: first_node(transformation.args()),
                location: ets_shim::scanner::get_error_range_for_node(sf, transformation.callee()),
            };
        }
    }
    ProvideLocation::default()
}

// go: internal/rules/missing_effect_context.go:151 findRelatedLayerProviderDiagnostics cbd4a3d334ded383
//
// Rust: Go's checker argument is the context's checker (`ctx.checker()`); Go's nil checker and context checks are
// the type system's.
pub(crate) fn find_related_layer_provider_diagnostics(
    ctx: &mut Context<'_, '_>,
    layer_node: Option<P<Node>>,
    missing_types: &[P<Type>],
) -> Vec<P<Diagnostic>> {
    let Some(layer_node) = layer_node else {
        return Vec::new();
    };
    if missing_types.is_empty() {
        return Vec::new();
    }
    let root_layer_provides = root_layer_provides_types(ctx.type_parser, Some(layer_node));
    let full_graph = ets_layergraph::extract_layer_graph(
        ctx.type_parser,
        &[layer_node],
        Some(ctx.source_file),
        ExtractLayerGraphOptions { follow_symbols_depth: 2, ..Default::default() },
    );
    // Rust: Go's nil check on the outline graph drops (ExtractOutlineGraph never returns nil).
    let outline_graph = ets_layergraph::extract_outline_graph(ctx.type_parser, &full_graph);
    let mut related = Vec::new();
    let mut seen: FxHashSet<String> = FxHashSet::default();
    for &missing_type in missing_types {
        if root_provides_type(ctx.checker(), &root_layer_provides, missing_type) {
            continue;
        }
        let missing_type_text = ctx.checker().type_to_string_exported(missing_type);
        for (_, node_info) in outline_graph.nodes() {
            // Rust: `display_node` is never nil (Go falls back to `Node` for a nil one).
            let display_node = node_info.display_node;
            if !outline_node_provides_type(ctx.checker(), node_info, missing_type) {
                continue;
            }
            let mut key = missing_type_text.clone();
            let display_pos = go_pos(display_node);
            if display_pos >= 0 {
                let _ = write!(key, ":{display_pos}");
            }
            if !seen.insert(key) {
                continue;
            }
            let display_source_file = ast::get_source_file_of_node(display_node)
                .expect("a layer graph node is in a source file");
            related.push(ctx.new_diagnostic(
                display_source_file,
                ets_shim::scanner::get_error_range_for_node(display_source_file, display_node),
                &tsdiag::THIS_LAYER_PROVIDES_THE_MISSING_SERVICE_0_EFFECT_MISSING_EFFECT_CONTEXT,
                &[],
                &[&missing_type_text],
            ));
        }
    }
    related
}

// go: internal/rules/missing_effect_context.go:206 rootLayerProvidesTypes daa168f985a53e0f
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub(crate) fn root_layer_provides_types(
    tp: &mut TypeParser<'_>,
    layer_node: Option<P<Node>>,
) -> Vec<P<Type>> {
    let Some(layer_node) = layer_node else {
        return Vec::new();
    };
    let layer_node_type = tp.get_type_at_location(layer_node);
    let Some(layer_type) = tp.layer_type(layer_node_type) else {
        return Vec::new();
    };
    tp.unroll_union_members(layer_type.r_out()).unwrap_or_default()
}

// go: internal/rules/missing_effect_context.go:217 rootProvidesType b69946a55e7d6fc5
pub(crate) fn root_provides_type(
    c: &mut Checker,
    provided_types: &[P<Type>],
    target: P<Type>,
) -> bool {
    for &provided_type in provided_types {
        if provided_type == target {
            return true;
        }
        if c.is_type_assignable_to(provided_type, target)
            && c.is_type_assignable_to(target, provided_type)
        {
            return true;
        }
    }
    false
}

// go: internal/rules/missing_effect_context.go:230 outlineNodeProvidesType 7d8733f2b4b4a216
pub(crate) fn outline_node_provides_type(
    c: &mut Checker,
    node: &LayerOutlineGraphNodeInfo,
    target: P<Type>,
) -> bool {
    for &provided_type in &node.actual_provides {
        if provided_type == target {
            return true;
        }
        if c.is_type_assignable_to(provided_type, target)
            && c.is_type_assignable_to(target, provided_type)
        {
            return true;
        }
    }
    false
}

// go: internal/rules/missing_effect_context.go:243 firstNode 98b4f9ef625cdb21
pub(crate) fn first_node(nodes: Slice<'_, P<Node>>) -> Option<P<Node>> {
    if nodes.len() == 0 {
        return None;
    }
    Some(*nodes.at(0))
}

// go: internal/rules/missing_effect_context.go:250 findTransformationIndexContainingNode 0eca03cc036a6f09
pub(crate) fn find_transformation_index_containing_node(
    flow: Option<&PipingFlow>,
    target: Option<P<Node>>,
) -> i64 {
    let (Some(flow), Some(target)) = (flow, target) else {
        return -1;
    };
    let target = ast::skip_parentheses(target);
    for (i, _) in flow.transformations().all() {
        let transformation = flow.transformations().at(i);
        let callee = ast::skip_parentheses(transformation.callee());
        if callee == target || go_pos(callee) == go_pos(target) && go_end(target) >= go_end(callee)
        {
            return i;
        }
    }
    -1
}

// go: internal/rules/missing_effect_context.go:265 nodeContains 1b416adbd327da4c
pub(crate) fn node_contains(ancestor: Option<P<Node>>, target: Option<P<Node>>) -> bool {
    let (Some(ancestor), Some(target)) = (ancestor, target) else {
        return false;
    };
    go_pos(ancestor) <= go_pos(target) && go_end(target) <= go_end(ancestor)
}

/// Rust-only: Go's `node.Pos()` as an `int` (`-1` for a synthesized node).
pub(crate) fn go_pos(node: P<Node>) -> i64 {
    node.pos().source_offset().map_or(-1, i64::from)
}

/// Rust-only: Go's `node.End()` as an `int` (`-1` for a synthesized node).
pub(crate) fn go_end(node: P<Node>) -> i64 {
    node.end().source_offset().map_or(-1, i64::from)
}
