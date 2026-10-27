use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::P;
use ets_shim::ast::{self, Kind, Node, NodeFlags, SourceFile, SymbolFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_typeparser::ModuleReference;

use crate::*;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::EFFECT_TAP_EXPRESSES_THIS_OBSERVATION_MORE_DIRECTLY_THAN_EFFECT_0_WITH_A_MANUAL_RE_YIELD_EFFECT_FLAT_MAP_TO_TAP;

// go: internal/fixables/flat_map_to_tap.go:17 FlatMapToTapFix e443e42f95da863c
pub static FLAT_MAP_TO_TAP_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "flatMapToTap",
    description: "Replace Effect.flatMap or Effect.andThen with Effect.tap",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["flatMapToTap_fix"],
    run: run_flat_map_to_tap_fix,
});

// go: internal/fixables/flat_map_to_tap.go:27 runFlatMapToTapFix 26efbe97982ed978
fn run_flat_map_to_tap_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_flat_map_to_tap(&mut ctx.type_parser, source_file) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        let tap = effect_module_member("tap");
        let target = tap_reference(ctx, m.source_file, m.callee, &tap);
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
                let replacement = target.expression(tracker, &tap);
                tracker.replace_node(m.source_file, Some(m.callee), Some(replacement), None);
            }),
        }) {
            return vec![action];
        }
    }
    Vec::new()
}

// go: internal/fixables/flat_map_to_tap.go:54 tapReference dd803e40a2b20d63
// tapReference finds a reference that reaches Effect.tap at callee: the file's
// Effect import, or a binding in scope that is tap itself or the Effect module.
//
// Rust: `GetSymbolsInScope` returns one symbol per name (the checker collects them in a symbol table keyed by
// name), so the keys of Go's unstable sort are unique and a stable sort orders them the same way.
fn tap_reference(
    ctx: &mut Context<'_, '_>,
    sf: P<SourceFile>,
    callee: P<Node>,
    tap: &ModuleMember,
) -> Option<ModuleMemberReference> {
    if let Some(reference) = imported_module_member(
        ctx,
        callee,
        &ets_typeparser::find_effect_module_reference(Some(sf)),
        tap,
    ) {
        return Some(reference);
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
        // Rust: Go passes a nil initializer on; both predicates reject it.
        let Some(reference) = reference else {
            continue;
        };
        if (tap.is_member)(&mut ctx.type_parser, reference) {
            return Some(ModuleMemberReference {
                path: ModuleReference(vec![name.text().to_string()]),
                via_module: false,
            });
        }
        if (tap.is_module)(&mut ctx.type_parser, reference) {
            let name_type = ctx.type_parser.get_type_at_location(name);
            if type_has_member(ctx, name_type, tap) {
                return Some(ModuleMemberReference {
                    path: ModuleReference(vec![name.text().to_string()]),
                    via_module: true,
                });
            }
        }
    }
    None
}
