use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, NodeFlags, SourceFile};
use ets_shim::checker::{SignatureKind, Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    PipingFlow, PipingFlowSubject, PipingFlowTransformation, TransformationKind, TypeParser,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_SCOPED_PROVIDES_A_SCOPE_THAT_THIS_BODY_IMMEDIATELY_READS_BACK_WITH_EFFECT_SCOPE_EFFECT_SCOPED_WITH_PASSES_THE_SCOPE_TO_THE_CALLBACK_DIRECTLY_EFFECT_SCOPED_IMMEDIATE_SCOPE_TO_SCOPED_WITH;

// go: internal/rules/scoped_immediate_scope_to_scoped_with.go:19 ScopedImmediateScopeToScopedWith 1988b0cb3463996c
// ScopedImmediateScopeToScopedWith suggests Effect.scopedWith when
// Effect.scoped wraps a body that immediately reads the provided scope back
// with Effect.scope.
pub static SCOPED_IMMEDIATE_SCOPE_TO_SCOPED_WITH: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "scopedImmediateScopeToScopedWith",
    group: "style",
    description: "Suggests Effect.scopedWith instead of Effect.scoped around a body that immediately reads Effect.scope",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches =
            analyze_scoped_immediate_scope_to_scoped_with(ctx.type_parser, Some(source_file));
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/scoped_immediate_scope_to_scoped_with.go:45 ScopedImmediateScopeToScopedWithMatch b3d2698b1bfb0ebd
// ScopedImmediateScopeToScopedWithMatch holds the nodes needed by the
// diagnostic and quick fix.
//
// Rust: Go's `*PipingFlow` is an owned copy (never nil in a match).
#[derive(Clone)]
pub struct ScopedImmediateScopeToScopedWithMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub flow: PipingFlow,
    // PrefixLength counts the transformations up to and including Effect.scoped;
    // the fix replaces them with the Effect.scopedWith call.
    pub prefix_length: i64,
    pub scoped: PipingFlowTransformation,
    // Generator form: the Effect.gen callee as written, its generator, and the
    // name bound by the leading `const <name> = yield* Effect.scope`.
    pub gen_callee: Option<P<Node>>,
    pub gen_function: Option<P<Node>>,
    pub binding_name: &'static str,
    // Combinator form: the flatMap (or v3 scopeWith) callback.
    pub callback: Option<P<Node>>,
    // Fixable is false when the callback parameter would capture a name the
    // replacement evaluates outside the original generator body.
    pub fixable: bool,
}

/// Rust-only: the fields of `ScopedImmediateScopeToScopedWithMatch` that Go's `analyzeScopedImmediateScope*`
/// helpers fill in a zero-valued match; `AnalyzeScopedImmediateScopeToScopedWith` then sets the source file,
/// flow, scoped step and location, which have no zero value here.
pub(crate) struct ScopedImmediateScopeAnalysis {
    pub(crate) prefix_length: i64,
    pub(crate) gen_callee: Option<P<Node>>,
    pub(crate) gen_function: Option<P<Node>>,
    pub(crate) binding_name: &'static str,
    pub(crate) callback: Option<P<Node>>,
    pub(crate) fixable: bool,
}

/// Rust-only: Go's `isEffectScope` closure, which captures the parser.
pub(crate) type IsEffectScope<'p> = &'p dyn Fn(&mut TypeParser<'_>, Option<P<Node>>) -> bool;

// go: internal/rules/scoped_immediate_scope_to_scoped_with.go:69 AnalyzeScopedImmediateScopeToScopedWith bd4379a55565e3b9
// AnalyzeScopedImmediateScopeToScopedWith finds piping flows that apply
// Effect.scoped to an Effect.gen whose first statement reads Effect.scope, or
// to Effect.flatMap(Effect.scope, f) (Effect.scopeWith(f) on Effect v3), when
// nothing else in the body reads or requires the ambient Scope.
//
// Rust: Go's checker argument is the parser's checker, so it is dropped; Go's nil parser check has no counterpart
// (the parser is a reference). Go's `Callee() == nil` and `arg == nil` checks drop: both are non-optional here.
pub fn analyze_scoped_immediate_scope_to_scoped_with(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<ScopedImmediateScopeToScopedWithMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };

    let no_type_arguments = |transformation: &PipingFlowTransformation| -> bool {
        transformation.type_arguments().is_none_or(|arguments| arguments.nodes().is_empty())
    };
    let is_effect_api =
        |name: &'static str, arg_count: i64, kinds: &'static [TransformationKind]| {
            move |tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| -> bool {
                if transformation.args().len() != arg_count || !no_type_arguments(transformation) {
                    return false;
                }
                if !kinds.is_empty() && !kinds.contains(&transformation.kind()) {
                    return false;
                }
                for (_, arg) in transformation.args().all() {
                    if arg.kind() == Kind::SpreadElement {
                        return false;
                    }
                }
                tp.is_node_reference_to_effect_module_api(transformation.callee(), name)
            }
        };
    // Effect.fn post-transformations apply to a function body, not an expression.
    let is_scoped = is_effect_api(
        "scoped",
        0,
        &[TransformationKind::Call, TransformationKind::Pipe, TransformationKind::Pipeable],
    );
    let is_gen = is_effect_api("gen", 0, &[TransformationKind::Call]);
    let is_flat_map = is_effect_api(
        "flatMap",
        1,
        &[
            TransformationKind::Call,
            TransformationKind::DataFirst,
            TransformationKind::Pipe,
            TransformationKind::Pipeable,
        ],
    );
    let is_scope_with = is_effect_api("scopeWith", 0, &[]);
    let is_generator_subject = |_: &mut TypeParser<'_>, subject: &PipingFlowSubject| -> bool {
        subject.node().is_some_and(|node| {
            node.kind() == Kind::FunctionExpression
                && node.as_function_expression().asterisk_token().is_some()
        })
    };
    let is_effect_scope = |tp: &mut TypeParser<'_>, node: Option<P<Node>>| -> bool {
        node.is_some_and(|node| {
            (ast::is_identifier(node) || ast::is_property_access_expression(node))
                && tp.is_node_reference_to_effect_module_api(node, "scope")
        })
    };
    let is_scope_subject = |tp: &mut TypeParser<'_>, subject: &PipingFlowSubject| -> bool {
        is_effect_scope(tp, subject.node())
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        let mut analysis = None;
        if PipingFlow::matches_prefix(Some(flow), tp, &is_generator_subject, &[&is_gen, &is_scoped])
        {
            analysis = analyze_scoped_immediate_scope_gen(tp, flow, &is_effect_scope);
        } else if PipingFlow::matches_prefix(
            Some(flow),
            tp,
            &is_scope_subject,
            &[&is_flat_map, &is_scoped],
        ) {
            analysis = analyze_scoped_immediate_scope_callback(
                tp,
                flow,
                Some(*flow.transformations().at(0).args().at(0)),
                2,
                &is_effect_scope,
            );
        } else {
            let sequences = PipingFlow::find_transformation_sequences(
                Some(flow),
                tp,
                &[&is_scope_with, &is_scoped],
            );
            for sequence in sequences.iter().flatten() {
                if let Some(callback) =
                    PipingFlow::transformation_input_node(Some(flow), sequence.start())
                {
                    analysis = analyze_scoped_immediate_scope_callback(
                        tp,
                        flow,
                        Some(callback),
                        sequence.start() + 2,
                        &is_effect_scope,
                    );
                    if analysis.is_some() {
                        break;
                    }
                }
            }
        }
        let Some(analysis) = analysis else {
            continue;
        };
        let scoped = flow.transformations().at(analysis.prefix_length - 1).clone();
        matches.push(ScopedImmediateScopeToScopedWithMatch {
            source_file: sf,
            location: ets_shim::scanner::get_error_range_for_node(sf, scoped.callee()),
            flow: flow.clone(),
            prefix_length: analysis.prefix_length,
            scoped,
            gen_callee: analysis.gen_callee,
            gen_function: analysis.gen_function,
            binding_name: analysis.binding_name,
            callback: analysis.callback,
            fixable: analysis.fixable,
        });
    }
    matches
}

// go: internal/rules/scoped_immediate_scope_to_scoped_with.go:143 analyzeScopedImmediateScopeGen 0310fba4b9aa72b9
//
// Rust: `(match, ok)` is an `Option`; the checker is the parser's. Go's nil checks on the declaration list, its
// declarations and the declaration name drop: tsrs holds them as non-optional.
pub(crate) fn analyze_scoped_immediate_scope_gen(
    tp: &mut TypeParser<'_>,
    flow: &PipingFlow,
    is_effect_scope: IsEffectScope<'_>,
) -> Option<ScopedImmediateScopeAnalysis> {
    let gen_step = flow.transformations().at(0);
    let gen_ = tp.effect_gen_call(gen_step.callee().parent());
    // Options, pipe arguments and a return annotation pin the result
    // independently of the body.
    let gen_ = gen_?;
    if gen_.options_node().is_some()
        || gen_.pipe_arguments().len() != 0
        || Some(gen_.generator_function()) != flow.subject().node()
    {
        return None;
    }
    let gen_fn_node = gen_.generator_function();
    let gen_fn = gen_fn_node.as_function_expression();
    if gen_fn.type_().is_some()
        || gen_fn.modifiers().is_some()
        || gen_fn.type_parameters().is_some_and(|parameters| !parameters.nodes().is_empty())
        || gen_fn.body().is_none_or(|body| !ast::is_block(body))
    {
        return None;
    }
    let body = gen_fn.body().unwrap();
    let statements = body.as_block().statements();
    if statements.nodes().is_empty() {
        return None;
    }

    // The first statement must be exactly `const <name> = yield* Effect.scope`.
    let first = statements.nodes()[0];
    if !ast::is_variable_statement(first) {
        return None;
    }
    let declaration_list = first.as_variable_statement().declaration_list();
    if declaration_list.flags() & NodeFlags::BlockScoped != NodeFlags::Const {
        return None;
    }
    let declarations = declaration_list.as_variable_declaration_list().declarations();
    if declarations.nodes().len() != 1 {
        return None;
    }
    let declaration = declarations.nodes()[0].as_variable_declaration();
    if !ast::is_identifier(declaration.name())
        || declaration.type_().is_some()
        || declaration.exclamation_token().is_some()
        || declaration
            .initializer()
            .is_none_or(|initializer| !ast::is_yield_expression(initializer))
    {
        return None;
    }
    let retrieval_node = declaration.initializer().unwrap();
    let retrieval = retrieval_node.as_yield_expression();
    retrieval.asterisk_token()?;
    let operand = ast::skip_parentheses(retrieval.expression()?);
    if !is_effect_scope(tp, Some(operand)) {
        return None;
    }

    // The retrieval must be the only Effect.scope reference in the body.
    if has_effect_scope_reference(tp, Some(body), Some(operand), is_effect_scope) {
        return None;
    }

    // Every other yielded effect must keep its requirements under
    // Effect.scopedWith, which does not provide the ambient Scope.
    let keeps_requirements = scoped_with_keeps_requirements(tp, flow, 2);
    let changes_requirements = ets_shim::checker::for_each_yield_expression(body, |expr| {
        if expr == retrieval_node
            || expr.kind() != Kind::YieldExpression
            || tp.get_effect_yield_generator_function(expr) != Some(gen_fn_node)
        {
            return false;
        }
        let yield_ = expr.as_yield_expression();
        let (Some(_), Some(expression)) = (yield_.asterisk_token(), yield_.expression()) else {
            return false;
        };
        let effect = tp.effect_yieldable_type_at_location(expression);
        effect.is_none_or(|effect| !keeps_requirements(tp, effect.r()))
    });
    if changes_requirements {
        return None;
    }

    let binding_name = declaration.name().text();
    Some(ScopedImmediateScopeAnalysis {
        prefix_length: 2,
        gen_callee: Some(gen_step.callee()),
        gen_function: Some(gen_fn_node),
        binding_name,
        callback: None,
        // The new callback parameter scopes over the gen callee and the
        // generator's name and parameters, which the body binding did not.
        fixable: !references_name(Some(gen_step.callee()), binding_name)
            && !references_name(gen_fn.name(), binding_name)
            && gen_fn.parameters().is_none_or(|parameters| {
                !parameters
                    .nodes()
                    .iter()
                    .any(|&parameter| references_name(Some(parameter), binding_name))
            }),
    })
}

// go: internal/rules/scoped_immediate_scope_to_scoped_with.go:227 analyzeScopedImmediateScopeCallback 6948a952db373522
//
// Rust: `(match, ok)` is an `Option`; the checker is the parser's.
pub(crate) fn analyze_scoped_immediate_scope_callback(
    tp: &mut TypeParser<'_>,
    flow: &PipingFlow,
    callback: Option<P<Node>>,
    prefix_length: i64,
    is_effect_scope: IsEffectScope<'_>,
) -> Option<ScopedImmediateScopeAnalysis> {
    let callback = callback?;
    if has_effect_scope_reference(tp, Some(callback), None, is_effect_scope) {
        return None;
    }
    // The flatMap and scopeWith results already require Scope, so the effect
    // the callback returns must keep its requirements under Effect.scopedWith.
    let keeps_requirements = scoped_with_keeps_requirements(tp, flow, prefix_length);
    let callback_type = tp.get_type_at_location(callback)?;
    let call_signatures = tp.checker().get_signatures_of_type(callback_type, SignatureKind::Call);
    if call_signatures.is_empty() {
        return None;
    }
    for &signature in call_signatures {
        let return_type = tp.checker().get_return_type_of_signature(signature);
        let effect = tp.effect_type(return_type);
        if effect.is_none_or(|effect| !keeps_requirements(tp, effect.r())) {
            return None;
        }
    }
    Some(ScopedImmediateScopeAnalysis {
        prefix_length,
        gen_callee: None,
        gen_function: None,
        binding_name: "",
        callback: Some(callback),
        fixable: true,
    })
}

// go: internal/rules/scoped_immediate_scope_to_scoped_with.go:257 hasEffectScopeReference 40be2db853dd237b
// hasEffectScopeReference reports whether node contains a reference to
// Effect.scope other than allowed, skipping type positions.
//
// Rust: Go's recursive `visit` closure is a nested `fn` taking what it captured.
pub(crate) fn has_effect_scope_reference(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
    allowed: Option<P<Node>>,
    is_effect_scope: IsEffectScope<'_>,
) -> bool {
    fn visit(
        tp: &mut TypeParser<'_>,
        allowed: Option<P<Node>>,
        is_effect_scope: IsEffectScope<'_>,
        found: &mut bool,
        current: Option<P<Node>>,
    ) -> bool {
        let Some(current) = current else {
            return *found;
        };
        if *found || ast::is_type_node(current) {
            return *found;
        }
        if ast::is_property_access_expression(current) {
            if Some(current) != allowed && is_effect_scope(tp, Some(current)) {
                *found = true;
                return true;
            }
            // The member name is part of this access, not a reference of its own.
            return visit(
                tp,
                allowed,
                is_effect_scope,
                found,
                Some(current.as_property_access_expression().expression()),
            );
        } else if ast::is_identifier(current) {
            if Some(current) != allowed && is_effect_scope(tp, Some(current)) {
                *found = true;
            }
            return *found;
        }
        current.for_each_child(&mut |child| visit(tp, allowed, is_effect_scope, found, Some(child)))
    }
    let mut found = false;
    visit(tp, allowed, is_effect_scope, &mut found, node);
    found
}

// go: internal/rules/scoped_immediate_scope_to_scoped_with.go:297 scopedWithKeepsRequirements e3df00091d154d4b
// scopedWithKeepsRequirements returns a check that an effect moved from
// Effect.scoped to Effect.scopedWith, which does not provide the ambient
// Scope, neither needs that Scope nor changes the result type.
//
// Each union member of its requirements must be independent of Scope: not
// any, unknown or generic, and neither assignable to Scope nor a supertype of
// it, since any of those may stand for a Scope the effect reads at runtime.
// Comparing against the combined requirements alone cannot show that, because
// broad combined requirements such as unknown accept Scope too.
//
// The requirements must also be assignable to what Effect.scoped produced,
// Exclude<R, Scope>, so the rewrite, which yields them unchanged, keeps the
// result type.
//
// Rust: the returned check takes the parser (Go's closure captures it and the checker). Its requirements are
// non-optional (every caller passes `Effect::r()`), so Go's `requirements == nil` check drops, as does
// `scoped.R() == nil`. `independentOfScope` takes the Scope type found by the returned check's guard, where Go's
// closure reads the captured, possibly nil variable that the guard has ruled out.
pub(crate) fn scoped_with_keeps_requirements(
    tp: &mut TypeParser<'_>,
    flow: &PipingFlow,
    prefix_length: i64,
) -> impl Fn(&mut TypeParser<'_>, P<Type>) -> bool + use<> {
    let scoped_index = prefix_length - 1;
    let scoped_out_type = flow.transformations().at(scoped_index).out_type(tp);
    let scoped = tp.effect_type(scoped_out_type);
    // The retrieval puts Scope in the requirements Effect.scoped receives.
    let mut scope_type: Option<P<Type>> = None;
    let input_type = PipingFlow::transformation_input_type(Some(flow), tp, scoped_index);
    if let Some(input) = tp.effect_type(input_type) {
        for member in tp.unroll_union_members(input.r()).into_iter().flatten() {
            if tp.scope_type(member).is_some() {
                scope_type = Some(member);
                break;
            }
        }
    }
    let independent_of_scope =
        |tp: &mut TypeParser<'_>, member: P<Type>, scope_type: P<Type>| -> bool {
            if member.flags().intersects(TypeFlags::Never) {
                return true;
            }
            !member.flags().intersects(TypeFlags::AnyOrUnknown | TypeFlags::Instantiable)
                && tp.scope_type(member).is_none()
                && !tp.checker().is_type_assignable_to(member, scope_type)
                && !tp.checker().is_type_assignable_to(scope_type, member)
        };
    move |tp: &mut TypeParser<'_>, requirements: P<Type>| -> bool {
        let (Some(scoped), Some(scope_type)) = (scoped, scope_type) else {
            return false;
        };
        for member in tp.unroll_union_members(requirements).into_iter().flatten() {
            if !independent_of_scope(tp, member, scope_type) {
                return false;
            }
        }
        tp.checker().is_type_assignable_to(requirements, scoped.r())
    }
}

// go: internal/rules/scoped_immediate_scope_to_scoped_with.go:334 referencesName d17dc26aa3a6adb5
// referencesName reports whether node contains an identifier named name,
// other than a property access member name.
//
// Rust: Go's recursive `visit` closure is a nested `fn`.
pub(crate) fn references_name(node: Option<P<Node>>, name: &str) -> bool {
    fn visit(current: Option<P<Node>>, name: &str) -> bool {
        let Some(current) = current else {
            return false;
        };
        if ast::is_property_access_expression(current) {
            return visit(Some(current.as_property_access_expression().expression()), name);
        } else if ast::is_identifier(current) {
            return current.text() == name;
        }
        current.for_each_child(&mut |child| visit(Some(child), name))
    }
    visit(node, name)
}
