use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, NodeFlags, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{LazyExpressionFlags, TypeParser};

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_EFFECT_USE_SPAN_CALLBACK_NEVER_USES_THE_SPAN_IT_RECEIVES_EFFECT_USE_SPAN_DOES_NOT_MAKE_THAT_SPAN_THE_PARENT_OF_SPANS_CREATED_INSIDE_THE_CALLBACK_SO_THEY_WILL_NOT_NEST_UNDER_IT_EFFECT_WITH_SPAN_DOES_AND_REMOVES_PARENT_SPAN_FROM_THE_REQUIREMENTS_EFFECT_USE_SPAN_UNUSED_SPAN_TO_WITH_SPAN;

// go: internal/rules/use_span_unused_span_to_with_span.go:15 UseSpanUnusedSpanToWithSpan 0dae2670af7dcc15
pub static USE_SPAN_UNUSED_SPAN_TO_WITH_SPAN: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "useSpanUnusedSpanToWithSpan",
    group: "antipattern",
    description: "Suggests Effect.withSpan when an Effect.useSpan callback never uses its span, since useSpan does not parent inner spans",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_use_span_unused_span_to_with_span(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/use_span_unused_span_to_with_span.go:38 UseSpanUnusedSpanToWithSpanMatch c129e722fc301dd7
// UseSpanUnusedSpanToWithSpanMatch is an Effect.useSpan call whose inline
// callback ignores its span. Receiver and Body are nil when the call cannot be
// rewritten to `Body.pipe(Receiver.withSpan(Name, Options))`.
#[derive(Clone)]
pub struct UseSpanUnusedSpanToWithSpanMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub call: P<Node>,
    pub name: P<Node>,
    pub options: Option<P<Node>>,
    pub callback: P<Node>,
    pub receiver: Option<P<Node>>,
    pub body: Option<P<Node>>,
}

// go: internal/rules/use_span_unused_span_to_with_span.go:49 AnalyzeUseSpanUnusedSpanToWithSpan bb1c56b6a3cc6de2
//
// Rust: Go's checker argument is unused and dropped.
pub fn analyze_use_span_unused_span_to_with_span(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<UseSpanUnusedSpanToWithSpanMatch> {
    let mut matches = Vec::new();
    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        node: P<Node>,
        matches: &mut Vec<UseSpanUnusedSpanToWithSpanMatch>,
    ) -> bool {
        if node.kind() == Kind::CallExpression
            && let Some(m) = analyze_use_span_call(tp, sf, node)
        {
            matches.push(m);
        }
        node.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }
    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}

// go: internal/rules/use_span_unused_span_to_with_span.go:65 analyzeUseSpanCall 7a0df952cef35297
//
// Rust: tsrs's call expression and arguments are never nil, so Go's nil checks drop.
pub(crate) fn analyze_use_span_call(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
) -> Option<UseSpanUnusedSpanToWithSpanMatch> {
    let call = node.as_call_expression();
    if node.flags().intersects(NodeFlags::OptionalChain) {
        return None;
    }
    let args = call.arguments().nodes();
    if args.len() != 2 && args.len() != 3 {
        return None;
    }
    for arg in args {
        if arg.kind() == Kind::SpreadElement {
            return None;
        }
    }
    let callee = ast::skip_parentheses(call.expression());
    if callee.flags().intersects(NodeFlags::OptionalChain) {
        return None;
    } else if callee.kind() == Kind::PropertyAccessExpression {
        if callee.as_property_access_expression().name().text() != "useSpan" {
            return None;
        }
    } else if callee.kind() != Kind::Identifier {
        return None;
    }
    let callback = ast::skip_parentheses(args[args.len() - 1]);
    if callback.kind() != Kind::ArrowFunction && callback.kind() != Kind::FunctionExpression {
        return None;
    }
    let params = callback.parameters();
    match params.len() {
        0 => {}
        1 => {
            let param = params[0];
            if !plain_tap_parameter(param)
                || param.name().expect("a plain parameter has a name").text() == "this"
            {
                return None;
            }
        }
        _ => return None,
    }

    if !tp.is_node_reference_to_effect_module_api(callee, "useSpan") {
        return None;
    }
    let body = callback.body()?;
    if reads_arguments(body)
        || params.len() == 1 && tp.are_parameters_referenced_in_exported(callback, &[body])
    {
        return None;
    }

    let mut m = UseSpanUnusedSpanToWithSpanMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, callee),
        call: node,
        name: args[0],
        options: None,
        callback,
        receiver: None,
        body: None,
    };
    if args.len() == 3 {
        m.options = Some(args[1]);
    }
    (m.receiver, m.body) = use_span_rewrite_parts(tp, node, callee, callback);
    Some(m)
}

// go: internal/rules/use_span_unused_span_to_with_span.go:133 useSpanRewriteParts c0288e5c8cb88b1b
// useSpanRewriteParts returns the Effect module receiver and the callback's
// returned expression when the call can be rewritten to Effect.withSpan, or nil
// for both when moving the expression out of the callback could change what
// it observes or there is no module receiver to reuse.
pub(crate) fn use_span_rewrite_parts(
    tp: &mut TypeParser<'_>,
    call: P<Node>,
    callee: P<Node>,
    callback: P<Node>,
) -> (Option<P<Node>>, Option<P<Node>>) {
    if call.as_call_expression().type_arguments().is_some()
        || callee.kind() != Kind::PropertyAccessExpression
    {
        return (None, None);
    }
    let receiver = callee.as_property_access_expression().expression();
    if !is_imported_module_receiver(tp, receiver)
        || !tp.is_expression_effect_module(unwrap_zero_duration_expression(receiver))
    {
        return (None, None);
    }
    let Some(parsed) = ets_typeparser::parse_lazy_expression(callback, LazyExpressionFlags::None)
    else {
        return (None, None);
    };
    let body = parsed.expression();
    if callback.kind() == Kind::FunctionExpression
        && uses_function_expression_bindings(tp, callback, ets_readonly::new(None), body)
    {
        return (None, None);
    }
    if !tp.is_expression_evaluable_at_location(body, call) {
        return (None, None);
    }
    (Some(receiver), Some(body))
}

// go: internal/rules/use_span_unused_span_to_with_span.go:158 readsArguments c9f88dd3103cf22a
// readsArguments reports whether body reads an `arguments` object outside
// nested functions that bind their own: the callback's own for a function
// expression, the enclosing function's for an arrow function.
pub(crate) fn reads_arguments(body: P<Node>) -> bool {
    // Rust: Go's recursive `visit` closure is a nested function.
    fn visit(node: P<Node>) -> bool {
        match node.kind() {
            Kind::Identifier => {
                if node.text() != "arguments" {
                    return false;
                }
                let parent = node.parent();
                if parent.is_some_and(ast::is_shorthand_property_assignment) {
                    // `{ arguments }` reads the object it names.
                    return true;
                }
                if ast::is_declaration_name(node) {
                    return false;
                }
                return parent.is_none_or(|parent| {
                    parent.kind() != Kind::PropertyAccessExpression || parent.name() != Some(node)
                });
            }
            Kind::FunctionExpression
            | Kind::FunctionDeclaration
            | Kind::MethodDeclaration
            | Kind::GetAccessor
            | Kind::SetAccessor
            | Kind::Constructor => {
                // A computed method name is evaluated in the enclosing scope.
                if let Some(name) = node.name()
                    && ast::is_computed_property_name(name)
                {
                    return visit(name);
                }
                return false;
            }
            _ => {}
        }
        node.for_each_child(&mut |child| visit(child))
    }
    visit(body)
}
