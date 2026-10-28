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
    &tsdiag::X_0_IS_CALLED_INSIDE_AN_EFFECT_CONTEXT_AND_STARTS_A_SEPARATE_ROOT_RUNTIME_WHICH_THE_SURROUNDING_EFFECT_DOES_NOT_OWN_RETURN_THE_EFFECT_INSTEAD_AND_RUN_IT_WITH_RUN_MAIN_ONCE_AT_THE_APPLICATION_ENTRYPOINT_EFFECT_RUN_EFFECT_INSIDE_EFFECT;

// go: internal/rules/run_effect_inside_effect.go:16 runEffectApis 46156d4d0e340906
// runEffectApis lists the Effect module run APIs that should not be called inside Effect generators.
pub(crate) static RUN_EFFECT_APIS: &[&str] =
    &["runSync", "runSyncExit", "runPromise", "runPromiseExit", "runFork", "runCallback"];

// go: internal/rules/run_effect_inside_effect.go:20 platformRunMainApi 5267f433b399d5d7
// platformRunMainApi is the platform runtime API (NodeRuntime, BunRuntime, BrowserRuntime)
// that starts a separate root runtime and should only be called at the application entrypoint.
pub(crate) const PLATFORM_RUN_MAIN_API: &str = "runMain";

// go: internal/rules/run_effect_inside_effect.go:24 RunEffectInsideEffect e072bf93a6ef4412
// RunEffectInsideEffect detects calls to the runEffectApis and to the platform runMain inside
// Effect contexts and suggests alternatives.
pub static RUN_EFFECT_INSIDE_EFFECT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "runEffectInsideEffect",
    group: "antipattern",
    description: "Suggests using Runtime or Effect.run*With methods instead of Effect.run* inside Effect contexts, and reports platform runMain calls there",
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

// go: internal/rules/run_effect_inside_effect.go:61 RunEffectInsideEffectMatch 56bf39dda5809122
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
    pub is_nested_scope: bool, // True when call is in a nested scope rather than direct generator scope
    pub is_platform_run_main: bool, // True when the callee is a platform runMain (e.g., NodeRuntime.runMain)
    pub generator_function: Option<P<Node>>, // The enclosing Effect generator function, nil for a platform runMain in an Effect constructor thunk
}

// go: internal/rules/run_effect_inside_effect.go:74 AnalyzeRunEffectInsideEffect b3af332d490681d5
// AnalyzeRunEffectInsideEffect finds all Effect.run* calls inside Effect generators, and the
// platform runMain calls inside Effect generators or Effect constructor thunks, returning matches with structured data for both diagnostics and quick-fixes.
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

// go: internal/rules/run_effect_inside_effect.go:98 analyzeRunEffectInsideEffectNode be7a84b10e3e412f
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

    // A platform runMain starts its own root runtime, so it is reported in every Effect context
    if tp.is_node_reference_to_platform_runtime_api(callee, PLATFORM_RUN_MAIN_API) {
        return analyze_platform_run_main_node(tp, sf, node, callee);
    }

    // Check if the callee is one of the Effect.run* APIs
    let method_name = match_run_effect_api(tp, callee);
    if method_name.is_empty() {
        return None;
    }

    let gen_fn = find_enclosing_effect_generator_function(tp, node)?;

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
        is_nested_scope,
        is_platform_run_main: false,
        generator_function: Some(gen_fn),
    })
}

// go: internal/rules/run_effect_inside_effect.go:151 analyzePlatformRunMainNode 105fe3b70291cd8e
// analyzePlatformRunMainNode reports a platform runMain call found inside an Effect generator,
// including its nested functions, or directly inside an Effect constructor thunk such as Effect.sync.
//
// Rust: `(match, ok)` is an `Option`.
fn analyze_platform_run_main_node(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
    callee: P<Node>,
) -> Option<RunEffectInsideEffectMatch> {
    let gen_fn = find_enclosing_effect_generator_function(tp, node);
    if gen_fn.is_none()
        && !tp
            .get_effect_context_flags(node)
            .intersects(EffectContextFlags::InEffectConstructorThunk)
    {
        return None;
    }

    Some(RunEffectInsideEffectMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, callee),
        call_node: node,
        callee_node: callee,
        method_name: PLATFORM_RUN_MAIN_API,
        is_nested_scope: gen_fn.is_some() && ast::get_containing_function(node) != gen_fn,
        is_platform_run_main: true,
        generator_function: gen_fn,
    })
}

// go: internal/rules/run_effect_inside_effect.go:171 findEnclosingEffectGeneratorFunction b476ed04924b00d0
// findEnclosingEffectGeneratorFunction returns the Effect generator node yields to, or else the
// generator of the closest ancestor that can yield an Effect.
fn find_enclosing_effect_generator_function(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
) -> Option<P<Node>> {
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
    gen_fn
}

// go: internal/rules/run_effect_inside_effect.go:188 matchRunEffectApi eeef31b0776d262d
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
