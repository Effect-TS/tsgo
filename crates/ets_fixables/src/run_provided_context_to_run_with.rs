use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::PipingFlowTransformationReplacement;
use ets_rules::RunProvidedContextToRunWithMatch;
use ets_shim::P;
use ets_shim::ast::{self, Kind, Node, NodeFlags, OuterExpressionKinds};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::TransformationKind;

use crate::*;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_0_OF_EFFECT_1_RE_IMPLEMENTS_EFFECT_2_USE_EFFECT_2_CONTEXT_EFFECT_TO_RUN_THE_EFFECT_WITH_THE_CONTEXT_DIRECTLY_EFFECT_RUN_PROVIDED_CONTEXT_TO_RUN_WITH;

// go: internal/fixables/run_provided_context_to_run_with.go:16 RunProvidedContextToRunWithFix dbe89eb1af54a92f
pub static RUN_PROVIDED_CONTEXT_TO_RUN_WITH_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "runProvidedContextToRunWith",
    description: "Replace with the dedicated Effect run*With runner",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["runProvidedContextToRunWith_fix"],
    run: run_run_provided_context_to_run_with_fix,
});

// go: internal/fixables/run_provided_context_to_run_with.go:26 runRunProvidedContextToRunWithFix 202902ba10bff9c0
fn run_run_provided_context_to_run_with_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;
    for m in ets_rules::analyze_run_provided_context_to_run_with(&mut ctx.type_parser, Some(sf)) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        // Explicit runner type arguments have no verified rewrite: report only.
        if let Some(type_arguments) = m.runner.type_arguments()
            && !type_arguments.nodes().is_empty()
        {
            continue;
        }

        // run*With(context)(effect) evaluates the context before the effect;
        // only reorder when the context cannot observe or affect that work.
        if run_provided_context_reorders_context(&m) && !order_insensitive_context(ctx, m.context) {
            continue;
        }
        // Only offer the fix when the generated reference resolves at the call site.
        let replacement = effect_module_member(m.replacement_name);
        let mut reference = None;
        if m.effect_module_node.is_none() {
            reference = resolve_module_member(
                ctx,
                m.runner_callee,
                &ets_typeparser::find_effect_module_reference(Some(sf)),
                &replacement,
            );
            if reference.is_none() {
                continue;
            }
        }

        let description = "Replace with Effect.".to_string() + m.replacement_name;
        if let Some(action) = ctx.new_fix_action(FixAction {
            description,
            run: Box::new(move |tracker| {
                let callee = if let Some(effect_module_node) = m.effect_module_node {
                    tracker.new_property_access_expression(
                        tracker.deep_clone_node(Some(effect_module_node)).unwrap(),
                        None,
                        tracker.new_identifier(m.replacement_name),
                        NodeFlags::None,
                    )
                } else {
                    reference
                        .as_ref()
                        .expect("resolved when the runner has no Effect module receiver")
                        .expression(tracker, &replacement)
                };
                // The provide step and the runner collapse into run*With(context),
                // applied to the provide step's input with the runner's options.
                let arguments =
                    tracker.new_node_list(vec![tracker.deep_clone_node(Some(m.context)).unwrap()]);
                tracker.collapse_piping_flow_transformations(
                    Some(sf),
                    Some(&m.flow),
                    m.provide_index,
                    m.provide_index + 1,
                    PipingFlowTransformationReplacement {
                        callee: Some(callee),
                        arguments: Some(arguments),
                        ..Default::default()
                    },
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}

// go: internal/fixables/run_provided_context_to_run_with.go:78 runProvidedContextReordersContext 6b8ac2d49ccc66fe
// runProvidedContextReordersContext reports whether the fix moves the context
// ahead of the effect expression. A called runner originally evaluates its whole
// effect argument, the context last unless the provide step is the curried
// provideContext(context)(effect); a runner pipe step keeps the source order.
fn run_provided_context_reorders_context(m: &RunProvidedContextToRunWithMatch) -> bool {
    match m.runner.kind() {
        TransformationKind::Call | TransformationKind::CallWithOptionals => {
            m.provide.kind() != TransformationKind::Call
        }
        _ => false,
    }
}

// go: internal/fixables/run_provided_context_to_run_with.go:88 runProvidedContextConstructors d4d8e34593107f93
// runProvidedContextConstructors are the Context constructors that only build a
// new Context from their arguments.
static RUN_PROVIDED_CONTEXT_CONSTRUCTORS: &[&str] = &["empty", "make", "add", "merge", "mergeAll"];

// go: internal/fixables/run_provided_context_to_run_with.go:101 orderInsensitiveContext a46f6c2f9774c2fe
// orderInsensitiveContext reports whether evaluating node earlier yields the
// same value and has no effect other code could observe: string literals, const
// bindings, parameters that are never reassigned, and object literals and
// Context constructor calls built only from those.
//
// A binding is only a stable reference: its contents may still change, which
// is harmless where the reference itself is provided, because both forms read
// the Context when the effect runs. A constructor call reads the contents of
// its Context arguments when it is evaluated, so those must be fresh
// constructor calls. Tags only contribute their key and service values are
// stored by reference, so stable bindings are fine there.
//
// Rust: tsrs's `skip_alias` never returns nil, so Go's nil check on the target drops.
fn order_insensitive_context(ctx: &mut Context<'_, '_>, node: P<Node>) -> bool {
    let node = ast::skip_outer_expressions(node, OuterExpressionKinds::All);
    match node.kind() {
        Kind::StringLiteral => true,
        Kind::Identifier => {
            let Some(symbol) = ctx.type_parser.get_symbol_at_location(node) else {
                return false;
            };
            // The value declaration, so a const merged with an interface still counts.
            let target = ets_shim::checker::skip_alias(symbol, ctx.checker());
            let Some(declaration) = target.value_declaration() else {
                return false;
            };
            match declaration.kind() {
                Kind::VariableDeclaration => ast::is_var_const(declaration),
                Kind::Parameter => !ctx.checker().is_symbol_assigned(target),
                _ => false,
            }
        }
        Kind::ObjectLiteralExpression => {
            for &property in node.as_object_literal_expression().properties().nodes() {
                if property.kind() != Kind::PropertyAssignment
                    || property.name().expect("a property assignment has a name").kind()
                        == Kind::ComputedPropertyName
                    || !order_insensitive_context(
                        ctx,
                        property.as_property_assignment().initializer(),
                    )
                {
                    return false;
                }
            }
            true
        }
        Kind::CallExpression => {
            let call = node.as_call_expression();
            if !RUN_PROVIDED_CONTEXT_CONSTRUCTORS.iter().any(|name| {
                ctx.type_parser
                    .is_node_reference_to_effect_context_module_api(call.expression(), name)
            }) {
                return false;
            }
            for &argument in call.arguments().nodes() {
                if ctx.type_parser.context_type_at_location(argument).is_some()
                    && ast::skip_outer_expressions(argument, OuterExpressionKinds::All).kind()
                        != Kind::CallExpression
                {
                    return false;
                }
                if !order_insensitive_context(ctx, argument) {
                    return false;
                }
            }
            true
        }
        _ => false,
    }
}
