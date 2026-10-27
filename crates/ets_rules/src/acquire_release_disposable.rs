use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile, Symbol, SymbolFlags};
use ets_shim::checker::{Type, TypeFlags, UnionReduction};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectMajorVersion, LazyExpressionFlags, TypeParser};

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_ACQUIRE_DISPOSABLE_EXPRESSES_THIS_DISPOSABLE_RESOURCE_ACQUISITION_MORE_DIRECTLY_THAN_EFFECT_ACQUIRE_RELEASE_WITH_A_MANUAL_DISPOSAL_FINALIZER_EFFECT_ACQUIRE_RELEASE_DISPOSABLE;

// go: internal/rules/acquire_release_disposable.go:17 AcquireReleaseDisposable 8e579c8e95847096
// AcquireReleaseDisposable suggests Effect.acquireDisposable when an
// acquireRelease finalizer only invokes the acquired resource's JavaScript
// disposal protocol.
pub static ACQUIRE_RELEASE_DISPOSABLE: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "acquireReleaseDisposable",
    group: "style",
    description: "Suggests Effect.acquireDisposable when Effect.acquireRelease only invokes the acquired resource's disposal protocol",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_acquire_release_disposable(ctx.type_parser, Some(source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diagnostics
    },
});

// go: internal/rules/acquire_release_disposable.go:43 AcquireReleaseDisposableMatch 6b952ab7f98e91ee
// AcquireReleaseDisposableMatch holds the nodes needed by the diagnostic and
// its quick fix.
#[derive(Clone)]
pub struct AcquireReleaseDisposableMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub call_node: P<Node>,
    pub acquire: P<Node>,
    pub has_type_arguments: bool,
}

// go: internal/rules/acquire_release_disposable.go:54 AnalyzeAcquireReleaseDisposable 97fb0da445cc9c45
// AnalyzeAcquireReleaseDisposable finds Effect.acquireRelease calls whose
// resulting success type is disposable and whose release callback consists
// solely of invoking that resource's disposal protocol.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`), so its nil check drops.
pub fn analyze_acquire_release_disposable(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<AcquireReleaseDisposableMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }

    let disposable = global_disposable_union(tp);
    let global_symbol = tp.checker().get_global_symbol_exported("Symbol", SymbolFlags::Value, None);
    let (Some(disposable), Some(global_symbol)) = (disposable, global_symbol) else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        node: P<Node>,
        disposable: P<Type>,
        global_symbol: P<Symbol>,
        matches: &mut Vec<AcquireReleaseDisposableMatch>,
    ) -> bool {
        if let Some(m) =
            analyze_acquire_release_disposable_call(tp, sf, node, disposable, global_symbol)
        {
            matches.push(m);
        }

        node.for_each_child(&mut |child| walk(tp, sf, child, disposable, global_symbol, matches));
        false
    }

    walk(tp, sf, sf.as_node(), disposable, global_symbol, &mut matches);
    matches
}

// go: internal/rules/acquire_release_disposable.go:84 globalDisposableUnion cd019e3fc2ce5ad8
// Rust: Go's checker argument is the parser's checker. `GetDeclaredTypeOfSymbol` never returns nil in tsrs.
pub(crate) fn global_disposable_union(tp: &mut TypeParser<'_>) -> Option<P<Type>> {
    let c = tp.checker();
    let mut types = Vec::new();
    for name in ["Disposable", "AsyncDisposable"] {
        let Some(symbol) = c.get_global_symbol_exported(name, SymbolFlags::Type, None) else {
            continue;
        };
        types.push(c.get_declared_type_of_symbol_exported(symbol));
    }
    if types.is_empty() {
        return None;
    }
    if types.len() == 1 {
        return Some(types[0]);
    }
    Some(c.get_union_type_ex_exported(&types, UnionReduction::None))
}

// go: internal/rules/acquire_release_disposable.go:104 analyzeAcquireReleaseDisposableCall f76adc225a44bdd6
pub(crate) fn analyze_acquire_release_disposable_call(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
    disposable: P<Type>,
    global_symbol: P<Symbol>,
) -> Option<AcquireReleaseDisposableMatch> {
    if node.kind() != Kind::CallExpression {
        return None;
    }
    let call = node.as_call_expression();
    let args = call.arguments().nodes();
    if call.question_dot_token().is_some()
        || args.len() != 2
        || contains_spread_element(args)
        || !tp.is_node_reference_to_effect_module_api(call.expression(), "acquireRelease")
    {
        return None;
    }

    let node_type = tp.get_type_at_location(node);
    let result = tp.strict_effect_type(node_type)?;
    if !is_definitely_disposable(tp, Some(result.a()), Some(disposable)) {
        return None;
    }

    if !is_disposal_release(tp, args[1], global_symbol) {
        return None;
    }

    Some(AcquireReleaseDisposableMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, call.expression()),
        call_node: node,
        acquire: args[0],
        has_type_arguments: has_call_type_arguments(Some(call)),
    })
}

// go: internal/rules/acquire_release_disposable.go:139 isDefinitelyDisposable 006cbf7992fb77dd
pub(crate) fn is_definitely_disposable(
    tp: &mut TypeParser<'_>,
    success: Option<P<Type>>,
    disposable: Option<P<Type>>,
) -> bool {
    let (Some(success), Some(disposable)) = (success, disposable) else {
        return false;
    };
    for member in tp.unroll_union_members(success).unwrap_or_default() {
        if member.flags().intersects(TypeFlags::AnyOrUnknown | TypeFlags::Never) {
            return false;
        }
    }
    tp.checker().is_type_assignable_to_exported(success, disposable)
}

// go: internal/rules/acquire_release_disposable.go:151 isDisposalRelease f2b585469555f5b4
pub(crate) fn is_disposal_release(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
    global_symbol: P<Symbol>,
) -> bool {
    let Some(release) = ets_typeparser::parse_lazy_expression(node, LazyExpressionFlags::None)
    else {
        return false;
    };
    if release.params().len() < 1 || release.params().len() > 2 {
        return false;
    }

    let resource = *release.params().at(0);
    if !is_plain_identifier_parameter(Some(resource)) {
        return false;
    }
    if release.params().len() == 2 && !is_plain_identifier_parameter(Some(*release.params().at(1)))
    {
        return false;
    }

    let expression = ast::skip_parentheses(release.expression());
    if expression.kind() != Kind::CallExpression {
        return false;
    }
    let wrapper = expression.as_call_expression();
    let wrapper_args = wrapper.arguments().nodes();
    if wrapper.question_dot_token().is_some()
        || wrapper_args.len() != 1
        || contains_spread_element(wrapper_args)
    {
        return false;
    }
    let Some(resource_symbol) = tp.get_symbol_at_location(resource.name()) else {
        return false;
    };

    let wrapper_name = if tp.is_node_reference_to_effect_module_api(wrapper.expression(), "sync") {
        "sync"
    } else if tp.is_node_reference_to_effect_module_api(wrapper.expression(), "promise") {
        "promise"
    } else {
        return false;
    };

    let mut flags = LazyExpressionFlags::Thunk;
    if wrapper_name == "promise" {
        flags = flags | LazyExpressionFlags::AllowAsync;
    }
    // Rust: `ParsedLazyExpression.Expression` is never nil in Rust, so Go's nil check drops.
    let Some(thunk) = ets_typeparser::parse_lazy_expression(wrapper_args[0], flags) else {
        return false;
    };

    let (protocol, ok) =
        disposal_protocol_call(tp, Some(thunk.expression()), resource_symbol, global_symbol);
    ok && (protocol == "dispose" && wrapper_name == "sync"
        || protocol == "asyncDispose" && wrapper_name == "promise")
}

// go: internal/rules/acquire_release_disposable.go:202 isPlainIdentifierParameter 74c6e6e2192550b0
pub(crate) fn is_plain_identifier_parameter(node: Option<P<Node>>) -> bool {
    let Some(node) = node else {
        return false;
    };
    if node.kind() != Kind::Parameter || node.name().is_none_or(|n| n.kind() != Kind::Identifier) {
        return false;
    }
    let parameter = node.as_parameter_declaration();
    parameter.dot_dot_dot_token().is_none()
        && parameter.question_token().is_none()
        && parameter.initializer().is_none()
}

// go: internal/rules/acquire_release_disposable.go:210 disposalProtocolCall 770e22371150945d
pub(crate) fn disposal_protocol_call(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
    resource_symbol: P<Symbol>,
    global_symbol: P<Symbol>,
) -> (&'static str, bool) {
    let Some(node) = node.map(ast::skip_parentheses) else {
        return ("", false);
    };
    if node.kind() != Kind::CallExpression {
        return ("", false);
    }
    let call = node.as_call_expression();
    if call.question_dot_token().is_some()
        || call.type_arguments().is_some_and(|t| !t.nodes().is_empty())
        || !call.arguments().nodes().is_empty()
        || call.expression().kind() != Kind::ElementAccessExpression
    {
        return ("", false);
    }

    let element = call.expression().as_element_access_expression();
    if element.question_dot_token().is_some() {
        return ("", false);
    }

    let key = ast::skip_parentheses(element.argument_expression());
    if key.kind() != Kind::PropertyAccessExpression {
        return ("", false);
    }
    let access = key.as_property_access_expression();
    if access.question_dot_token().is_some() {
        return ("", false);
    }
    let Some(actual_resource) =
        tp.get_symbol_at_location(ast::skip_parentheses(element.expression()))
    else {
        return ("", false);
    };
    if tp.checker().get_symbol_if_same_reference(actual_resource, resource_symbol).is_none() {
        return ("", false);
    }
    let access_symbol = tp.get_symbol_at_location(access.expression());
    let actual_symbol = tp.resolve_to_global_symbol(access_symbol);
    if actual_symbol != Some(global_symbol) {
        return ("", false);
    }

    let protocol = access.name().text();
    (protocol, protocol == "dispose" || protocol == "asyncDispose")
}
