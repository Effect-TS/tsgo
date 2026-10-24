use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::P;
use ets_shim::ast::{self, Kind, Node, NodeFlags, SymbolFlags};
use ets_shim::core::alloc_str;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::Effect_tap_expresses_this_observation_more_directly_than_Effect_0_with_a_manual_re_yield_effect_flatMapToTap;

// go: internal/fixables/flat_map_to_tap.go:16 FlatMapToTapFix
pub static FLAT_MAP_TO_TAP_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "flatMapToTap",
    description: "Replace Effect.flatMap or Effect.andThen with Effect.tap",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["flatMapToTap_fix"],
    run: run_flat_map_to_tap_fix,
});

// go: internal/fixables/flat_map_to_tap.go:26 runFlatMapToTapFix
fn run_flat_map_to_tap_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_flat_map_to_tap(&mut ctx.type_parser, source_file) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        let (target, receiver) = tap_reference(ctx, m.callee);
        let Some(target) = target else {
            continue;
        };
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.tap".to_string(),
            run: Box::new(move |tracker| {
                if !tracker.remove_trailing_piping_flow_transformation(
                    Some(m.source_file),
                    Some(&m.return_flow),
                ) {
                    return;
                }
                if m.callee.kind() == Kind::PropertyAccessExpression
                    && target == m.callee.as_property_access_expression().expression()
                {
                    let replacement = tracker.new_identifier("tap");
                    tracker.replace_node(
                        m.source_file,
                        Some(m.callee.as_property_access_expression().name()),
                        Some(replacement),
                        None,
                    );
                    return;
                }
                let replacement = if receiver {
                    effect_module_method(tracker, m.source_file, Some(target), "tap")
                } else {
                    tracker.new_identifier(alloc_str(target.text()))
                };
                tracker.replace_node(m.source_file, Some(m.callee), Some(replacement), None);
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}

// go: internal/fixables/flat_map_to_tap.go:60 tapReference
//
// Rust: `GetSymbolsInScope` returns one symbol per name (the checker collects them in a symbol table keyed by
// name), so the keys of Go's unstable sort are unique and a stable sort orders them the same way.
fn tap_reference(ctx: &mut Context<'_, '_>, callee: P<Node>) -> (Option<P<Node>>, bool) {
    if callee.kind() == Kind::PropertyAccessExpression {
        let receiver = callee.as_property_access_expression().expression();
        if ctx.type_parser.is_expression_effect_module(receiver)
            && tap_module_reference(ctx, receiver)
        {
            return (Some(receiver), true);
        }
    }
    let mut symbols = ctx
        .checker()
        .get_symbols_in_scope_exported(callee, SymbolFlags::Value | SymbolFlags::Alias);
    symbols.sort_by(|left, right| left.name().cmp(right.name()));
    for symbol in symbols {
        if symbol.declarations().len() != 1 || ctx.checker().is_symbol_assigned(symbol) {
            continue;
        }
        let declaration = symbol.declarations()[0];
        if ast::is_part_of_type_only_import_or_export_declaration(declaration) {
            continue;
        }
        let Some(name) = declaration.name() else {
            continue;
        };
        if name.kind() != Kind::Identifier {
            continue;
        }
        let mut reference = Some(name);
        if declaration.kind() == Kind::VariableDeclaration {
            let Some(parent) = declaration.parent() else {
                continue;
            };
            if !parent.flags().contains(NodeFlags::Const)
                || declaration.end().expect_source("a declaration")
                    > callee.pos().expect_source("a callee")
            {
                continue;
            }
            reference = declaration.as_variable_declaration().initializer();
            if ctx.type_parser.get_type_at_location(name)
                != ctx.type_parser.get_type_at_location(reference)
            {
                continue;
            }
        }
        // Rust: Go passes a nil initializer on; both callees reject it.
        let Some(reference) = reference else {
            continue;
        };
        if ctx.type_parser.is_node_reference_to_effect_module_api(reference, "tap") {
            return (Some(name), false);
        }
        if ctx.type_parser.is_expression_effect_module(reference) && tap_module_reference(ctx, name)
        {
            return (Some(name), true);
        }
    }
    (None, false)
}

// go: internal/fixables/flat_map_to_tap.go:103 tapModuleReference
//
// Rust: Go reads a property of the receiver's type, which `GetTypeAtLocation` always finds for these expressions;
// a missing type is no `tap` property.
fn tap_module_reference(ctx: &mut Context<'_, '_>, receiver: P<Node>) -> bool {
    let Some(receiver_type) = ctx.type_parser.get_type_at_location(receiver) else {
        return false;
    };
    let tap = ctx.checker().get_property_of_type_exported(receiver_type, "tap");
    let Some(tap) = tap else {
        return false;
    };
    let Some(value_declaration) = tap.value_declaration() else {
        return false;
    };
    let Some(name) = value_declaration.name() else {
        return false;
    };
    ctx.type_parser.is_node_reference_to_effect_module_api(name, "tap")
}
