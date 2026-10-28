use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, ModifierFlags, Node};
use ets_shim::diagnostics::{self as tsdiag, Message};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_CODE_DECLARES_AN_ASYNC_FUNCTION_CONSIDER_REPRESENTING_THIS_ASYNC_CONTROL_FLOW_WITH_EFFECT_VALUES_AND_EFFECT_GEN_EFFECT_ASYNC_FUNCTION;

// go: internal/rules/async_function.go:10 AsyncFunction 412c4b4d2cfa3431
pub static ASYNC_FUNCTION: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "asyncFunction",
    group: "effectNative",
    description: "Warns when declaring async functions and suggests using Effect values and Effect.gen for async control flow",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();

        // Rust: Go's recursive `walk` closure is a nested function.
        fn walk(ctx: &Context<'_, '_>, node: P<Node>, diags: &mut Vec<P<Diagnostic>>) -> bool {
            match node.kind() {
                Kind::FunctionDeclaration
                | Kind::FunctionExpression
                | Kind::ArrowFunction
                | Kind::MethodDeclaration
                    if ast::get_combined_modifier_flags(node).intersects(ModifierFlags::Async) =>
                {
                    diags.push(ctx.new_diagnostic(
                        ctx.source_file,
                        ctx.get_error_range(node),
                        MESSAGE,
                        &[],
                        &[],
                    ));
                }
                _ => {}
            }

            node.for_each_child(&mut |child| walk(ctx, child, diags));
            false
        }

        walk(ctx, ctx.source_file.as_node(), &mut diags);
        diags
    },
});
