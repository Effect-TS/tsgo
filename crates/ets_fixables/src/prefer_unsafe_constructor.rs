use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_shim::ast::Kind;
use ets_shim::core::{TextRange, alloc_str};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_shim::scanner;

/// Rust-only: the fixable's diagnostic message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::X_0_starts_a_fiber_to_run_a_pure_constructor_Use_the_synchronous_variant_1_instead_effect_preferUnsafeConstructor;

// go: internal/fixables/prefer_unsafe_constructor.go:14 PreferUnsafeConstructorFix
pub static PREFER_UNSAFE_CONSTRUCTOR_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "preferUnsafeConstructor",
    description: "Replace Effect.runSync of a pure constructor with the synchronous *Unsafe variant",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["preferUnsafeConstructor_fix"],
    run: run_prefer_unsafe_constructor_fix,
});

// go: internal/fixables/prefer_unsafe_constructor.go:24 runPreferUnsafeConstructorFix
fn run_prefer_unsafe_constructor_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let sf = ctx.source_file;

    for m in ets_rules::analyze_prefer_unsafe_constructor(&mut ctx.type_parser, sf) {
        if !m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location) {
            continue;
        }
        // A bare identifier callee (e.g. `import { make } from "effect/Scope"`) has no
        // local binding for the sibling, so renaming it would leave an unbound identifier.
        if m.inner_callee.kind() != Kind::PropertyAccessExpression {
            continue;
        }

        let description = "Replace with ".to_string() + &m.unsafe_callee_text;

        if let Some(action) = ctx.new_fix_action(FixAction {
            description,
            run: Box::new(move |tracker| {
                let outer_start = scanner::get_token_pos_of_node(m.outer_call, sf, false);
                let inner_start = scanner::get_token_pos_of_node(m.inner_call, sf, false);
                tracker.delete_range(sf, TextRange::new(outer_start, inner_start));
                let identifier = tracker.new_identifier(alloc_str(&m.unsafe_name));
                tracker.replace_node(sf, Some(m.inner_callee_name), Some(identifier), None);
                tracker.delete_range(
                    sf,
                    TextRange::from_text_pos(m.inner_call.end(), m.outer_call.end()),
                );
            }),
        }) {
            return vec![action];
        }
    }

    Vec::new()
}
