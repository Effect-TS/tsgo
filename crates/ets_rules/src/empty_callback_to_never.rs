use std::cell::Cell;
use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, ModifierFlags, Node, NodeFlags, NodeList, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    EffectMajorVersion, PipingFlow, PipingFlowSubject, PipingFlowTransformation, TypeParser,
};

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_EFFECT_CALLBACK_REGISTER_IS_EMPTY_SO_THE_EFFECT_NEVER_COMPLETES_USE_EFFECT_NEVER_INSTEAD_EFFECT_EMPTY_CALLBACK_TO_NEVER;

// go: internal/rules/empty_callback_to_never.go:14 EmptyCallbackToNever 74f534902ebd0bac
pub static EMPTY_CALLBACK_TO_NEVER: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "emptyCallbackToNever",
    group: "style",
    description: "Suggests Effect.never instead of Effect.callback with an empty register",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_empty_callback_to_never(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/empty_callback_to_never.go:34 EmptyCallbackToNeverMatch 3bb8e86c0267bcf0
#[derive(Clone)]
pub struct EmptyCallbackToNeverMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    // Flow has the register as its subject and Effect.callback as its first step;
    // the fix replaces that prefix.
    //
    // Rust: Go's `*PipingFlow` is an owned copy.
    pub flow: PipingFlow,
    // EffectModule is the callee's own module receiver, or nil when no fix is offered.
    pub effect_module: Option<P<Node>>,
}

// go: internal/rules/empty_callback_to_never.go:48 AnalyzeEmptyCallbackToNever e0c8a4132c2f352e
// AnalyzeEmptyCallbackToNever finds v4 piping flows that apply Effect.callback to a
// function literal with an empty body, where the explicit type arguments, if any, are
// all never. The register is the subject of its flow, whether it is written as
// Effect.callback(register) or piped into a bare Effect.callback step.
//
// Rust: Go's checker argument is unused and dropped; the nil checks on `tp` and `sf` drop with it (both are
// non-optional here). The flow predicates are `Fn`, so the `callee` they set is a `Cell`.
pub fn analyze_empty_callback_to_never(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<EmptyCallbackToNeverMatch> {
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        let callee: Cell<Option<P<Node>>> = Cell::new(None);
        if !PipingFlow::matches_prefix(
            Some(flow),
            tp,
            &|_: &mut TypeParser<'_>, subject: &PipingFlowSubject| {
                subject.node().is_some_and(is_empty_register_literal)
            },
            &[&|tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| {
                callee.set(empty_callback_callee(transformation));
                callee.get().is_some_and(|callee| {
                    tp.is_node_reference_to_effect_module_api(callee, "callback")
                })
            }],
        ) {
            continue;
        }
        let Some(callee) = callee.get() else {
            continue;
        };
        let mut receiver = None;
        if callee.kind() == Kind::PropertyAccessExpression {
            let candidate = callee.as_property_access_expression().expression();
            if is_imported_module_receiver(tp, candidate)
                && tp.is_expression_effect_module(unwrap_zero_duration_expression(candidate))
            {
                receiver = Some(candidate);
            }
        }
        matches.push(EmptyCallbackToNeverMatch {
            source_file: sf,
            location: ets_shim::scanner::get_error_range_for_node(
                sf,
                flow.transformations().at(0).callee(),
            ),
            flow: flow.clone(),
            effect_module: receiver,
        });
    }
    matches
}

// go: internal/rules/empty_callback_to_never.go:85 emptyCallbackCallee f6c1ff39463d8cb8
// emptyCallbackCallee returns the callee of a bare step, unwrapped from parentheses,
// assertions and instantiation expressions, or nil when the step is an optional call,
// takes arguments of its own, or has an explicit type argument other than never.
//
// Rust: Go's nil check on the callee drops (tsrs holds it as non-optional).
pub(crate) fn empty_callback_callee(transformation: &PipingFlowTransformation) -> Option<P<Node>> {
    let mut callee = transformation.callee();
    if transformation.args().len() != 0 {
        return None;
    }
    if let Some(call) = callee.parent()
        && ast::is_call_expression(call)
        && call.as_call_expression().expression() == callee
        && call.flags().intersects(NodeFlags::OptionalChain)
    {
        return None;
    }
    let mut type_arguments: Vec<Option<P<NodeList>>> = vec![transformation.type_arguments()];
    callee = unwrap_zero_duration_expression(callee);
    while callee.kind() == Kind::ExpressionWithTypeArguments {
        let instantiation = callee.as_expression_with_type_arguments();
        type_arguments.push(instantiation.type_arguments());
        callee = unwrap_zero_duration_expression(instantiation.expression());
    }
    if callee.flags().intersects(NodeFlags::OptionalChain) {
        return None;
    }
    for list in type_arguments {
        let Some(list) = list else {
            continue;
        };
        for &type_argument in list.nodes() {
            if ast::skip_type_parentheses(type_argument).kind() != Kind::NeverKeyword {
                return None;
            }
        }
    }
    Some(callee)
}

// go: internal/rules/empty_callback_to_never.go:119 isEmptyRegisterLiteral 09ab3c4c9957a84b
// isEmptyRegisterLiteral reports whether node is a non-async, non-generator arrow or function
// expression with a statement-free block body and plain parameters without defaults.
//
// Rust: Go's nil check on `node` moves to the caller, which holds an `Option`.
pub(crate) fn is_empty_register_literal(node: P<Node>) -> bool {
    match node.kind() {
        Kind::ArrowFunction => {}
        Kind::FunctionExpression => {
            if node.as_function_expression().asterisk_token().is_some() {
                return false;
            }
        }
        _ => return false,
    }
    if ast::has_syntactic_modifier(node, ModifierFlags::Async) {
        return false;
    }
    let Some(body) = ets_typeparser::get_function_like_body(node) else {
        return false;
    };
    if body.kind() != Kind::Block {
        return false;
    }
    if !body.as_block().statements().nodes().is_empty() {
        return false;
    }
    if let Some(parameters) = ets_typeparser::get_function_like_parameters(node) {
        for &parameter in parameters.nodes() {
            let declaration = parameter.as_parameter_declaration();
            if declaration.initializer().is_some() || ast::is_binding_pattern(declaration.name()) {
                return false;
            }
        }
    }
    true
}
