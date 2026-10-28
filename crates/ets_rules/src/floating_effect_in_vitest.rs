use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node};
use ets_shim::checker::{SignatureKind, Type};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{LazyExpressionFlags, TypeParser};

// go: internal/rules/floating_effect_in_vitest.go:12 vitestCallbackKind 2c8ba8af9b8c533e
// go: internal/rules/floating_effect_in_vitest.go:14 vitestCallbackNone c56122ecf7d11e84
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum VitestCallbackKind {
    None,
    Test,
    Hook,
}

// go: internal/rules/floating_effect_in_vitest.go:20 vitestRoot b298416e671ed4f6
#[derive(Clone, Copy, Default)]
pub(crate) struct VitestRoot {
    pub(crate) name: &'static str,
    pub(crate) effect_vitest: bool,
}

// go: internal/rules/floating_effect_in_vitest.go:25 vitestTestModifiers 29300f7b211a1658
// Rust: Go's `map[string]struct{}` sets are slices searched with `contains`.
pub(crate) static VITEST_TEST_MODIFIERS: &[&str] = &[
    "concurrent",
    "each",
    "fails",
    "for",
    "only",
    "prop",
    "runIf",
    "sequential",
    "skip",
    "skipIf",
    "todo",
];

// go: internal/rules/floating_effect_in_vitest.go:39 vitestHookNames a65c68c83e41e6d5
// Rust: Go iterates this set in random order in `matchVitestRoot`; a node references at most one of the hooks, so
// the order is not observable.
pub(crate) static VITEST_HOOK_NAMES: &[&str] =
    &["afterAll", "afterEach", "beforeAll", "beforeEach"];

// go: internal/rules/floating_effect_in_vitest.go:46 effectAwareVitestMethods d7c05723027ebdaf
pub(crate) static EFFECT_AWARE_VITEST_METHODS: &[&str] =
    &["effect", "live", "scoped", "scopedLive"];

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_VITEST_CALLBACK_RETURNS_AN_EFFECT_THAT_VITEST_DOES_NOT_RUN_USE_AN_EFFECT_AWARE_TEST_API_OR_RUN_THE_EFFECT_WITH_EFFECT_RUN_PROMISE_EFFECT_FLOATING_EFFECT_IN_VITEST;

// go: internal/rules/floating_effect_in_vitest.go:55 FloatingEffectInVitest a97f36d1f01a1274
// FloatingEffectInVitest detects Effects returned from Vitest callbacks that do
// not execute returned Effect values.
pub static FLOATING_EFFECT_IN_VITEST: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "floatingEffectInVitest",
    group: "correctness",
    description: "Detects Effects returned from non-Effect-aware Vitest callbacks",
    default_severity: Severity::Error,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diags = Vec::new();

        // Rust: Go's recursive `walk` closure is a nested function.
        fn walk(ctx: &mut Context<'_, '_>, node: P<Node>, diags: &mut Vec<P<Diagnostic>>) -> bool {
            if node.kind() == Kind::CallExpression
                && let Some(callback) = floating_effect_vitest_callback(ctx, node)
            {
                let mut range_node = callback;
                if let Some(lazy) =
                    ets_typeparser::parse_lazy_expression(callback, LazyExpressionFlags::AllowAsync)
                {
                    range_node = lazy.expression();
                }
                diags.push(ctx.new_diagnostic(
                    ctx.source_file,
                    ctx.get_error_range(range_node),
                    MESSAGE,
                    &[],
                    &[],
                ));
            }

            node.for_each_child(&mut |child| walk(ctx, child, diags));
            false
        }

        walk(ctx, ctx.source_file.as_node(), &mut diags);
        diags
    },
});

// go: internal/rules/floating_effect_in_vitest.go:96 floatingEffectVitestCallback 8161dadb33315aa1
//
// Rust: takes the call expression node (Go's `*ast.CallExpression`).
pub(crate) fn floating_effect_vitest_callback(
    ctx: &mut Context<'_, '_>,
    call_node: P<Node>,
) -> Option<P<Node>> {
    let call = call_node.as_call_expression();

    let (root, members) = match_vitest_callee(ctx.type_parser, Some(call.expression()));
    let callback_kind = classify_vitest_callback(root, &members);
    if callback_kind == VitestCallbackKind::None {
        return None;
    }

    let mut start = 1;
    if callback_kind == VitestCallbackKind::Hook {
        start = 0;
    }
    let arguments = call.arguments().nodes();
    arguments
        .iter()
        .skip(start)
        .copied()
        .find(|&callback| vitest_callback_returns_effect(ctx.type_parser, callback))
}

// go: internal/rules/floating_effect_in_vitest.go:120 matchVitestCallee 678e2d0ef18c3040
pub(crate) fn match_vitest_callee(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
) -> (VitestRoot, Vec<&'static str>) {
    let mut node = node;
    let mut reversed_members: Vec<&'static str> = Vec::new();
    while let Some(n) = node {
        let n = ast::skip_parentheses(n);
        let root = match_vitest_root(tp, n);
        if !root.name.is_empty() {
            let members: Vec<&'static str> = reversed_members.iter().rev().copied().collect();
            return (root, members);
        }
        match n.kind() {
            Kind::PropertyAccessExpression => {
                let property = n.as_property_access_expression();
                reversed_members.push(property.name().text());
                node = Some(property.expression());
            }
            Kind::CallExpression => {
                let call = n.as_call_expression();
                node = Some(call.expression());
            }
            Kind::TaggedTemplateExpression => {
                let tagged = n.as_tagged_template_expression();
                node = Some(tagged.tag());
            }
            _ => return (VitestRoot::default(), Vec::new()),
        }
    }
    (VitestRoot::default(), Vec::new())
}

// go: internal/rules/floating_effect_in_vitest.go:158 matchVitestRoot b9c247f8743ede84
pub(crate) fn match_vitest_root(tp: &mut TypeParser<'_>, node: P<Node>) -> VitestRoot {
    for name in ["it", "test"] {
        if tp.is_node_reference_to_vitest_api(node, name) {
            return VitestRoot { name, effect_vitest: false };
        }
    }
    if tp.is_node_reference_to_effect_vitest_api(node, "it") {
        return VitestRoot { name: "it", effect_vitest: true };
    }
    for &name in VITEST_HOOK_NAMES {
        if tp.is_node_reference_to_vitest_api(node, name) {
            return VitestRoot { name, effect_vitest: false };
        }
    }
    VitestRoot::default()
}

// go: internal/rules/floating_effect_in_vitest.go:175 classifyVitestCallback 5cc74556d6c66a19
pub(crate) fn classify_vitest_callback(root: VitestRoot, members: &[&str]) -> VitestCallbackKind {
    if root.name.is_empty() {
        return VitestCallbackKind::None;
    }
    if VITEST_HOOK_NAMES.contains(&root.name) {
        if members.is_empty() {
            return VitestCallbackKind::Hook;
        }
        return VitestCallbackKind::None;
    }

    if root.effect_vitest
        && !members.is_empty()
        && EFFECT_AWARE_VITEST_METHODS.contains(&members[0])
    {
        return VitestCallbackKind::None;
    }
    if members.len() == 1 && VITEST_HOOK_NAMES.contains(&members[0]) {
        return VitestCallbackKind::Hook;
    }
    for member in members {
        if !VITEST_TEST_MODIFIERS.contains(member) {
            return VitestCallbackKind::None;
        }
    }
    VitestCallbackKind::Test
}

// go: internal/rules/floating_effect_in_vitest.go:204 vitestCallbackReturnsEffect d814c01a545f512d
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub(crate) fn vitest_callback_returns_effect(tp: &mut TypeParser<'_>, callback: P<Node>) -> bool {
    let Some(callback_type) = tp.get_type_at_location(callback) else {
        return false;
    };
    for member in tp.unroll_union_members(callback_type).unwrap_or_default() {
        for &signature in tp.checker().get_signatures_of_type(member, SignatureKind::Call) {
            let return_type = tp.checker().get_return_type_of_signature(signature);
            if vitest_return_type_contains_effect(tp, Some(return_type), callback, 0) {
                return true;
            }
        }
    }
    if let Some(lazy) =
        ets_typeparser::parse_lazy_expression(callback, LazyExpressionFlags::AllowAsync)
    {
        let expression_type = tp.get_type_at_location(lazy.expression());
        return vitest_return_type_contains_effect(tp, expression_type, lazy.expression(), 0);
    }
    false
}

// go: internal/rules/floating_effect_in_vitest.go:222 vitestReturnTypeContainsEffect a1d0a9b434d8425f
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
#[expect(
    clippy::only_used_in_recursion,
    reason = "Go's signature: `atLocation` is passed down but never read"
)]
pub(crate) fn vitest_return_type_contains_effect(
    tp: &mut TypeParser<'_>,
    return_type: Option<P<Type>>,
    at_location: P<Node>,
    depth: i64,
) -> bool {
    for member in tp.unroll_union_members(return_type).unwrap_or_default() {
        if tp.strict_is_effect_type(member) {
            return true;
        }
        if depth == 0 && tp.promise_type(member).is_some() {
            for &type_arg in tp.checker().get_type_arguments(member) {
                if vitest_return_type_contains_effect(tp, Some(type_arg), at_location, depth + 1) {
                    return true;
                }
            }
        }
    }
    false
}
