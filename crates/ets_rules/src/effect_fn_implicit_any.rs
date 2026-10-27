use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node};
use ets_shim::checker::{ContextFlags, SignatureKind};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectFnCallResult, TypeParser};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::PARAMETER_0_IMPLICITLY_HAS_TYPE_ANY_IN_EFFECT_FN_EFFECT_FN_UNTRACED_OR_EFFECT_FN_UNTRACED_EAGER_NO_PARAMETER_TYPE_IS_AVAILABLE_FROM_AN_EXPLICIT_ANNOTATION_OR_CONTEXTUAL_FUNCTION_TYPE_EFFECT_EFFECT_FN_IMPLICIT_ANY;

// go: internal/rules/effect_fn_implicit_any.go:16 EffectFnImplicitAny 9d347675f9888d18
// EffectFnImplicitAny mirrors TypeScript's noImplicitAny behavior for
// Effect.fn-family callbacks, which are otherwise
// contextually typed by the helper's internal any[] fallback.
pub static EFFECT_FN_IMPLICIT_ANY: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "effectFnImplicitAny",
    group: "correctness",
    description: "Mirrors noImplicitAny for unannotated Effect.fn, Effect.fnUntraced, and Effect.fnUntracedEager callback parameters when no outer contextual function type exists. Requires TS's noImplicitAny: true",
    default_severity: Severity::Error,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let options = ctx.program.options();
        if !options.get_strict_option_value(options.no_implicit_any) {
            return Vec::new();
        }

        let mut diags = Vec::new();

        // Rust: Go's recursive `walk` closure is a nested function.
        fn walk(ctx: &mut Context<'_, '_>, n: P<Node>, diags: &mut Vec<P<Diagnostic>>) -> bool {
            if let Some(result) = ctx.type_parser.effect_fn_call(n) {
                if result.is_generator() {
                    diags.extend(check_effect_fn_implicit_any(ctx, Some(&result)));
                } else {
                    diags.extend(check_effect_fn_implicit_any_body(
                        ctx,
                        result.call(),
                        Some(result.function_node()),
                    ));
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

// go: internal/rules/effect_fn_implicit_any.go:55 checkEffectFnImplicitAny 184400d9d3a65c50
pub(crate) fn check_effect_fn_implicit_any(
    ctx: &mut Context<'_, '_>,
    result: Option<&EffectFnCallResult>,
) -> Vec<P<Diagnostic>> {
    let Some(result) = result else {
        return Vec::new();
    };
    let Some(gen_fn) = result.generator_function() else {
        return Vec::new();
    };
    let Some(parameters) = gen_fn.as_function_expression().parameters() else {
        return Vec::new();
    };
    check_effect_fn_implicit_any_parameters(ctx, result.call(), parameters.nodes())
}

// go: internal/rules/effect_fn_implicit_any.go:63 checkEffectFnImplicitAnyBody 190fdd908947f763
pub(crate) fn check_effect_fn_implicit_any_body(
    ctx: &mut Context<'_, '_>,
    call_node: P<Node>,
    fn_node: Option<P<Node>>,
) -> Vec<P<Diagnostic>> {
    let Some(fn_node) = fn_node else {
        return Vec::new();
    };

    match fn_node.kind() {
        Kind::ArrowFunction => {
            let Some(parameters) = fn_node.as_arrow_function().parameters() else {
                return Vec::new();
            };
            check_effect_fn_implicit_any_parameters(ctx, call_node, parameters.nodes())
        }
        Kind::FunctionExpression => {
            let Some(parameters) = fn_node.as_function_expression().parameters() else {
                return Vec::new();
            };
            check_effect_fn_implicit_any_parameters(ctx, call_node, parameters.nodes())
        }
        _ => Vec::new(),
    }
}

// go: internal/rules/effect_fn_implicit_any.go:86 checkEffectFnImplicitAnyParameters b4f5df1d83e70309
pub(crate) fn check_effect_fn_implicit_any_parameters(
    ctx: &mut Context<'_, '_>,
    call_node: P<Node>,
    params: &[P<Node>],
) -> Vec<P<Diagnostic>> {
    if has_outer_contextual_function_type(ctx.type_parser, Some(call_node)) {
        return Vec::new();
    }

    let mut diags = Vec::new();
    for &param in params {
        if param.type_node().is_some() || param.initializer().is_some() {
            continue;
        }

        let mut name = ets_shim::scanner::declaration_name_to_string(param.name());
        if name.is_empty() {
            name = "parameter".to_string();
        }

        let name_node = param.name().expect("a parameter has a name");
        diags.push(ctx.new_diagnostic(
            ctx.source_file,
            ctx.get_error_range(name_node),
            MESSAGE,
            &[],
            &[&name],
        ));
    }

    diags
}

// go: internal/rules/effect_fn_implicit_any.go:114 hasOuterContextualFunctionType 70112d73265631e9
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub(crate) fn has_outer_contextual_function_type(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
) -> bool {
    let Some(node) = node else {
        return false;
    };
    if !ast::is_expression(node) {
        return false;
    }

    let Some(contextual_type) = tp.get_contextual_type(node, ContextFlags::None) else {
        return false;
    };

    for member in tp.unroll_union_members(contextual_type).unwrap_or_default() {
        if !tp.checker().get_signatures_of_type_exported(member, SignatureKind::Call).is_empty() {
            return true;
        }
    }

    false
}
