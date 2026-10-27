use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THE_ADAPTER_OF_EFFECT_GEN_IS_NOT_REQUIRED_ANYMORE_IT_IS_NOW_JUST_AN_ALIAS_OF_PIPE_EFFECT_EFFECT_GEN_USES_ADAPTER;

// go: internal/rules/effect_gen_uses_adapter.go:11 EffectGenUsesAdapter faa306c834d412c4
// EffectGenUsesAdapter warns when using the deprecated adapter parameter in Effect.gen.
pub static EFFECT_GEN_USES_ADAPTER: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "effectGenUsesAdapter",
    group: "antipattern",
    description: "Warns when using the deprecated adapter parameter in Effect.gen",
    default_severity: Severity::Warning,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();

        // Rust: Go's recursive `walk` closure is a nested function.
        fn walk(ctx: &mut Context<'_, '_>, n: P<Node>, diags: &mut Vec<P<Diagnostic>>) -> bool {
            if n.kind() == Kind::CallExpression {
                if let Some(diag) = check_effect_gen_uses_adapter(ctx, n) {
                    diags.push(diag);
                }
            }

            n.for_each_child(&mut |child| walk(ctx, child, diags));
            false
        }

        let root = ctx.source_file.as_node();
        walk(ctx, root, &mut diags);

        diags
    },
});

// go: internal/rules/effect_gen_uses_adapter.go:43 checkEffectGenUsesAdapter 9cf27c302150ce68
pub(crate) fn check_effect_gen_uses_adapter(
    ctx: &mut Context<'_, '_>,
    n: P<Node>,
) -> Option<P<Diagnostic>> {
    let gen_result = ctx.type_parser.effect_gen_call(n)?;

    let parameters = gen_result.generator_function().as_function_expression().parameters();
    let parameters = parameters.map(|p| p.nodes()).unwrap_or_default();
    if parameters.is_empty() {
        return None;
    }

    let adapter_param = parameters[0];
    Some(ctx.new_diagnostic(ctx.source_file, ctx.get_error_range(adapter_param), MESSAGE, &[], &[]))
}
