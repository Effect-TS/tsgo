// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Node, SourceFile};
use ets_shim::checker::{Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    EffectMajorVersion, LazyExpressionFlags, PipingFlowTransformation, TypeParser,
    parse_lazy_expression,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_1_EXPRESSES_IGNORED_FAILURE_MORE_DIRECTLY_THAN_EFFECT_0_RETURNING_EFFECT_VOID_EFFECT_CATCH_TO_IGNORE;

// go: internal/rules/catch_to_ignore.go:16 CatchToIgnore 7d7b201fefbed8e4
// CatchToIgnore suggests using Effect.ignore or Effect.ignoreCause instead of Effect.catch/catchCause + Effect.void.
pub static CATCH_TO_IGNORE: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "catchToIgnore",
    group: "style",
    description: "Suggests using Effect.ignore or Effect.ignoreCause instead of Effect.catch/catchCause returning Effect.void",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_catch_to_ignore(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[m.catch_method_name, m.ignore_method_name],
            ));
        }
        diags
    },
});

// go: internal/rules/catch_to_ignore.go:35 CatchToIgnoreMatch d957fe8c62b67e80
// CatchToIgnoreMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the catchToIgnore pattern.
#[derive(Clone)]
pub struct CatchToIgnoreMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub transformation: PipingFlowTransformation,
    pub catch_method_name: &'static str,
    pub ignore_method_name: &'static str,
}

// go: internal/rules/catch_to_ignore.go:45 AnalyzeCatchToIgnore 7931ec83d976bcab
// AnalyzeCatchToIgnore finds Effect.catch/catchCause callbacks that return Effect.void
// where the resulting success channel is void-like, so Effect.ignore/ignoreCause is equivalent.
//
// Rust: Go's checker argument is the parser's checker; Go ignores it, so it is dropped.
pub fn analyze_catch_to_ignore(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<CatchToIgnoreMatch> {
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }

    let mut matches = Vec::new();

    let flows = tp.piping_flows(sf, true);
    for (_, flow) in ets_readonly::new(flows.as_deref()).all() {
        for (index, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(index);
            if transformation.args().len() < 1 {
                continue;
            }

            let Some((catch_method_name, ignore_method_name)) =
                catch_to_ignore_methods(tp, Some(transformation.callee()))
            else {
                continue;
            };

            let lazy =
                parse_lazy_expression(*transformation.args().at(0), LazyExpressionFlags::None);
            let Some(lazy) = lazy else {
                continue;
            };
            if !is_effect_void_reference(tp, Some(lazy.expression())) {
                continue;
            }

            let out_type = transformation.out_type(tp);
            let effect = tp.strict_effect_type(out_type);
            let Some(effect) = effect else {
                continue;
            };
            if !is_void_like_effect_success(Some(effect.a())) {
                continue;
            }

            matches.push(CatchToIgnoreMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, transformation.callee()),
                transformation: transformation.clone(),
                catch_method_name,
                ignore_method_name,
            });
        }
    }

    matches
}

// go: internal/rules/catch_to_ignore.go:88 catchToIgnoreMethods 5638072388b27ab6
// Rust: Go's `(catchMethodName, ignoreMethodName, ok)` is an `Option` of the first two.
pub(crate) fn catch_to_ignore_methods(
    tp: &mut TypeParser<'_>,
    callee: Option<P<Node>>,
) -> Option<(&'static str, &'static str)> {
    let callee = callee?;
    if tp.is_node_reference_to_effect_module_api(callee, "catch") {
        Some(("catch", "ignore"))
    } else if tp.is_node_reference_to_effect_module_api(callee, "catchCause") {
        Some(("catchCause", "ignoreCause"))
    } else {
        None
    }
}

// go: internal/rules/catch_to_ignore.go:102 isEffectVoidReference 984dedd0b9db9d55
pub(crate) fn is_effect_void_reference(tp: &mut TypeParser<'_>, node: Option<P<Node>>) -> bool {
    let Some(node) = node else {
        return false;
    };
    tp.is_node_reference_to_effect_module_api(ast::skip_parentheses(node), "void")
}

// go: internal/rules/catch_to_ignore.go:109 isVoidLikeEffectSuccess 3dd64589730c59f2
pub(crate) fn is_void_like_effect_success(t: Option<P<Type>>) -> bool {
    let Some(t) = t else {
        return false;
    };
    let flags = t.flags();
    if flags.intersects(TypeFlags::VoidLike | TypeFlags::Never) {
        return true;
    }
    if !flags.intersects(TypeFlags::Union) {
        return false;
    }
    let types = t.types();
    if types.is_empty() {
        return false;
    }
    for &member in types {
        if !is_void_like_effect_success(Some(member)) {
            return false;
        }
    }
    true
}
