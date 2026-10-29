// Package rules contains all Effect diagnostic rule implementations.

use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::scanner::get_source_text_of_node_from_source_file;
use ets_typeparser::{EffectContextFlags, EffectMajorVersion, TypeParser};

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

// go: internal/rules/run_effect_inside_effect.go:57 RunEffectInsideEffectMatch a3f9aa0b9ff527c9
// RunEffectInsideEffectMatch holds the diagnostic and AST nodes needed by both the
// diagnostic rule and the quick-fix for the runEffectInsideEffect pattern.
//
// Rust: Go's `*ast.FunctionExpression` is its node, and its nil is `None`.
#[derive(Clone)]
pub struct RunEffectInsideEffectMatch {
    pub source_file: P<SourceFile>, // The source file where this match was found
    pub location: TextRange,        // Pre-computed error range (on the callee expression)
    pub call_node: P<Node>, // The full call expression node (e.g., Effect.runPromise(check))
    pub callee_node: P<Node>, // The callee expression (e.g., Effect.runPromise)
    pub method_name: &'static str, // The matched run API name (e.g., "runPromise")
    pub is_platform_run_main: bool, // True for NodeRuntime.runMain and the other platform runMain APIs
    pub is_nested_scope: bool, // True when call is in a nested scope rather than direct generator scope
    pub generator_function: Option<P<Node>>, // The enclosing Effect generator function, None for a platform runMain
}

// go: internal/rules/run_effect_inside_effect.go:70 AnalyzeRunEffectInsideEffect 96865eebeae4b9db
// AnalyzeRunEffectInsideEffect finds all Effect.run* calls inside Effect generators,
// returning matches with structured data for both diagnostics and quick-fixes.
//
// Rust: Go's checker argument is the parser's checker; its callees ignore it, so it is dropped.
pub fn analyze_run_effect_inside_effect(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<RunEffectInsideEffectMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        n: P<Node>,
        matches: &mut Vec<RunEffectInsideEffectMatch>,
    ) -> bool {
        if n.kind() == Kind::CallExpression
            && let Some(m) = analyze_run_effect_inside_effect_node(tp, sf, n)
        {
            matches.push(m);
        }

        n.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}

// go: internal/rules/run_effect_inside_effect.go:94 analyzeRunEffectInsideEffectNode 337a4283a92e59bf
// analyzeRunEffectInsideEffectNode checks a single call expression for Effect.run* inside an Effect generator.
//
// Rust: `(match, ok)` is an `Option`; Go's unused checker parameter is dropped.
pub(crate) fn analyze_run_effect_inside_effect_node(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
) -> Option<RunEffectInsideEffectMatch> {
    if node.kind() != Kind::CallExpression {
        return None;
    }
    let call = node.as_call_expression();

    // Must have at least one argument (matching the TS reference: node.arguments.length === 0 => continue)
    if call.arguments().nodes().is_empty() {
        return None;
    }

    let callee = call.expression();

    if tp.is_node_reference_to_platform_runtime_api(callee, "runMain") {
        if !is_inside_effect_context(tp, node) {
            return None;
        }
        return Some(RunEffectInsideEffectMatch {
            source_file: sf,
            location: ets_shim::scanner::get_error_range_for_node(sf, callee),
            call_node: node,
            callee_node: callee,
            method_name: "runMain",
            is_platform_run_main: true,
            is_nested_scope: false,
            generator_function: None,
        });
    }

    // Check if the callee is one of the Effect.run* APIs
    let method_name = match_run_effect_api(tp, callee);
    if method_name.is_empty() {
        return None;
    }

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
    let gen_fn = gen_fn?;

    // Check that the generator body has at least one statement
    let body = gen_fn.as_function_expression().body()?;
    if body.kind() != Kind::Block {
        return None;
    }
    let block = body.as_block();
    if block.statements().nodes().is_empty() {
        return None;
    }

    let is_nested_scope = ast::get_containing_function(node) != Some(gen_fn);

    Some(RunEffectInsideEffectMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, callee),
        call_node: node,
        callee_node: callee,
        method_name,
        is_platform_run_main: false,
        is_nested_scope,
        generator_function: Some(gen_fn),
    })
}

// go: internal/rules/run_effect_inside_effect.go:166 matchRunEffectApi eeef31b0776d262d
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

// go: internal/rules/run_effect_inside_effect.go:177 isInsideEffectContext 232e99a24cd200c8
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
