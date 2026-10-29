use std::sync::LazyLock;

use ets_layergraph::{ExtractLayerGraphOptions, LayerMagicResult};
use ets_refactor::{Context, Refactor, RefactorAction};
use ets_rewriter::Tracker;
use ets_shim::ast::{self, Kind, Node, NodeFlags, SourceFile};
use ets_shim::astnav;
use ets_shim::checker::{Checker, Type, TypeFormatFlags};
use ets_shim::core::{P, alloc_str};
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;
use ets_typeparser::ModuleReference;

use crate::*;

// go: internal/refactors/layer_magic.go:17 LayerMagic 41262d15bc94ab49
pub static LAYER_MAGIC: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "layerMagic",
    description: "Layer Magic",
    kind: "rewrite.effect.layerMagic",
    run: run_layer_magic,
});

// go: internal/refactors/layer_magic.go:24 runLayerMagic 3b6c9a048685ab2d
// Rust: tsrs's `get_token_at_position` never returns nil, so Go's nil check drops. Go passes the context's
// type parser and checker next to the context; the Rust callees reach both through it.
pub(crate) fn run_layer_magic(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let token = astnav::get_token_at_position(ctx.source_file, span_pos(ctx));

    let layer_identifier = ets_typeparser::find_module_reference(ctx.source_file, "Layer");

    // Collect ancestor nodes from the token up to the source file.
    let mut ancestors = Vec::new();
    let mut node = Some(token);
    while let Some(n) = node.filter(|n| n.kind() != Kind::SourceFile) {
        ancestors.push(n);
        node = n.parent();
    }

    // Try build first on all ancestors, then prepare as fallback.
    for &node in &ancestors {
        let action = try_build_refactor(ctx, node, &layer_identifier);
        if !action.is_empty() {
            return action;
        }
    }
    for &node in &ancestors {
        let action = try_prepare_refactor(ctx, node, &layer_identifier);
        if !action.is_empty() {
            return action;
        }
    }

    Vec::new()
}

// go: internal/refactors/layer_magic.go:59 adjustedNode 7c5fb205a0766931
// adjustedNode redirects from a variable/property declaration name to the initializer.
pub(crate) fn adjusted_node(node: P<Node>) -> P<Node> {
    if let Some(parent) = node.parent()
        && node.kind() == Kind::Identifier
    {
        if parent.kind() == Kind::VariableDeclaration {
            let vd = parent.as_variable_declaration();
            if let Some(initializer) = vd.initializer()
                && vd.name() == node
            {
                return initializer;
            }
        }
        if parent.kind() == Kind::PropertyDeclaration {
            let pd = parent.as_property_declaration();
            if let Some(initializer) = pd.initializer()
                && pd.name() == node
            {
                return initializer;
            }
        }
    }
    node
}

// go: internal/refactors/layer_magic.go:80 tryBuildRefactor fbe01b7e6d6b17d4
// tryBuildRefactor tries to produce the "build" refactor action for the given node.
// It detects the `(expr as any) as Layer.Layer<ROut>` pattern.
//
// Rust: Go's `tp` and `c` parameters are the context's (`ctx.type_parser`, `ctx.checker()`).
// `ExtractLayerGraph`, `ExtractOutlineGraph` and `ConvertOutlineGraphToLayerMagic` return their results by value
// (never nil). The missing output types are printed here, before the action runs (Go prints them in the action's
// closure, which cannot borrow the checker in Rust while the context builds the action); printing does not depend
// on the tracker, so the text is the same.
pub(crate) fn try_build_refactor(
    ctx: &mut Context<'_, '_>,
    node: P<Node>,
    layer_identifier: &ModuleReference,
) -> Vec<CodeAction> {
    let at_location = adjusted_node(node);

    // Must be an AsExpression: `... as Layer.Layer<ROut>`
    if at_location.kind() != Kind::AsExpression {
        return Vec::new();
    }
    let outer_as = at_location.as_as_expression();
    let outer_type = outer_as.type_();
    if outer_type.kind() != Kind::TypeReference {
        return Vec::new();
    }

    // Inner expression must be `expr as any`
    let inner_expr = outer_as.expression();
    if inner_expr.kind() != Kind::AsExpression {
        return Vec::new();
    }
    let inner_as = inner_expr.as_as_expression();
    if inner_as.type_().kind() != Kind::AnyKeyword {
        return Vec::new();
    }

    // Parse the outer type as a Layer type to get ROut
    let Some(layer) = ctx.type_parser.layer_type_at_location(outer_type) else {
        return Vec::new();
    };

    // The inner expression (before `as any`) is the casted structure
    let casted_structure = inner_as.expression();

    // Extract layer graph from the casted structure
    let layer_graph = ets_layergraph::extract_layer_graph(
        &mut ctx.type_parser,
        &[casted_structure],
        Some(ctx.source_file),
        ExtractLayerGraphOptions {
            array_literal_as_merge: true,
            explode_only_layer_calls: true,
            follow_symbols_depth: 0,
            ..Default::default()
        },
    );

    // Extract outline graph
    let outline_graph = ets_layergraph::extract_outline_graph(&mut ctx.type_parser, &layer_graph);
    if outline_graph.node_count() <= 1 {
        return Vec::new();
    }

    // Get target output types from the Layer ROut
    let target_output_types =
        ctx.type_parser.unroll_union_members(layer.r_out()).unwrap_or_default();

    // Convert to magic result
    let magic_result = ets_layergraph::convert_outline_graph_to_layer_magic(
        &ctx.type_parser,
        &outline_graph,
        &target_output_types,
    );
    if magic_result.nodes.is_empty() {
        return Vec::new();
    }

    let missing_type_names = type_names(ctx.checker(), &magic_result.missing_output_types);
    let sf = ctx.source_file;
    let layer_identifier = layer_identifier.clone();
    let action = ctx.new_refactor_action(RefactorAction {
        description: "Compose layers automatically with target output services".to_string(),
        run: Box::new(move |tracker: &mut Tracker| {
            build_layer_magic_build(
                tracker,
                sf,
                &missing_type_names,
                at_location,
                &magic_result,
                &layer_identifier,
            );
        }),
    });
    let Some(mut action) = action else {
        return Vec::new();
    };
    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.layerMagicBuild");
    vec![action]
}

/// Rust-only: Go's `c.TypeToStringEx(t, nil, checker.TypeFormatFlagsNoTruncation, nil)` of each type, in order,
/// printed before the action runs (see `try_build_refactor`).
fn type_names(c: &mut Checker, types: &[P<Type>]) -> Vec<String> {
    types
        .iter()
        .map(|&t| c.type_to_string_ex(t, None, TypeFormatFlags::NoTruncation, None))
        .collect()
}

// go: internal/refactors/layer_magic.go:151 buildLayerMagicBuild 4adfdbf48e86808d
// buildLayerMagicBuild generates a pipe from the first requested output layer, or Layer.empty
// when the first layer does not provide a requested output.
//
// Rust: Go's `ctx` is used for its source file and `c` for printing the missing output types, which the caller
// prints (`missing_type_names`, see `try_build_refactor`).
pub(crate) fn build_layer_magic_build(
    tracker: &mut Tracker,
    sf: P<SourceFile>,
    missing_type_names: &[String],
    old_node: P<Node>,
    magic_result: &LayerMagicResult,
    layer_identifier: &ModuleReference,
) {
    let nodes = &magic_result.nodes;
    if nodes.is_empty() {
        return;
    }

    let receiver;
    let mut first_pipe_arg = 0;
    if nodes[0].merges {
        receiver = tracker.deep_clone_node(Some(nodes[0].node)).unwrap();
        first_pipe_arg = 1;
    } else {
        receiver = tracker.new_property_access_path(&layer_identifier.append(&["empty"]));
    }

    let mut pipe_args = Vec::new();
    for mn in &nodes[first_pipe_arg..] {
        // Determine combinator name
        let combinator_name = if mn.merges && mn.provides {
            "provideMerge"
        } else if mn.merges {
            "merge"
        } else {
            "provide"
        };

        // Layer.provideMerge(expr) or Layer.provide(expr) or Layer.merge(expr)
        let combinator_access =
            tracker.new_property_access_path(&layer_identifier.append(&[combinator_name]));
        let cloned_expr = tracker.deep_clone_node(Some(mn.node)).unwrap();
        let call = tracker.new_call_expression(
            combinator_access,
            None,
            None,
            tracker.new_node_list(vec![cloned_expr]),
            NodeFlags::None,
        );
        pipe_args.push(call);
    }

    let pipe_access = tracker.new_property_access_expression(
        receiver,
        None,
        tracker.new_identifier("pipe"),
        NodeFlags::None,
    );
    let mut new_declaration = tracker.new_call_expression(
        pipe_access,
        None,
        None,
        tracker.new_node_list(pipe_args),
        NodeFlags::None,
    );

    // Add trailing comment for missing output types
    if !magic_result.missing_output_types.is_empty() {
        let type_names: Vec<&str> = missing_type_names.iter().map(String::as_str).collect();
        let comment =
            " Unable to find ".to_string() + &type_names.join(", ") + " in the provided layers. ";
        new_declaration = tracker.emit_context.add_synthetic_trailing_comment(
            new_declaration,
            Kind::MultiLineCommentTrivia,
            &comment,
            false,
        );
    }

    ast::set_parent_in_children(new_declaration);

    // If the old node is an expression inside a variable declaration, use replaceExpressionViaStatement
    if old_node.parent().is_some_and(|p| p.kind() == Kind::VariableDeclaration) {
        replace_expression_via_statement(tracker, sf, old_node, new_declaration);
    } else {
        tracker.replace_node(sf, Some(old_node), Some(new_declaration), None);
    }
}

// go: internal/refactors/layer_magic.go:221 tryPrepareRefactor 635329e2566094f8
// tryPrepareRefactor tries to produce the "prepare" refactor action for the given node.
// It flattens a layer expression into `[...] as any as Layer.Layer<T>`.
//
// Rust: Go's `tp` and `c` parameters are the context's; the graphs are returned by value (never nil). Go's
// `map[*checker.Type]bool` is ranged in random order before `sortedTypeSlice` sorts it; the Rust set keeps
// insertion order (the deterministic order of the outline nodes). The type names are printed here, before the
// action runs (see `try_build_refactor`).
pub(crate) fn try_prepare_refactor(
    ctx: &mut Context<'_, '_>,
    node: P<Node>,
    layer_identifier: &ModuleReference,
) -> Vec<CodeAction> {
    let at_location = adjusted_node(node);

    // Skip if already in `as any as Layer.Layer<T>` form
    if is_as_any_as_layer_form(at_location) {
        return Vec::new();
    }

    // Must be an expression
    if !ast::is_expression(at_location) {
        return Vec::new();
    }

    // Extract layer graph
    let layer_graph = ets_layergraph::extract_layer_graph(
        &mut ctx.type_parser,
        &[at_location],
        Some(ctx.source_file),
        ExtractLayerGraphOptions {
            array_literal_as_merge: true,
            explode_only_layer_calls: true,
            follow_symbols_depth: 0,
            ..Default::default()
        },
    );

    // Extract outline graph
    let outline_graph = ets_layergraph::extract_outline_graph(&mut ctx.type_parser, &layer_graph);
    if outline_graph.node_count() <= 1 {
        return Vec::new();
    }

    // Collect all Provides types from outline nodes
    let mut layer_output_types: Vec<P<Type>> = Vec::new();
    for (_, node_info) in outline_graph.nodes() {
        for &p in &node_info.provides {
            if !layer_output_types.contains(&p) {
                layer_output_types.push(p);
            }
        }
    }

    // Collect leaf layer nodes (sorted by position)
    let mut layer_nodes = Vec::new();
    for (_, node_info) in outline_graph.nodes() {
        if ast::is_expression(node_info.node) {
            layer_nodes.push(node_info.node);
        }
    }
    let sf = ctx.source_file;
    ets_go::sort::slice(&mut layer_nodes, |&a, &b| {
        astnav::get_start_of_node(a, sf, false) < astnav::get_start_of_node(b, sf, false)
    });

    if layer_nodes.is_empty() {
        return Vec::new();
    }

    // Parse the expression's current type as a Layer to find "previously provided" types
    let mut previously_provided = None;
    if let Some(parsed_layer) = ctx.type_parser.layer_type_at_location(at_location) {
        previously_provided = Some(parsed_layer.r_out());
    }

    // Partition types into newlyIntroduced and existingBefore
    let c = ctx.checker();
    let all_output_types = sorted_type_slice(c, &layer_output_types);
    let mut newly_introduced = Vec::new();
    let mut existing_before = Vec::new();
    for t in all_output_types {
        if let Some(previously_provided) = previously_provided
            && c.is_type_assignable_to(t, previously_provided)
        {
            existing_before.push(t);
        } else {
            newly_introduced.push(t);
        }
    }

    let newly_introduced = type_names(c, &newly_introduced);
    let existing_before = type_names(c, &existing_before);
    let layer_identifier = layer_identifier.clone();
    let action = ctx.new_refactor_action(RefactorAction {
        description: "Prepare layers for automatic composition".to_string(),
        run: Box::new(move |tracker: &mut Tracker| {
            build_layer_magic_prepare(
                tracker,
                sf,
                at_location,
                &layer_nodes,
                &newly_introduced,
                &existing_before,
                &layer_identifier,
            );
        }),
    });
    let Some(mut action) = action else {
        return Vec::new();
    };
    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.layerMagicPrepare");
    vec![action]
}

// go: internal/refactors/layer_magic.go:306 buildLayerMagicPrepare 13661bf984b27139
// buildLayerMagicPrepare generates: [leaf1, leaf2, ...] as any as Layer.Layer<NewTypes /* ExistingTypes */>
//
// Rust: Go's `ctx` is used for its source file, and `c` for printing `newlyIntroduced` and `existingBefore`,
// which the caller prints (see `try_prepare_refactor`).
pub(crate) fn build_layer_magic_prepare(
    tracker: &mut Tracker,
    sf: P<SourceFile>,
    old_node: P<Node>,
    layer_nodes: &[P<Node>],
    newly_introduced: &[String],
    existing_before: &[String],
    layer_identifier: &ModuleReference,
) {
    // Build array of cloned layer expressions
    let mut elements = Vec::new();
    for &entry in layer_nodes {
        elements.push(tracker.deep_clone_node(Some(entry)).unwrap());
    }
    let array_literal =
        tracker.new_array_literal_expression(tracker.new_node_list(elements), false);

    // Build `any` type node
    let any_type = tracker.new_keyword_type_node(Kind::AnyKeyword);

    // Inner as expression: `[...] as any`
    let inner_as = tracker.new_as_expression(array_literal, any_type);

    // Build the provides union type node
    let mut provides_type = if newly_introduced.is_empty() {
        tracker.new_type_reference_node(tracker.new_identifier("never"), None)
    } else {
        let mut type_nodes = Vec::new();
        for type_str in newly_introduced {
            type_nodes.push(
                tracker.new_type_reference_node(tracker.new_identifier(alloc_str(type_str)), None),
            );
        }
        if type_nodes.len() == 1 {
            type_nodes[0]
        } else {
            tracker.new_union_type_node(tracker.new_node_list(type_nodes))
        }
    };

    // Add trailing comment for existing types
    if !existing_before.is_empty() {
        let type_strings: Vec<&str> = existing_before.iter().map(String::as_str).collect();
        let comment = " ".to_string() + &type_strings.join(" | ") + " ";
        provides_type = tracker.emit_context.add_synthetic_trailing_comment(
            provides_type,
            Kind::MultiLineCommentTrivia,
            &comment,
            false,
        );
    }

    // Build Layer.Layer<providesType>
    let qualified_name = tracker.new_qualified_name_path(&layer_identifier.append(&["Layer"]));
    let layer_type_ref = tracker
        .new_type_reference_node(qualified_name, Some(tracker.new_node_list(vec![provides_type])));

    // Outer as expression: `([...] as any) as Layer.Layer<T>`
    let outer_as = tracker.new_as_expression(inner_as, layer_type_ref);

    ast::set_parent_in_children(outer_as);

    // If the old node is an expression inside a variable declaration, use replaceExpressionViaStatement
    if old_node.parent().is_some_and(|p| p.kind() == Kind::VariableDeclaration) {
        replace_expression_via_statement(tracker, sf, old_node, outer_as);
    } else {
        tracker.replace_node(sf, Some(old_node), Some(outer_as), None);
    }
}

// go: internal/refactors/layer_magic.go:374 isAsAnyAsLayerForm 648cf73415fbd888
// isAsAnyAsLayerForm checks if a node is already in the `(expr as any) as Layer.Layer<T>` form.
pub(crate) fn is_as_any_as_layer_form(node: P<Node>) -> bool {
    if node.kind() != Kind::AsExpression {
        return false;
    }
    let outer_as = node.as_as_expression();
    if outer_as.type_().kind() != Kind::TypeReference {
        return false;
    }
    let inner_expr = outer_as.expression();
    if inner_expr.kind() != Kind::AsExpression {
        return false;
    }
    let inner_as = inner_expr.as_as_expression();
    inner_as.type_().kind() == Kind::AnyKeyword
}

// go: internal/refactors/layer_magic.go:391 sortedTypeSlice 96c65de830a8f01c
// sortedTypeSlice converts a type set to a sorted slice using TypeToString for deterministic ordering.
//
// Rust: the set is an insertion-ordered slice (see `try_prepare_refactor`); the sort is Go's `sort.Slice`.
pub(crate) fn sorted_type_slice(c: &mut Checker, types: &[P<Type>]) -> Vec<P<Type>> {
    let mut result = Vec::with_capacity(types.len());
    for &t in types {
        result.push(t);
    }
    ets_go::sort::slice(&mut result, |&a, &b| {
        c.type_to_string_ex(a, None, TypeFormatFlags::NoTruncation, None)
            < c.type_to_string_ex(b, None, TypeFormatFlags::NoTruncation, None)
    });
    result
}
