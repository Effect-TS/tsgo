use ets_graph::{Graph, NodeIndex};
use ets_shim::ast::{Kind, Node, SourceFile, SymbolFlags};
use ets_shim::checker::{ContextFlags, SignatureKind, Type, TypeFlags};
use ets_shim::core::P;
use ets_typeparser::TypeParser;
use rustc_hash::FxHashMap;

use crate::*;

// go: internal/layergraph/extract.go:14 workItem
// workItem represents a node to visit in the DFS work queue.
pub(crate) struct WorkItem {
    node: P<Node>,
    #[expect(dead_code, reason = "Go stores it and reads the depth budget map instead")]
    depth: i64,
}

/// Rust-only: Go's `appendNodeToVisit` closure over the depth budget and the work stack.
fn append_node_to_visit(
    depth_budget: &mut FxHashMap<P<Node>, i64>,
    stack: &mut Vec<WorkItem>,
    n: P<Node>,
    depth: i64,
) {
    depth_budget.insert(n, depth);
    stack.push(WorkItem { node: n, depth });
}

/// Rust-only: Go's `addNode` closure over the graph and the node-to-index map.
fn add_node(
    g: &mut Graph<LayerGraphNodeInfo, LayerGraphEdgeInfo>,
    node_to_graph_index: &mut FxHashMap<P<Node>, NodeIndex>,
    n: P<Node>,
    info: LayerGraphNodeInfo,
) -> NodeIndex {
    let idx = g.add_node(info);
    node_to_graph_index.insert(n, idx);
    idx
}

// go: internal/layergraph/extract.go:23 ExtractLayerGraph
// ExtractLayerGraph builds a directed graph of Layer composition from the given AST nodes.
// It performs a DFS with a two-pass approach using an explicit work stack:
//   - First pass: push children for processing
//   - Second pass: link processed children into the graph
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`). Go returns a `*Graph`; the graph is
// returned by value.
#[expect(clippy::too_many_lines, reason = "one Go function")]
pub fn extract_layer_graph(
    tp: &mut TypeParser<'_>,
    nodes: &[P<Node>],
    sf: Option<P<SourceFile>>,
    opts: ExtractLayerGraphOptions,
) -> Graph<LayerGraphNodeInfo, LayerGraphEdgeInfo> {
    let mut g = ets_graph::new::<LayerGraphNodeInfo, LayerGraphEdgeInfo>();
    let mut node_to_graph_index: FxHashMap<P<Node>, NodeIndex> = FxHashMap::default();
    let mut visited_nodes: FxHashMap<P<Node>, bool> = FxHashMap::default();
    let mut node_in_pipe_context: FxHashMap<P<Node>, bool> = FxHashMap::default();
    let mut depth_budget: FxHashMap<P<Node>, i64> = FxHashMap::default();

    // Resolve the Layer module import name for ExplodeOnlyLayerCalls checks.
    let mut layer_module_name = String::new();
    if !opts.skip_explode {
        layer_module_name = find_layer_module_name(sf);
    }

    // Initialize the work stack with the root nodes.
    let mut stack: Vec<WorkItem> = Vec::new();

    for &node in nodes {
        append_node_to_visit(&mut depth_budget, &mut stack, node, opts.follow_symbols_depth);
    }

    let visited = |visited_nodes: &FxHashMap<P<Node>, bool>, n: P<Node>| {
        visited_nodes.get(&n).copied().unwrap_or(false)
    };
    let in_pipe_context = |node_in_pipe_context: &FxHashMap<P<Node>, bool>, n: P<Node>| {
        node_in_pipe_context.get(&n).copied().unwrap_or(false)
    };

    while let Some(item) = stack.pop() {
        // Pop from the stack (LIFO).
        let current = item.node;
        let current_depth = depth_budget.get(&current).copied().unwrap_or(0);

        // Case 1: Pipe detection
        if !opts.skip_explode
            && let Some(pipe_result) = tp.parse_pipe_call(current)
        {
            if visited(&visited_nodes, current) {
                // Second pass: collect child graph indices.
                let mut all_children: Vec<P<Node>> = vec![pipe_result.subject()];
                all_children = pipe_result.args().append_to(all_children);

                let child_indices = collect_child_indices(&all_children, &node_to_graph_index, &g);

                if child_indices.len() == all_children.len() {
                    // All members are graph nodes — link them sequentially.
                    let mut last_idx: NodeIndex = 0;
                    for (i, &child_idx) in child_indices.iter().enumerate() {
                        if i > 0 {
                            g.add_edge(
                                child_idx,
                                last_idx,
                                LayerGraphEdgeInfo::new(EdgeRelationship::Pipe),
                            );
                        }
                        last_idx = child_idx;
                    }
                    // Add a node for the pipe call itself, linking to the last child.
                    let info = extract_node_info(
                        tp,
                        current,
                        sf,
                        in_pipe_context(&node_in_pipe_context, current),
                    );
                    let pipe_idx = add_node(&mut g, &mut node_to_graph_index, current, info);
                    g.add_edge(pipe_idx, last_idx, LayerGraphEdgeInfo::new(EdgeRelationship::Pipe));
                } else {
                    // Not all children are graph nodes — remove partial children and try as leaf.
                    remove_partial_children(&all_children, &mut node_to_graph_index, &mut g);
                    let info = extract_node_info(
                        tp,
                        current,
                        sf,
                        in_pipe_context(&node_in_pipe_context, current),
                    );
                    if info.layer_type.is_some() {
                        add_node(&mut g, &mut node_to_graph_index, current, info);
                    }
                }
            } else {
                // First pass: push self back, then subject and args.
                append_node_to_visit(&mut depth_budget, &mut stack, current, current_depth);
                append_node_to_visit(
                    &mut depth_budget,
                    &mut stack,
                    pipe_result.subject(),
                    current_depth,
                );
                for (_, &arg) in pipe_result.args().all() {
                    append_node_to_visit(&mut depth_budget, &mut stack, arg, current_depth);
                    node_in_pipe_context.insert(arg, true);
                }
                visited_nodes.insert(current, true);
            }
            continue;
        }

        // Case 2: Call expression
        if !opts.skip_explode && current.kind() == Kind::CallExpression {
            let mut should_explode = !opts.explode_only_layer_calls;
            if opts.explode_only_layer_calls && is_layer_module_call(current, &layer_module_name) {
                should_explode = true;
            }
            if should_explode {
                // Rust: tsrs's call expressions always have an argument list (Go checks for nil).
                let args: &[P<Node>] = current.as_call_expression().arguments().nodes();
                if visited(&visited_nodes, current) {
                    // Second pass: collect child graph indices.
                    let child_indices = collect_child_indices(args, &node_to_graph_index, &g);

                    if child_indices.len() == args.len() {
                        // All arguments are graph nodes.
                        let info = extract_node_info(
                            tp,
                            current,
                            sf,
                            in_pipe_context(&node_in_pipe_context, current),
                        );
                        let call_idx = add_node(&mut g, &mut node_to_graph_index, current, info);
                        for (i, &child_idx) in child_indices.iter().enumerate() {
                            g.add_edge(
                                call_idx,
                                child_idx,
                                LayerGraphEdgeInfo {
                                    relationship: EdgeRelationship::Call,
                                    argument_index: i as i64,
                                    index: 0,
                                },
                            );
                        }
                    } else {
                        // Not all arguments are graph nodes.
                        remove_partial_children(args, &mut node_to_graph_index, &mut g);
                        let info = extract_node_info(
                            tp,
                            current,
                            sf,
                            in_pipe_context(&node_in_pipe_context, current),
                        );
                        if info.layer_type.is_some() {
                            add_node(&mut g, &mut node_to_graph_index, current, info);
                        }
                    }
                } else {
                    // First pass: push self back, then all arguments.
                    append_node_to_visit(&mut depth_budget, &mut stack, current, current_depth);
                    for &arg in args {
                        append_node_to_visit(&mut depth_budget, &mut stack, arg, current_depth);
                    }
                    visited_nodes.insert(current, true);
                }
                continue;
            }
        }

        // Case 3: Array literal (when ArrayLiteralAsMerge is enabled)
        if !opts.skip_explode
            && opts.array_literal_as_merge
            && current.kind() == Kind::ArrayLiteralExpression
        {
            // Rust: tsrs's array literals always have an element list (Go checks for nil).
            let elements: &[P<Node>] = current.as_array_literal_expression().elements().nodes();
            if visited(&visited_nodes, current) {
                // Second pass: collect child graph indices (doesn't require ALL).
                let child_indices = collect_child_indices(elements, &node_to_graph_index, &g);
                if !child_indices.is_empty() {
                    let info = extract_node_info(
                        tp,
                        current,
                        sf,
                        in_pipe_context(&node_in_pipe_context, current),
                    );
                    let array_idx = add_node(&mut g, &mut node_to_graph_index, current, info);
                    for (i, &child_idx) in child_indices.iter().enumerate() {
                        g.add_edge(
                            array_idx,
                            child_idx,
                            LayerGraphEdgeInfo {
                                relationship: EdgeRelationship::ArrayLiteral,
                                argument_index: 0,
                                index: i as i64,
                            },
                        );
                    }
                }
            } else {
                // First pass: push self back, then all elements.
                append_node_to_visit(&mut depth_budget, &mut stack, current, current_depth);
                for &elem in elements {
                    append_node_to_visit(&mut depth_budget, &mut stack, elem, current_depth);
                }
                visited_nodes.insert(current, true);
            }
            continue;
        }

        // Case 4: Symbol following
        if current_depth > 0
            && is_simple_identifier(current)
            && let Some(mut sym) = tp.get_symbol_at_location(current)
        {
            if sym.flags().intersects(SymbolFlags::Alias) {
                // Rust: tsrs's skip_alias never returns nil (Go keeps the symbol when it does).
                sym = ets_shim::checker::skip_alias(sym, tp.checker());
            }
            if sym.declarations().len() == 1
                && let Some(decl_node) = get_adjusted_node(sym.declarations()[0])
            {
                if !visited(&visited_nodes, current) {
                    // First pass: push self back, push declaration with decremented depth.
                    append_node_to_visit(&mut depth_budget, &mut stack, current, current_depth);
                    append_node_to_visit(
                        &mut depth_budget,
                        &mut stack,
                        decl_node,
                        current_depth - 1,
                    );
                    visited_nodes.insert(current, true);
                    continue;
                }
                // Second pass: link to declaration if it's in the graph.
                if let Some(&child_idx) = node_to_graph_index.get(&decl_node) {
                    let info = extract_node_info(
                        tp,
                        current,
                        sf,
                        in_pipe_context(&node_in_pipe_context, current),
                    );
                    let ident_idx = add_node(&mut g, &mut node_to_graph_index, current, info);
                    g.add_edge(
                        ident_idx,
                        child_idx,
                        LayerGraphEdgeInfo::new(EdgeRelationship::Symbol),
                    );
                    continue;
                }
            }
        }

        // Case 5: Leaf node (base case)
        if ets_shim::ast::is_expression(current) {
            let info =
                extract_node_info(tp, current, sf, in_pipe_context(&node_in_pipe_context, current));
            if info.layer_type.is_some() {
                add_node(&mut g, &mut node_to_graph_index, current, info);
            }
        }
    }

    g
}

// go: internal/layergraph/extract.go:232 extractNodeInfo
// extractNodeInfo computes the LayerGraphNodeInfo for a given AST node.
pub(crate) fn extract_node_info(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
    _sf: Option<P<SourceFile>>,
    in_pipe_context: bool,
) -> LayerGraphNodeInfo {
    let mut info = LayerGraphNodeInfo {
        node,
        display_node: get_display_node(node),
        layer_type: None,
        provides: Vec::new(),
        actual_provides: Vec::new(),
        requires: Vec::new(),
    };

    // Get the type of the node.
    // When in a pipe context, resolve the contextual type's return type
    // (the pipe argument is a function whose return type is the Layer).
    let mut t: Option<P<Type>> = None;
    if in_pipe_context && ets_shim::ast::is_expression(node) {
        let contextual_type = tp.get_contextual_type(node, ContextFlags::None);
        if let Some(contextual_type) = contextual_type {
            let call_signatures =
                tp.checker().get_signatures_of_type_exported(contextual_type, SignatureKind::Call);
            if call_signatures.len() == 1 {
                t = Some(tp.checker().get_return_type_of_signature_exported(call_signatures[0]));
            }
        }
    } else {
        t = tp.get_type_at_location(node);
    }
    let Some(t) = t else {
        return info;
    };

    // Parse as Layer type.
    let Some(layer) = tp.layer_type(t) else {
        return info;
    };
    info.layer_type = Some(layer);

    // Unroll provides and requires, filtering out Never types.
    for p in tp.unroll_union_members(layer.r_out()).into_iter().flatten() {
        if !p.flags().intersects(TypeFlags::Never) {
            info.provides.push(p);
        }
    }
    for r in tp.unroll_union_members(layer.r_in()).into_iter().flatten() {
        if !r.flags().intersects(TypeFlags::Never) {
            info.requires.push(r);
        }
    }

    // ActualProvides: provides that are NOT assignable to the RIn type.
    for &p in &info.provides {
        if !tp.checker().is_type_assignable_to(p, layer.r_in()) {
            info.actual_provides.push(p);
        }
    }

    // Sort all type lists deterministically by their string representation.
    // NOTE: Intentional divergence from .repos reference. The TypeScript implementation
    // preserves union member order in output/nested formats (using deterministicTypeOrder
    // only for providers/requirers extraction and quickinfo). We sort alphabetically
    // everywhere for consistent determinism independent of compiler-internal union ordering.
    let mut sort_types = |types: &mut Vec<P<Type>>| {
        ets_go::sort::slice(types, |&a, &b| {
            tp.checker().type_to_string_exported(a) < tp.checker().type_to_string_exported(b)
        });
    };
    sort_types(&mut info.provides);
    sort_types(&mut info.requires);
    sort_types(&mut info.actual_provides);

    info
}

// go: internal/layergraph/extract.go:303 getDisplayNode
// getDisplayNode returns the node used for display purposes.
// If the node's parent is a variable declaration and the node is the initializer,
// use the variable's name node instead.
pub(crate) fn get_display_node(node: P<Node>) -> P<Node> {
    if let Some(parent) = node.parent()
        && parent.kind() == Kind::VariableDeclaration
    {
        let var_decl = parent.as_variable_declaration();
        if var_decl.initializer() == Some(node) {
            return var_decl.name();
        }
    }
    if let Some(parent) = node.parent()
        && parent.kind() == Kind::PropertyDeclaration
    {
        let prop_decl = parent.as_property_declaration();
        if prop_decl.initializer() == Some(node)
            && let Some(name) = parent.name()
        {
            return name;
        }
    }
    node
}

// go: internal/layergraph/extract.go:321 getAdjustedNode
// getAdjustedNode extracts the initializer expression from a declaration node
// to follow during symbol resolution.
pub(crate) fn get_adjusted_node(node: P<Node>) -> Option<P<Node>> {
    match node.kind() {
        Kind::VariableDeclaration => node.as_variable_declaration().initializer(),
        Kind::PropertyDeclaration => node.as_property_declaration().initializer(),
        _ => {
            if ets_shim::ast::is_expression(node) {
                return Some(node);
            }
            None
        }
    }
}

// go: internal/layergraph/extract.go:337 isSimpleIdentifier
// isSimpleIdentifier checks if a node is a simple identifier or a chain of property accesses
// with simple identifiers (e.g., `a`, `a.b`, `a.b.c`).
pub(crate) fn is_simple_identifier(node: P<Node>) -> bool {
    if node.kind() == Kind::Identifier {
        return true;
    }
    if node.kind() == Kind::PropertyAccessExpression {
        let prop = node.as_property_access_expression();
        return prop.name().kind() == Kind::Identifier && is_simple_identifier(prop.expression());
    }
    false
}

// go: internal/layergraph/extract.go:350 isLayerModuleCall
// isLayerModuleCall checks if a call expression's callee is a Layer module API call
// (e.g., Layer.provide, Layer.merge, etc.).
//
// Rust: `call_expr` is the call expression node (Go's `*ast.CallExpression`).
pub(crate) fn is_layer_module_call(call_expr: P<Node>, layer_module_name: &str) -> bool {
    let expr = call_expr.as_call_expression().expression();
    if expr.kind() != Kind::PropertyAccessExpression {
        return false;
    }
    let prop_access = expr.as_property_access_expression();
    if prop_access.expression().kind() != Kind::Identifier {
        return false;
    }
    ets_shim::scanner::get_text_of_node(prop_access.expression()) == layer_module_name
}

// go: internal/layergraph/extract.go:364 findLayerModuleName
// findLayerModuleName resolves the imported identifier name for the "Layer" export
// from the "effect" package. Falls back to "Layer" if not found.
pub(crate) fn find_layer_module_name(sf: Option<P<SourceFile>>) -> String {
    let Some(sf) = sf else {
        return "Layer".to_string();
    };
    for &stmt in sf.as_node().statements() {
        if stmt.kind() != Kind::ImportDeclaration {
            continue;
        }
        let import_decl = stmt.as_import_declaration();
        // Rust: tsrs's import declarations always have a module specifier (Go checks for nil).
        let mut module_name = ets_shim::scanner::get_text_of_node(import_decl.module_specifier());
        // Strip quotes from module specifier.
        if module_name.len() >= 2
            && (module_name.as_bytes()[0] == b'"' || module_name.as_bytes()[0] == b'\'')
        {
            module_name = module_name[1..module_name.len() - 1].to_string();
        }
        if module_name != "effect" {
            continue;
        }
        // Look for named imports.
        let Some(import_clause) = import_decl.import_clause() else {
            continue;
        };
        let clause = import_clause.as_import_clause();
        let Some(named_bindings) = clause.named_bindings() else {
            continue;
        };
        if named_bindings.kind() != Kind::NamedImports {
            continue;
        }
        let named_imports = named_bindings.as_named_imports();
        // Rust: tsrs's named imports always have an element list (Go checks for nil).
        for &elem in named_imports.elements().nodes() {
            let spec = elem.as_import_specifier();
            // The "real" name is PropertyName if set (import { Layer as X }),
            // otherwise Name.
            let imported_name = match spec.property_name() {
                Some(property_name) => ets_shim::scanner::get_text_of_node(property_name),
                None => ets_shim::scanner::get_text_of_node(spec.name()),
            };
            if imported_name == "Layer" {
                return ets_shim::scanner::get_text_of_node(spec.name());
            }
        }
    }
    "Layer".to_string()
}

// go: internal/layergraph/extract.go:416 collectChildIndices
// collectChildIndices gathers graph node indices for the given AST nodes,
// filtering out nodes that aren't in the graph.
pub(crate) fn collect_child_indices(
    nodes: &[P<Node>],
    node_to_graph_index: &FxHashMap<P<Node>, NodeIndex>,
    g: &Graph<LayerGraphNodeInfo, LayerGraphEdgeInfo>,
) -> Vec<NodeIndex> {
    let mut indices = Vec::new();
    for n in nodes {
        if let Some(&idx) = node_to_graph_index.get(n)
            && g.has_node(idx)
        {
            indices.push(idx);
        }
    }
    indices
}

// go: internal/layergraph/extract.go:431 removePartialChildren
// removePartialChildren removes graph nodes for the given AST nodes.
pub(crate) fn remove_partial_children(
    nodes: &[P<Node>],
    node_to_graph_index: &mut FxHashMap<P<Node>, NodeIndex>,
    g: &mut Graph<LayerGraphNodeInfo, LayerGraphEdgeInfo>,
) {
    for n in nodes {
        if let Some(&idx) = node_to_graph_index.get(n) {
            g.remove_node(idx);
            node_to_graph_index.remove(n);
        }
    }
}
