use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::{self, Kind, NodeFlags};
use ets_shim::core::alloc_str;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::CONTEXT_SERVICE_IS_ASSIGNED_TO_A_VARIABLE_HERE_BUT_THIS_API_IS_INTENDED_FOR_A_CLASS_DECLARATION_SHAPE_SUCH_AS_0_EFFECT_SERVICE_NOT_AS_CLASS;

// go: internal/fixables/service_not_as_class.go:12 ServiceNotAsClassFix
pub static SERVICE_NOT_AS_CLASS_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "serviceNotAsClass",
    description: "Convert to class declaration",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["serviceNotAsClass_fix"],
    run: run_service_not_as_class_fix,
});

// go: internal/fixables/service_not_as_class.go:20 runServiceNotAsClassFix
//
// Rust: tsrs's call expressions always have an argument list; Go's nil `Arguments` (the inner call, and the outer
// one without arguments) is an empty list, which prints the same `()`.
fn run_service_not_as_class_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    let matches = ets_rules::analyze_service_not_as_class(&mut ctx.type_parser, sf);
    for m in matches {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Convert to class declaration".to_string(),
            run: Box::new(move |tracker| {
                let call_expr = m.call_expr_node.as_call_expression();

                // Build the original service namespace property access.
                let service_map_service = tracker.new_property_access_expression(
                    tracker.new_identifier(m.service_module),
                    None,
                    tracker.new_identifier("Service"),
                    NodeFlags::None,
                );

                // Build combined type arguments: <ClassName, ...OriginalTypeArgs>
                let self_type_ref = tracker.new_type_reference_node(
                    tracker.new_identifier(alloc_str(&m.variable_name)),
                    None,
                );
                let mut type_arg_nodes = vec![self_type_ref];
                if let Some(type_arguments) = call_expr.type_arguments() {
                    for &ta in type_arguments.nodes() {
                        type_arg_nodes.push(tracker.deep_clone_node(Some(ta)).unwrap());
                    }
                }

                // Build inner call: <ServiceModule>.Service<Self, ...TypeArgs>()
                let inner_call = tracker.new_call_expression(
                    service_map_service,
                    None,
                    Some(tracker.new_node_list(type_arg_nodes)),
                    tracker.new_node_list(Vec::new()),
                    NodeFlags::None,
                );

                // Build outer call: innerCall(args...)
                let mut cloned_args = tracker.new_node_list(Vec::new());
                if !call_expr.arguments().nodes().is_empty() {
                    let arg_nodes = call_expr
                        .arguments()
                        .nodes()
                        .iter()
                        .map(|&arg| tracker.deep_clone_node(Some(arg)).unwrap())
                        .collect();
                    cloned_args = tracker.new_node_list(arg_nodes);
                }
                let outer_call = tracker.new_call_expression(
                    inner_call,
                    None,
                    None,
                    cloned_args,
                    NodeFlags::None,
                );

                // Build heritage clause: extends outerCall
                let expr_with_type_args =
                    tracker.new_expression_with_type_arguments(outer_call, None);
                let heritage_clause = tracker.new_heritage_clause(
                    Kind::ExtendsKeyword,
                    tracker.new_node_list(vec![expr_with_type_args]),
                );

                // Build modifiers for the class declaration
                let mut modifiers = None;
                if let Some(modifier_nodes) = m.modifier_nodes
                    && !modifier_nodes.nodes().is_empty()
                {
                    let mod_nodes = modifier_nodes
                        .nodes()
                        .iter()
                        .map(|&modifier| tracker.new_modifier(modifier.kind()))
                        .collect();
                    modifiers = Some(tracker.new_modifier_list(mod_nodes));
                }

                // Build class declaration
                let class_decl = tracker.new_class_declaration(
                    modifiers,
                    Some(tracker.new_identifier(alloc_str(&m.variable_name))),
                    None, // no type parameters
                    Some(tracker.new_node_list(vec![heritage_clause])),
                    tracker.new_node_list(Vec::new()), // empty members
                );

                ast::set_parent_in_children(class_decl);
                tracker.replace_node(sf, Some(m.target_node), Some(class_decl), None);
            }),
        }) {
            return vec![action];
        }
        return Vec::new();
    }

    Vec::new()
}
