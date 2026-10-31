use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::P;
use ets_shim::ast::{self, Kind, Node, NodeFactory, NodeFlags, NodeList};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_SCOPED_PROVIDES_A_SCOPE_THAT_THIS_BODY_IMMEDIATELY_READS_BACK_WITH_EFFECT_SCOPE_EFFECT_SCOPED_WITH_PASSES_THE_SCOPE_TO_THE_CALLBACK_DIRECTLY_EFFECT_SCOPED_IMMEDIATE_SCOPE_TO_SCOPED_WITH;

// go: internal/fixables/scoped_immediate_scope_to_scoped_with.go:13 ScopedImmediateScopeToScopedWithFix abd82d672ed07f4f
pub static SCOPED_IMMEDIATE_SCOPE_TO_SCOPED_WITH_FIX: LazyLock<Fixable> =
    LazyLock::new(|| Fixable {
        name: "scopedImmediateScopeToScopedWith",
        description: "Replace with Effect.scopedWith",
        error_codes: vec![MESSAGE.code()],
        fix_ids: vec!["scopedImmediateScopeToScopedWith_fix"],
        run: run_scoped_immediate_scope_to_scoped_with_fix,
    });

// go: internal/fixables/scoped_immediate_scope_to_scoped_with.go:23 runScopedImmediateScopeToScopedWithFix 69e759f56d854af2
fn run_scoped_immediate_scope_to_scoped_with_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in
        ets_rules::analyze_scoped_immediate_scope_to_scoped_with(&mut ctx.type_parser, Some(sf))
    {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        if !m.fixable {
            continue;
        }
        let member = effect_module_member("scopedWith");
        let scoped_callee = m.scoped.callee();
        // Keep the qualifier of Effect.scoped when it is the Effect module, which
        // already resolves where the edit lands; otherwise go through the file's
        // Effect import.
        let mut qualifier: Option<P<Node>> = None;
        let mut target: Option<ModuleMemberReference> = None;
        if ast::is_property_access_expression(scoped_callee) {
            let expression = scoped_callee.as_property_access_expression().expression();
            if ctx.type_parser.is_expression_effect_module(expression) {
                let expression_type = ctx.type_parser.get_type_at_location(expression);
                if type_has_member(ctx, expression_type, &member) {
                    qualifier = Some(expression);
                }
            }
        }
        if qualifier.is_none() {
            target = imported_module_member(
                ctx,
                scoped_callee,
                &ets_typeparser::find_effect_module_reference(Some(sf)),
                &member,
            );
            if target.is_none() {
                continue;
            }
        }

        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.scopedWith".to_string(),
            run: Box::new(move |tracker| {
                let callee = if let Some(qualifier) = qualifier {
                    tracker.new_property_access_expression(
                        tracker.deep_clone_node(Some(qualifier)).unwrap(),
                        None,
                        tracker.new_identifier("scopedWith"),
                        NodeFlags::None,
                    )
                } else {
                    target.as_ref().unwrap().expression(tracker, &member)
                };
                let argument = if m.gen_function.is_some() {
                    new_scoped_with_gen_callback(tracker, &m)
                } else {
                    tracker.deep_clone_node(m.callback).unwrap()
                };
                let replacement = tracker.new_call_expression(
                    callee,
                    None,
                    None,
                    tracker.new_node_list(vec![argument]),
                    NodeFlags::None,
                );
                tracker.replace_piping_flow_prefix(
                    Some(sf),
                    Some(&m.flow),
                    m.prefix_length,
                    Some(replacement),
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}

// go: internal/fixables/scoped_immediate_scope_to_scoped_with.go:80 newScopedWithGenCallback a8fd7dc9e5abfb5c
// newScopedWithGenCallback builds `(<binding>) => <gen>(function*() { <rest> })`,
// dropping the leading Effect.scope retrieval from the generator body.
//
// Rust: Go's tracker only builds nodes here, so the helper takes the node factory.
fn new_scoped_with_gen_callback(
    factory: &NodeFactory,
    m: &ets_rules::ScopedImmediateScopeToScopedWithMatch,
) -> P<Node> {
    let gen_fn = m.gen_function.unwrap().as_function_expression();
    let statements = &gen_fn.body().unwrap().as_block().statements().nodes()[1..];
    let cloned = statements
        .iter()
        .map(|&statement| factory.deep_clone_node(Some(statement)).unwrap())
        .collect();
    let mut name = None;
    if let Some(gen_name) = gen_fn.name() {
        name = factory.deep_clone_node(Some(gen_name));
    }
    let generator = factory.new_function_expression(
        None,
        Some(factory.new_token(Kind::AsteriskToken)),
        name,
        None,
        Some(clone_scoped_with_node_list(factory, gen_fn.parameters())),
        None,
        None,
        Some(factory.new_block(factory.new_node_list(cloned), true)),
    );
    let gen_call = factory.new_call_expression(
        factory.deep_clone_node(m.gen_callee).unwrap(),
        None,
        None,
        factory.new_node_list(vec![generator]),
        NodeFlags::None,
    );
    let parameter = factory.new_parameter_declaration(
        None,
        None,
        factory.new_identifier(m.binding_name),
        None,
        None,
        None,
    );
    factory.new_arrow_function(
        None,
        None,
        Some(factory.new_node_list(vec![parameter])),
        None,
        None,
        Some(factory.new_token(Kind::EqualsGreaterThanToken)),
        Some(gen_call),
    )
}

// go: internal/fixables/scoped_immediate_scope_to_scoped_with.go:106 cloneScopedWithNodeList 015bf7b68acdef98
//
// Rust: the helper only builds nodes, so it takes the node factory.
fn clone_scoped_with_node_list(factory: &NodeFactory, list: Option<P<NodeList>>) -> P<NodeList> {
    let mut nodes = Vec::new();
    if let Some(list) = list {
        for &node in list.nodes() {
            nodes.push(factory.deep_clone_node(Some(node)).unwrap());
        }
    }
    factory.new_node_list(nodes)
}
