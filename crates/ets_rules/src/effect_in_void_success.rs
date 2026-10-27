// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::Diagnostic;
use ets_shim::checker::{Type, TypeFlags};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THERE_IS_A_NESTED_0_IN_THE_VOID_SUCCESS_CHANNEL_BEWARE_THAT_THIS_COULD_LEAD_TO_NESTED_EFFECT_EFFECT_THAT_WON_T_BE_EXECUTED_EFFECT_EFFECT_IN_VOID_SUCCESS;

// go: internal/rules/effect_in_void_success.go:16 EffectInVoidSuccess 5a36f821c246dcd7
// EffectInVoidSuccess detects nested Effects in void success channels.
// When an Effect has void as its success type but the actual value contains
// an Effect type, this likely means a nested Effect<Effect<...>> that won't be executed.
pub static EFFECT_IN_VOID_SUCCESS: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "effectInVoidSuccess",
    group: "antipattern",
    description: "Detects nested Effects in void success channels that may cause unexecuted effects",
    default_severity: Severity::Warning,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();

        let source_file = ctx.source_file;
        let entries = ctx.type_parser.expected_and_real_types(source_file);
        for (_, entry) in ets_readonly::new(entries.as_deref()).all() {
            // The real type query re-checks the expression, so rule out values
            // that cannot be an Effect from their declared type first.
            if !ctx.type_parser.node_could_be_effect(entry.real_node()) {
                continue;
            }

            // The real type goes first: parsing a deeply recursive expected type
            // can raise TS2589 (#381), and the expected type stays unresolved
            // unless the real type is an Effect.
            let real_type = entry.real_type(ctx.type_parser);
            let Some(real_effect) = ctx.type_parser.effect_type(real_type) else {
                continue;
            };

            let expected_type = entry.expected_type(ctx.type_parser);
            if expected_type == entry.real_type(ctx.type_parser) {
                continue;
            }

            let Some(expected_effect) = ctx.type_parser.effect_type(expected_type) else {
                continue;
            };

            // Check if the expected Effect's success type is void
            if !expected_effect.a().flags().intersects(TypeFlags::Void) {
                continue;
            }

            // Unroll the real Effect's success type into union members
            // and check if any member is strictly an Effect type
            let members = ctx.type_parser.unroll_union_members(real_effect.a()).unwrap_or_default();
            let voided_effect = find_first_strict_effect(ctx.type_parser, &members);
            if let Some(voided_effect) = voided_effect {
                let type_string = ctx.checker().type_to_string_exported(voided_effect);
                let diag = ctx.new_diagnostic(
                    ctx.source_file,
                    ctx.get_error_range(entry.node()),
                    MESSAGE,
                    &[],
                    &[&type_string],
                );
                diags.push(diag);
            }
        }

        diags
    },
});

// go: internal/rules/effect_in_void_success.go:71 findFirstStrictEffect 34adecb62e659184
// findFirstStrictEffect returns the first type in the slice that is strictly an Effect type,
// or nil if none are found. This mirrors the Nano.firstSuccessOf pattern in the TS reference.
//
// Rust: Go's checker argument is unused and dropped.
#[expect(clippy::manual_find, reason = "Go's loop, kept as written")]
pub(crate) fn find_first_strict_effect(
    tp: &mut TypeParser<'_>,
    types: &[P<Type>],
) -> Option<P<Type>> {
    for &t in types {
        if tp.strict_is_effect_type(t) {
            return Some(t);
        }
    }
    None
}
