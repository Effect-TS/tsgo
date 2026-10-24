use std::sync::LazyLock;

use ets_refactor::{Context, Refactor, RefactorAction};
use ets_rewriter::Tracker;
use ets_shim::ast::{self, Kind, ModifierFlags, ModifierList, Node, NodeFactory, NodeFlags};
use ets_shim::astnav;
use ets_shim::core::P;
use ets_shim::ls::CodeAction;
use ets_shim::lsproto;

use crate::*;

// go: internal/refactors/function_to_arrow.go:12 FunctionToArrow
pub static FUNCTION_TO_ARROW: LazyLock<Refactor> = LazyLock::new(|| Refactor {
    name: "functionToArrow",
    description: "Convert to arrow",
    kind: "rewrite.effect.functionToArrow",
    run: run_function_to_arrow,
});

// go: internal/refactors/function_to_arrow.go:19 runFunctionToArrow
// Rust: tsrs's `get_token_at_position` never returns nil, so Go's nil check drops.
pub(crate) fn run_function_to_arrow(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let token = astnav::get_token_at_position(ctx.source_file, span_pos(ctx));

    // Walk ancestor chain looking for a FunctionDeclaration or MethodDeclaration
    let mut matched_node = None;
    let mut node = Some(token);
    while let Some(n) = node {
        node = n.parent();
        if n.kind() != Kind::FunctionDeclaration && n.kind() != Kind::MethodDeclaration {
            continue;
        }

        // Must have a body
        if ets_typeparser::get_function_like_body(n).is_none() {
            continue;
        }

        // Exclude async functions (those are handled by asyncAwaitToFn/asyncAwaitToGen)
        if ast::get_combined_modifier_flags(n).intersects(ModifierFlags::Async) {
            continue;
        }

        // Must have a name and cursor must be on it
        let name = match n.kind() {
            Kind::FunctionDeclaration => n.as_function_declaration().name(),
            Kind::MethodDeclaration => Some(n.as_method_declaration().name()),
            _ => None,
        };
        let Some(name) = name else {
            continue;
        };

        let name_pos = astnav::get_start_of_node(name, ctx.source_file, false);
        let name_end = name.end().expect_source("a parsed name");
        if span_pos(ctx) < name_pos || span_pos(ctx) > name_end {
            continue;
        }

        matched_node = Some(n);
        break;
    }

    let Some(matched_node) = matched_node else {
        return Vec::new();
    };

    let sf = ctx.source_file;
    let action = ctx.new_refactor_action(RefactorAction {
        description: "Convert to arrow".to_string(),
        run: Box::new(move |tracker: &mut Tracker| {
            let Some(new_node) = build_function_to_arrow_replacement(tracker, matched_node) else {
                return;
            };
            ast::set_parent_in_children(new_node);
            tracker.replace_node(sf, Some(matched_node), Some(new_node), None);
        }),
    });
    let Some(mut action) = action else {
        return Vec::new();
    };
    action.kind = lsproto::CodeActionKind("refactor.rewrite.effect.functionToArrow");
    vec![action]
}

// go: internal/refactors/function_to_arrow.go:89 buildFunctionToArrowReplacement
// buildFunctionToArrowReplacement builds the replacement node for converting a
// FunctionDeclaration or MethodDeclaration to an arrow function.
pub(crate) fn build_function_to_arrow_replacement(
    tracker: &NodeFactory,
    node: P<Node>,
) -> Option<P<Node>> {
    let body = ets_typeparser::get_function_like_body(node)?;

    // Determine concise vs block body:
    // If body has exactly one ReturnStatement with an expression, use concise body
    let mut new_body = None;
    if body.kind() == Kind::Block {
        let block = body.as_block();
        if block.statements().nodes().len() == 1 {
            let stmt = block.statements().nodes()[0];
            if stmt.kind() == Kind::ReturnStatement {
                let ret = stmt.as_return_statement();
                if let Some(expression) = ret.expression() {
                    // Concise body: just the expression
                    new_body = tracker.deep_clone_node(Some(expression));
                }
            }
        }
        if new_body.is_none() {
            // Block body
            new_body = tracker.deep_clone_node(Some(body));
        }
    } else {
        new_body = tracker.deep_clone_node(Some(body));
    }

    // Build arrow modifiers: strip Export and Default from the arrow itself
    let mut arrow_mods = None;
    if let Some(src_mods) = get_node_modifiers(node) {
        let mut mod_nodes = Vec::new();
        for &m in src_mods.nodes() {
            if m.kind() == Kind::ExportKeyword || m.kind() == Kind::DefaultKeyword {
                continue;
            }
            mod_nodes.push(tracker.new_modifier(m.kind()));
        }
        if !mod_nodes.is_empty() {
            arrow_mods = Some(tracker.new_modifier_list(mod_nodes));
        }
    }

    // Build the arrow function
    let type_params =
        clone_node_list(tracker, ets_typeparser::get_function_like_type_parameters(node));
    let params = clone_node_list(tracker, ets_typeparser::get_function_like_parameters(node));

    let arrow_fn = tracker.new_arrow_function(
        arrow_mods,
        type_params,
        params,
        None, // returnType
        None, // fullSignature
        Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
        new_body,
    );

    // Wrap in appropriate declaration
    match node.kind() {
        Kind::FunctionDeclaration => {
            Some(wrap_arrow_in_variable_statement(tracker, node, arrow_fn))
        }
        Kind::MethodDeclaration => {
            Some(wrap_arrow_in_property_declaration(tracker, node, arrow_fn))
        }
        _ => Some(arrow_fn),
    }
}

// go: internal/refactors/function_to_arrow.go:161 wrapArrowInVariableStatement
// wrapArrowInVariableStatement wraps an arrow function in a const variable statement,
// preserving export/default modifiers on the variable statement.
pub(crate) fn wrap_arrow_in_variable_statement(
    tracker: &NodeFactory,
    node: P<Node>,
    arrow_fn: P<Node>,
) -> P<Node> {
    let fd = node.as_function_declaration();
    let Some(name) = fd.name() else {
        return arrow_fn;
    };

    // Collect modifiers for the variable statement (keep all original modifiers)
    let mut mods = None;
    if let Some(modifiers) = fd.modifiers() {
        let mut mod_nodes = Vec::new();
        for &m in modifiers.nodes() {
            mod_nodes.push(tracker.new_modifier(m.kind()));
        }
        if !mod_nodes.is_empty() {
            mods = Some(tracker.new_modifier_list(mod_nodes));
        }
    }

    let var_decl = tracker.new_variable_declaration(
        tracker.deep_clone_node(Some(name)).unwrap(),
        None, // exclamationToken
        None, // type
        Some(arrow_fn),
    );
    let var_decl_list = tracker
        .new_variable_declaration_list(tracker.new_node_list(vec![var_decl]), NodeFlags::Const);
    tracker.new_variable_statement(mods, var_decl_list)
}

// go: internal/refactors/function_to_arrow.go:194 wrapArrowInPropertyDeclaration
// wrapArrowInPropertyDeclaration wraps an arrow function in a property declaration,
// preserving modifiers from the method.
pub(crate) fn wrap_arrow_in_property_declaration(
    tracker: &NodeFactory,
    node: P<Node>,
    arrow_fn: P<Node>,
) -> P<Node> {
    let md = node.as_method_declaration();

    let mut mods = None;
    if let Some(modifiers) = md.modifiers() {
        let mut mod_nodes = Vec::new();
        for &m in modifiers.nodes() {
            mod_nodes.push(tracker.new_modifier(m.kind()));
        }
        if !mod_nodes.is_empty() {
            mods = Some(tracker.new_modifier_list(mod_nodes));
        }
    }

    tracker.new_property_declaration(
        mods,
        tracker.deep_clone_node(Some(md.name())).unwrap(),
        None, // postfixToken
        None, // type
        Some(arrow_fn),
    )
}

// go: internal/refactors/function_to_arrow.go:218 getNodeModifiers
// getNodeModifiers returns the modifier list from a function-like node.
pub(crate) fn get_node_modifiers(node: P<Node>) -> Option<P<ModifierList>> {
    match node.kind() {
        Kind::FunctionDeclaration => node.as_function_declaration().modifiers(),
        Kind::MethodDeclaration => node.as_method_declaration().modifiers(),
        _ => None,
    }
}
