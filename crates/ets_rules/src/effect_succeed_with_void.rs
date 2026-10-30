use std::cell::Cell;
use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, NodeFlags, SourceFile};
use ets_shim::checker::TypeFlags;
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    Effect, EffectMajorVersion, PipingFlow, PipingFlowSubject, PipingFlowTransformation,
    TransformationKind, TypeParser,
};

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_VOID_REPRESENTS_THE_SAME_OUTCOME_AS_EFFECT_SUCCEED_UNDEFINED_OR_EFFECT_SUCCEED_VOID_0_EFFECT_EFFECT_SUCCEED_WITH_VOID;

// go: internal/rules/effect_succeed_with_void.go:15 EffectSucceedWithVoid 9b81fc628ebdf43f
// EffectSucceedWithVoid suggests using Effect.void instead of Effect.succeed(undefined) or Effect.succeed(void 0).
pub static EFFECT_SUCCEED_WITH_VOID: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "effectSucceedWithVoid",
    group: "style",
    description: "Suggests using Effect.void instead of Effect.succeed(undefined) or Effect.succeed(void 0)",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_effect_succeed_with_void(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/effect_succeed_with_void.go:34 EffectSucceedConstantMatch 36ff4781e8c74b52
// EffectSucceedConstantMatch holds the nodes needed by the effectSucceedWithVoid and
// effectSucceedWithUndefined diagnostics and their quick-fixes.
//
// Rust: Go's `*PipingFlow` is an owned copy.
#[derive(Clone)]
pub struct EffectSucceedConstantMatch {
    pub source_file: P<SourceFile>, // The source file where the diagnostic should be reported
    pub location: TextRange,        // The pre-computed error range for this match
    pub flow: PipingFlow,           // The flow whose subject and first step are replaced
    pub member: &'static str,       // The Effect constant that replaces them: "void" or "undefined"
    pub rewritable: bool, // False when replacing the step would drop a side effect of the callee
}

// go: internal/rules/effect_succeed_with_void.go:43 isVoidExpression 5e71e29c88500c9e
// isVoidExpression checks if a node is `undefined`, `void 0`, or a parenthesized version of either.
//
// Rust: Go dereferences the node without a nil check; a nil node is `None` and is not a void expression.
pub(crate) fn is_void_expression(node: Option<P<Node>>) -> bool {
    let Some(node) = node else {
        return false;
    };
    // Unwrap parenthesized expressions
    let node = ast::skip_parentheses(node);

    // Check for `undefined`
    if node.kind() == Kind::Identifier && ets_shim::scanner::get_text_of_node(node) == "undefined" {
        return true;
    }

    // Check for `void 0`
    if node.kind() == Kind::VoidExpression {
        let operand = node.as_void_expression().expression();
        if operand.kind() == Kind::NumericLiteral
            && ets_shim::scanner::get_text_of_node(operand) == "0"
        {
            return true;
        }
    }

    false
}

// go: internal/rules/effect_succeed_with_void.go:65 AnalyzeEffectSucceedWithVoid 2dab0d593bd965da
// AnalyzeEffectSucceedWithVoid finds Effect.succeed(undefined) and Effect.succeed(void 0)
// steps that Effect.void replaces.
//
// Rust: Go's checker argument is unused and dropped.
pub fn analyze_effect_succeed_with_void(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<EffectSucceedConstantMatch> {
    filter_effect_succeed_constant_matches(analyze_effect_succeed_constant(tp, Some(sf)), "void")
}

// go: internal/rules/effect_succeed_with_void.go:69 filterEffectSucceedConstantMatches 2cabda7db0baff6f
pub(crate) fn filter_effect_succeed_constant_matches(
    matches: Vec<EffectSucceedConstantMatch>,
    member: &str,
) -> Vec<EffectSucceedConstantMatch> {
    let mut result = Vec::new();
    for m in matches {
        if m.member == member {
            result.push(m);
        }
    }
    result
}

// go: internal/rules/effect_succeed_with_void.go:85 analyzeEffectSucceedConstant 4174182138fc9261
// analyzeEffectSucceedConstant finds piping flows whose subject is the constant `undefined`
// or `void 0`, possibly parenthesized, and whose first step is Effect.succeed, as a direct call
// or as a pipe step. The step's success type picks the replacement: void gets Effect.void,
// undefined gets Effect.undefined in Effect v4 and Effect.void in v3, which has no
// Effect.undefined. Any other success type, such as an explicit <number | undefined>, is
// left alone.
//
// Rust: Go's nil parser check has no counterpart (the parser is a reference). The flow predicates are `Fn`, so
// the `member` they set is a `Cell`.
pub(crate) fn analyze_effect_succeed_constant(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<EffectSucceedConstantMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        let member = Cell::new("");
        if !PipingFlow::matches_prefix(
            Some(flow),
            tp,
            &|tp: &mut TypeParser<'_>, subject: &PipingFlowSubject| {
                is_undefined_constant(tp, subject.node())
            },
            &[&|tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| {
                if !is_replaceable_succeed_step(transformation.kind())
                    || transformation.args().len() != 0
                    || !tp.is_node_reference_to_effect_module_api(
                        unwrap_zero_duration_expression(transformation.callee()),
                        "succeed",
                    )
                {
                    return false;
                }
                let out_type = transformation.out_type(tp);
                let effect = tp.strict_effect_type(out_type);
                member.set(effect_succeed_constant_member(tp, effect));
                !member.get().is_empty()
            }],
        ) {
            continue;
        }

        let callee = flow.transformations().at(0).callee();
        matches.push(EffectSucceedConstantMatch {
            source_file: sf,
            location: ets_shim::scanner::get_error_range_for_node(sf, callee),
            flow: flow.clone(),
            member: member.get(),
            rewritable: can_rewrite_succeed_callee(tp, callee),
        });
    }
    matches
}

// go: internal/rules/effect_succeed_with_void.go:122 isUndefinedConstant 902ac5bfa2565b9b
// isUndefinedConstant checks if a node is `undefined` or `void 0`, possibly parenthesized,
// where `undefined` must resolve to the global.
pub(crate) fn is_undefined_constant(tp: &mut TypeParser<'_>, node: Option<P<Node>>) -> bool {
    let Some(node) = node else {
        return false;
    };
    if !is_void_expression(Some(node)) {
        return false;
    }
    let node = ast::skip_parentheses(node);
    // The stability check accepts `undefined` only when it is the global.
    node.kind() != Kind::Identifier || tp.is_expression_value_stable_at_location(node, node)
}

// go: internal/rules/effect_succeed_with_void.go:133 effectSucceedConstantMember 0e6ad44e462e5149
// effectSucceedConstantMember returns the Effect constant with the same type as an
// Effect.succeed result, or "" when there is none.
//
// Rust: the parsed Effect's success type is never nil, so Go's nil check on it drops.
pub(crate) fn effect_succeed_constant_member(
    tp: &mut TypeParser<'_>,
    effect: Option<Effect>,
) -> &'static str {
    let Some(effect) = effect else {
        return "";
    };
    if effect.a().flags().intersects(TypeFlags::Void) {
        return "void";
    }
    if effect.a().flags().intersects(TypeFlags::Undefined) {
        if tp.supported_effect_version() == EffectMajorVersion::V4 {
            return "undefined";
        }
        return "void";
    }
    ""
}

// go: internal/rules/effect_succeed_with_void.go:151 isReplaceableSucceedStep 475162aef7b80183
// isReplaceableSucceedStep reports whether an Effect.succeed step of this kind applies to the
// flow subject: a direct call or a pipe step.
pub(crate) fn is_replaceable_succeed_step(kind: TransformationKind) -> bool {
    kind == TransformationKind::Call || kind == TransformationKind::Pipe
}

// go: internal/rules/effect_succeed_with_void.go:157 canRewriteSucceedCallee 0a90df679e39347e
// canRewriteSucceedCallee reports whether dropping the callee keeps the program's behaviour:
// an identifier, or a property access on an identifier or an imported module.
pub(crate) fn can_rewrite_succeed_callee(tp: &mut TypeParser<'_>, expr: P<Node>) -> bool {
    let callee = unwrap_zero_duration_expression(expr);
    match callee.kind() {
        Kind::Identifier => return true,
        Kind::PropertyAccessExpression => {
            if callee.flags().intersects(NodeFlags::OptionalChain) {
                return false;
            }
            let receiver = callee.as_property_access_expression().expression();
            return unwrap_zero_duration_expression(receiver).kind() == Kind::Identifier
                || is_imported_module_receiver(tp, receiver);
        }
        _ => {}
    }
    false
}
