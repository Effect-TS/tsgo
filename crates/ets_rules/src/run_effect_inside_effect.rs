// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::scanner::get_source_text_of_node_from_source_file;
use ets_typeparser::{
    EffectContextFlags, EffectMajorVersion, PipingFlowTransformation, TypeParser,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static EXISTING_CONTEXT_MESSAGE: &Message =
    &tsdiag::X_0_IS_CALLED_INSIDE_AN_EXISTING_EFFECT_CONTEXT_HERE_THE_INNER_EFFECT_CAN_BE_USED_DIRECTLY_EFFECT_RUN_EFFECT_INSIDE_EFFECT;

/// Rust-only: the rule's message (Go spells it out at each use).
static RUNTIME_MESSAGE: &Message =
    &tsdiag::X_0_IS_CALLED_INSIDE_AN_EFFECT_WITH_A_SEPARATE_RUNTIME_INVOCATION_IN_THIS_CONTEXT_RUN_CHILD_EFFECTS_WITH_THE_SURROUNDING_RUNTIME_WHICH_CAN_BE_ACCESSED_THROUGH_EFFECT_RUNTIME_AND_RUNTIME_1_EFFECT_RUN_EFFECT_INSIDE_EFFECT;

/// Rust-only: the rule's message (Go spells it out at each use).
static SERVICES_MESSAGE: &Message =
    &tsdiag::X_0_IS_CALLED_INSIDE_AN_EFFECT_WITH_A_SEPARATE_SERVICES_INVOCATION_IN_THIS_CONTEXT_CHILD_EFFECTS_RUN_WITH_THE_SURROUNDING_SERVICES_WHICH_CAN_BE_ACCESSED_THROUGH_EFFECT_CONTEXT_AND_EFFECT_1_WITH_EFFECT_RUN_EFFECT_INSIDE_EFFECT;

/// Rust-only: the rule's message (Go spells it out at each use).
static PLATFORM_RUN_MAIN_MESSAGE: &Message =
    &tsdiag::X_0_IS_CALLED_INSIDE_AN_EXISTING_EFFECT_CONTEXT_AND_STARTS_A_SEPARATE_ROOT_RUNTIME_THAT_THE_SURROUNDING_EFFECT_DOES_NOT_OWN_RETURN_THE_EFFECT_INSTEAD_AND_CALL_0_ONCE_AT_THE_APPLICATION_ENTRY_POINT_EFFECT_RUN_EFFECT_INSIDE_EFFECT;

// go: internal/rules/run_effect_inside_effect.go:16 runEffectApis 46156d4d0e340906
// runEffectApis lists the Effect module run APIs that should not be called inside Effect generators.
pub(crate) static RUN_EFFECT_APIS: &[&str] =
    &["runSync", "runSyncExit", "runPromise", "runPromiseExit", "runFork", "runCallback"];

// go: internal/rules/run_effect_inside_effect.go:20 RunEffectInsideEffect 79fe3f0bd785833b
// RunEffectInsideEffect detects calls to the runEffectApis inside Effect generator contexts
// and suggests alternatives.
pub static RUN_EFFECT_INSIDE_EFFECT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "runEffectInsideEffect",
    group: "antipattern",
    description: "Suggests using Runtime or Effect.run*With methods instead of Effect.run* inside Effect contexts",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![
        EXISTING_CONTEXT_MESSAGE.code(),
        RUNTIME_MESSAGE.code(),
        SERVICES_MESSAGE.code(),
        PLATFORM_RUN_MAIN_MESSAGE.code(),
    ],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let supported_effect = ctx.type_parser.supported_effect_version();

        let source_file = ctx.source_file;
        let matches = analyze_run_effect_inside_effect(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            let callee_text =
                get_source_text_of_node_from_source_file(m.source_file, m.callee_node, false);
            if m.is_platform_run_main {
                diags.push(ctx.new_diagnostic(
                    m.source_file,
                    m.location,
                    PLATFORM_RUN_MAIN_MESSAGE,
                    &[],
                    &[&callee_text],
                ));
            } else if m.is_nested_scope {
                if supported_effect == EffectMajorVersion::V4 {
                    diags.push(ctx.new_diagnostic(
                        m.source_file,
                        m.location,
                        SERVICES_MESSAGE,
                        &[],
                        &[&callee_text, m.method_name],
                    ));
                } else {
                    diags.push(ctx.new_diagnostic(
                        m.source_file,
                        m.location,
                        RUNTIME_MESSAGE,
                        &[],
                        &[&callee_text, m.method_name],
                    ));
                }
            } else {
                diags.push(ctx.new_diagnostic(
                    m.source_file,
                    m.location,
                    EXISTING_CONTEXT_MESSAGE,
                    &[],
                    &[&callee_text],
                ));
            }
        }
        diags
    },
});

// go: internal/rules/run_effect_inside_effect.go:57 RunEffectInsideEffectMatch bd5f911aa69d6d1b
// RunEffectInsideEffectMatch holds the diagnostic and AST nodes needed by both the
// diagnostic rule and the quick-fix for the runEffectInsideEffect pattern.
//
// Rust: Go's `*ast.FunctionExpression` is its node, and its nil is `None`. Go's never-nil transformation
// pointer is an owned clone.
#[derive(Clone)]
pub struct RunEffectInsideEffectMatch {
    pub source_file: P<SourceFile>, // The source file where this match was found
    pub location: TextRange,        // Pre-computed error range (on the callee expression)
    pub callee_node: P<Node>,       // The callee expression (e.g., Effect.runPromise)
    pub transformation: PipingFlowTransformation, // The piping flow step that runs the Effect
    pub method_name: &'static str,  // The matched run API name (e.g., "runPromise")
    pub is_platform_run_main: bool, // True for NodeRuntime.runMain and the other platform runMain APIs
    pub is_nested_scope: bool, // True when call is in a nested scope rather than direct generator scope
    pub generator_function: Option<P<Node>>, // The enclosing Effect generator function, None for a platform runMain
}

// go: internal/rules/run_effect_inside_effect.go:70 AnalyzeRunEffectInsideEffect 548171eab8c62b45
// AnalyzeRunEffectInsideEffect finds all Effect.run* and platform runMain steps of the piping flows
// that run inside an Effect, returning matches with structured data for both diagnostics and quick-fixes.
//
// Rust: Go's checker argument is the parser's checker; its callees ignore it, so it is dropped. A transformation
// callee is never nil in tsrs, so Go's nil check drops.
pub fn analyze_run_effect_inside_effect(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<RunEffectInsideEffectMatch> {
    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        let mut flow_context: Option<RunEffectInsideEffectFlowContext> = None;
        for (_, transformation) in flow.transformations().all() {
            let callee = transformation.callee();
            let is_platform_run_main =
                tp.is_node_reference_to_platform_runtime_api(callee, "runMain");
            let mut method_name = "runMain";
            if !is_platform_run_main {
                method_name = match_run_effect_api(tp, callee);
                if method_name.is_empty() {
                    continue;
                }
            }

            let flow_context = *flow_context.get_or_insert_with(|| {
                resolve_run_effect_inside_effect_flow_context(tp, flow.node())
            });
            if is_platform_run_main && !flow_context.in_effect
                || !is_platform_run_main && flow_context.generator_function.is_none()
            {
                continue;
            }

            let mut m = RunEffectInsideEffectMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, callee),
                callee_node: callee,
                transformation: transformation.clone(),
                method_name,
                is_platform_run_main,
                is_nested_scope: false,
                generator_function: None,
            };
            if !is_platform_run_main {
                m.is_nested_scope = flow_context.is_nested_scope;
                m.generator_function = flow_context.generator_function;
            }
            matches.push(m);
        }
    }
    matches
}

// go: internal/rules/run_effect_inside_effect.go:114 runEffectInsideEffectFlowContext 5831426dac92c1a9
// runEffectInsideEffectFlowContext is the Effect context of a piping flow, shared by all its steps.
//
// Rust: Go's `*ast.FunctionExpression` is its node, and its nil is `None`.
#[derive(Clone, Copy)]
struct RunEffectInsideEffectFlowContext {
    in_effect: bool, // True when the flow runs inside an Effect, such as an Effect.sync thunk
    generator_function: Option<P<Node>>, // The enclosing Effect generator function with at least one statement, or None
    is_nested_scope: bool, // True when the flow is in a nested scope rather than direct generator scope
}

// go: internal/rules/run_effect_inside_effect.go:121 resolveRunEffectInsideEffectFlowContext e384e52b0d2df94e
// resolveRunEffectInsideEffectFlowContext resolves the Effect context of the flow rooted at node.
//
// Rust: Go returns a pointer for its lazily filled local; the context is `Copy`, so it returns the value.
fn resolve_run_effect_inside_effect_flow_context(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
) -> RunEffectInsideEffectFlowContext {
    let mut flow_context = RunEffectInsideEffectFlowContext {
        in_effect: is_inside_effect_context(tp, node),
        generator_function: None,
        is_nested_scope: false,
    };

    let mut gen_fn = tp.get_effect_yield_generator_function(node);
    if gen_fn.is_none() {
        let mut current = node.parent();
        while let Some(c) = current {
            if tp.get_effect_context_flags(c).intersects(EffectContextFlags::CanYieldEffect) {
                gen_fn = tp.get_effect_yield_generator_function(c);
                if gen_fn.is_some() {
                    break;
                }
            }
            current = c.parent();
        }
    }
    let Some(gen_fn) = gen_fn else {
        return flow_context;
    };
    let Some(body) = gen_fn.as_function_expression().body() else {
        return flow_context;
    };
    if body.kind() != Kind::Block {
        return flow_context;
    }
    if body.as_block().statements().nodes().is_empty() {
        return flow_context;
    }

    flow_context.generator_function = Some(gen_fn);
    flow_context.is_nested_scope = ast::get_containing_function(node) != Some(gen_fn);
    flow_context
}

// go: internal/rules/run_effect_inside_effect.go:150 matchRunEffectApi eeef31b0776d262d
// matchRunEffectApi checks if the node references one of the Effect.run* APIs and returns the method name.
// Returns empty string if no match.
//
// Rust: Go's unused checker parameter is dropped.
pub(crate) fn match_run_effect_api(tp: &mut TypeParser<'_>, node: P<Node>) -> &'static str {
    for name in RUN_EFFECT_APIS {
        if tp.is_node_reference_to_effect_module_api(node, name) {
            return name;
        }
    }
    ""
}

// go: internal/rules/run_effect_inside_effect.go:161 isInsideEffectContext 232e99a24cd200c8
// isInsideEffectContext reports whether node or one of its ancestors runs inside an Effect,
// such as an Effect.gen body or an Effect.sync thunk.
fn is_inside_effect_context(tp: &mut TypeParser<'_>, node: P<Node>) -> bool {
    let mut current = Some(node);
    while let Some(c) = current {
        if tp.get_effect_context_flags(c) != EffectContextFlags::None {
            return true;
        }
        current = c.parent();
    }
    false
}
