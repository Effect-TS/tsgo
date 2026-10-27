use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rules::MapMapErrorToMapBothMatch;
use ets_shim::P;
use ets_shim::ast::{self, Kind, Node, NodeFlags, SymbolFlags};
use ets_shim::core::alloc_str;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_0_EFFECT_1_TRANSFORM_THE_SUCCESS_AND_FAILURE_CHANNELS_ONE_STEP_AT_A_TIME_EFFECT_2_EXPRESSES_BOTH_TRANSFORMATIONS_IN_A_SINGLE_STEP_EFFECT_MAP_MAP_ERROR_TO_MAP_BOTH;

// go: internal/fixables/map_map_error_to_map_both.go:17 MapMapErrorToMapBothFix
pub static MAP_MAP_ERROR_TO_MAP_BOTH_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "mapMapErrorToMapBoth",
    description: "Replace with Effect.mapBoth",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["mapMapErrorToMapBoth_fix"],
    run: run_map_map_error_to_map_both_fix,
});

// go: internal/fixables/map_map_error_to_map_both.go:27 runMapMapErrorToMapBothFix
fn run_map_map_error_to_map_both_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_map_map_error_to_map_both(&mut ctx.type_parser, Some(source_file)) {
        if !m.fixable || (!m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location)) {
            continue;
        }
        let (target, receiver) = map_both_reference(ctx, &m);
        let Some(target) = target else {
            continue;
        };
        if let Some(action) = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.".to_string() + m.replacement_name,
            run: Box::new(move |t| {
                let callee = if receiver {
                    effect_module_method(t, m.source_file, Some(target), m.replacement_name)
                } else {
                    t.new_identifier(alloc_str(target.text()))
                };
                // Keep the callbacks in flow order so they are evaluated as before.
                let (mut first_name, mut second_name) = ("onSuccess", "onFailure");
                if m.first_failure {
                    (first_name, second_name) = (second_name, first_name);
                }
                let options = t.new_object_literal_expression(
                    t.new_node_list(vec![
                        t.new_property_assignment(
                            None,
                            t.new_identifier(first_name),
                            None,
                            None,
                            t.deep_clone_node(Some(*m.first.args().at(0))).unwrap(),
                        ),
                        t.new_property_assignment(
                            None,
                            t.new_identifier(second_name),
                            None,
                            None,
                            t.deep_clone_node(Some(*m.second.args().at(0))).unwrap(),
                        ),
                    ]),
                    true,
                );
                // The first step is the second step's input, so removing it never
                // overlaps the second step's callee or callback.
                if !remove_piping_flow_transformation(t, Some(m.source_file), Some(&m.first)) {
                    return;
                }
                t.replace_node(m.source_file, Some(m.second.callee()), Some(callee), None);
                t.replace_node(m.source_file, Some(*m.second.args().at(0)), Some(options), None);
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}

// go: internal/fixables/map_map_error_to_map_both.go:74 mapBothReference
// mapBothReference finds a value that reaches the replacement API at the
// diagnostic: the Effect module receiver of one of the steps, or a binding in
// scope that is the replacement API itself (receiver false) or the Effect
// module (receiver true). It returns nil when the file has no such binding,
// for example when the steps only use named imports of map and mapError.
//
// Rust: `GetSymbolsInScope` returns one symbol per name (the checker collects them in a symbol table keyed by
// name), so the keys of Go's unstable sort are unique and a stable sort orders them the same way.
fn map_both_reference(
    ctx: &mut Context<'_, '_>,
    m: &MapMapErrorToMapBothMatch,
) -> (Option<P<Node>>, bool) {
    let method = m.replacement_name;
    for callee in [m.second.callee(), m.first.callee()] {
        if callee.kind() != Kind::PropertyAccessExpression {
            continue;
        }
        let receiver = callee.as_property_access_expression().expression();
        if ctx.type_parser.is_expression_effect_module(receiver)
            && effect_module_has_method(ctx, receiver, method)
        {
            return (Some(receiver), true);
        }
    }
    let location = m.second.callee();
    let mut symbols = ctx
        .checker()
        .get_symbols_in_scope_exported(location, SymbolFlags::Value | SymbolFlags::Alias);
    symbols.sort_by(|left, right| left.name().cmp(right.name()));
    // Among module bindings, prefer the one the file imports Effect under.
    let preferred = ets_typeparser::find_effect_module_identifier(m.source_file);
    let mut module: Option<P<Node>> = None;
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
        let mut reference = name;
        if declaration.kind() == Kind::VariableDeclaration {
            let Some(parent) = declaration.parent() else {
                continue;
            };
            if !parent.flags().contains(NodeFlags::Const)
                || declaration.end().expect_source("a declaration")
                    > location.pos().expect_source("a callee")
            {
                continue;
            }
            let Some(initializer) = declaration.as_variable_declaration().initializer() else {
                continue;
            };
            reference = initializer;
            if ctx.type_parser.get_type_at_location(name)
                != ctx.type_parser.get_type_at_location(reference)
            {
                continue;
            }
        }
        // Prefer a direct binding to the replacement API over a module binding.
        if ctx.type_parser.is_node_reference_to_effect_module_api(reference, method) {
            return (Some(name), false);
        }
        if (module.is_none() || name.text() == preferred)
            && ctx.type_parser.is_expression_effect_module(reference)
            && effect_module_has_method(ctx, name, method)
        {
            module = Some(name);
        }
    }
    if module.is_some() {
        return (module, true);
    }
    (None, false)
}

// go: internal/fixables/map_map_error_to_map_both.go:129 effectModuleHasMethod
//
// Rust: Go reads a property of the receiver's type, which `GetTypeAtLocation` always finds for these expressions;
// a missing type is no such property.
fn effect_module_has_method(ctx: &mut Context<'_, '_>, receiver: P<Node>, method: &str) -> bool {
    let Some(receiver_type) = ctx.type_parser.get_type_at_location(receiver) else {
        return false;
    };
    let property = ctx.checker().get_property_of_type_exported(receiver_type, method);
    let Some(property) = property else {
        return false;
    };
    let Some(value_declaration) = property.value_declaration() else {
        return false;
    };
    let Some(name) = value_declaration.name() else {
        return false;
    };
    ctx.type_parser.is_node_reference_to_effect_module_api(name, method)
}
