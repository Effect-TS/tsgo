use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::EffectContextFlags;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_CODE_USES_CRYPTO_RANDOM_UUID_PREFER_THE_EFFECT_CRYPTO_MODULE_INSTEAD_EFFECT_CRYPTO_RANDOM_UUID;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE_IN_EFFECT: &Message = &tsdiag::THIS_EFFECT_CODE_USES_CRYPTO_RANDOM_UUID_PREFER_THE_EFFECT_CRYPTO_MODULE_INSTEAD_EFFECT_CRYPTO_RANDOM_UUID_IN_EFFECT;

// go: internal/rules/crypto_random_uuid.go:12 CryptoRandomUUID 062373540fb780b3
pub static CRYPTO_RANDOM_UUID: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "cryptoRandomUUID",
    group: "effectNative",
    description: "Warns when using crypto.randomUUID() outside Effect generators instead of the Effect Crypto module",
    default_severity: Severity::Off,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> { run_crypto_random_uuid(ctx, false) },
});

// go: internal/rules/crypto_random_uuid.go:24 CryptoRandomUUIDInEffect c2875ac105c75437
pub static CRYPTO_RANDOM_UUID_IN_EFFECT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "cryptoRandomUUIDInEffect",
    group: "effectNative",
    description: "Warns when using crypto.randomUUID() inside Effect generators instead of the Effect Crypto module",
    default_severity: Severity::Off,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE_IN_EFFECT.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> { run_crypto_random_uuid(ctx, true) },
});

// go: internal/rules/crypto_random_uuid.go:36 runCryptoRandomUUID b09d4c170c6e8c8a
pub(crate) fn run_crypto_random_uuid(
    ctx: &mut Context<'_, '_>,
    check_in_effect: bool,
) -> Vec<P<Diagnostic>> {
    let mut message: &'static Message = MESSAGE;
    if check_in_effect {
        message = MESSAGE_IN_EFFECT;
    }

    let mut diags = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        ctx: &mut Context<'_, '_>,
        node: P<Node>,
        check_in_effect: bool,
        message: &'static Message,
        diags: &mut Vec<P<Diagnostic>>,
    ) -> bool {
        if node.kind() == Kind::CallExpression {
            let call = node.as_call_expression();
            let in_effect = ctx
                .type_parser
                .get_effect_context_flags(node)
                .intersects(EffectContextFlags::InEffect);
            if in_effect == check_in_effect
                && ctx.type_parser.is_node_reference_to_global_member(
                    call.expression(),
                    "crypto",
                    "randomUUID",
                )
            {
                diags.push(ctx.new_diagnostic(
                    ctx.source_file,
                    ets_shim::scanner::get_error_range_for_node(ctx.source_file, node),
                    message,
                    &[],
                    &[],
                ));
            }
        }

        node.for_each_child(&mut |child| walk(ctx, child, check_in_effect, message, diags));
        false
    }

    let root = ctx.source_file.as_node();
    walk(ctx, root, check_in_effect, message, &mut diags);

    diags
}
