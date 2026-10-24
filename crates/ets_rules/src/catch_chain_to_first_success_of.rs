use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, SourceFile};
use ets_shim::checker::{Checker, SignatureKind, Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    LazyExpressionFlags, PipingFlowTransformation, TypeParser, parse_lazy_expression,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::Chained_Effect_catch_fallbacks_that_ignore_the_error_can_be_written_with_Effect_firstSuccessOf_Wrap_each_lazy_fallback_with_Effect_suspend_to_preserve_when_it_is_constructed_effect_catchChainToFirstSuccessOf;

// go: internal/rules/catch_chain_to_first_success_of.go:15 CatchChainToFirstSuccessOf
// CatchChainToFirstSuccessOf suggests firstSuccessOf for equivalent catch fallback chains.
pub static CATCH_CHAIN_TO_FIRST_SUCCESS_OF: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "catchChainToFirstSuccessOf",
    group: "style",
    description: "Suggests Effect.firstSuccessOf for consecutive error-independent Effect.catch fallbacks when the error type is preserved",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_catch_chain_to_first_success_of(ctx.type_parser, Some(source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diagnostics
    },
});

// go: internal/rules/catch_chain_to_first_success_of.go:39 CatchChainToFirstSuccessOfMatch
#[derive(Clone)]
pub struct CatchChainToFirstSuccessOfMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
}

// go: internal/rules/catch_chain_to_first_success_of.go:44 catchChainToFirstSuccessOfCandidate
#[derive(Clone)]
pub(crate) struct CatchChainToFirstSuccessOfCandidate {
    transformation: PipingFlowTransformation,
    error_type: P<Type>,
}

// go: internal/rules/catch_chain_to_first_success_of.go:50 AnalyzeCatchChainToFirstSuccessOf
// AnalyzeCatchChainToFirstSuccessOf finds consecutive zero-argument catch fallbacks.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`), never nil.
pub fn analyze_catch_chain_to_first_success_of(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<CatchChainToFirstSuccessOfMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        let mut chain: Vec<CatchChainToFirstSuccessOfCandidate> = Vec::new();
        let mut initial_error: Option<P<Type>> = None;
        let mut output_error: Option<P<Type>> = None;

        // Rust: Go's `flush` closure is a nested function over the state it captures.
        fn flush(
            tp: &mut TypeParser<'_>,
            sf: P<SourceFile>,
            matches: &mut Vec<CatchChainToFirstSuccessOfMatch>,
            chain: &mut Vec<CatchChainToFirstSuccessOfCandidate>,
            initial_error: &mut Option<P<Type>>,
            output_error: &mut Option<P<Type>>,
        ) {
            if catch_chain_preserves_first_success_of_error(
                tp.checker(),
                *initial_error,
                *output_error,
                chain,
            ) {
                let outermost = &chain[chain.len() - 1].transformation;
                matches.push(CatchChainToFirstSuccessOfMatch {
                    source_file: sf,
                    location: ets_shim::scanner::get_error_range_for_node(sf, outermost.callee()),
                });
            }
            chain.clear();
            *initial_error = None;
            *output_error = None;
        }

        for (i, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(i);
            let Some(candidate) =
                analyze_catch_chain_to_first_success_of_candidate(tp, Some(transformation))
            else {
                flush(tp, sf, &mut matches, &mut chain, &mut initial_error, &mut output_error);
                continue;
            };

            if chain.is_empty() {
                let input_type = if i > 0 {
                    flow.transformations().at(i - 1).out_type(tp)
                } else {
                    flow.subject().out_type(tp)
                };
                let Some(input_effect) = tp.strict_effect_type(input_type) else {
                    flush(tp, sf, &mut matches, &mut chain, &mut initial_error, &mut output_error);
                    continue;
                };
                initial_error = Some(input_effect.e());
            }

            let out_type = transformation.out_type(tp);
            let Some(output_effect) = tp.strict_effect_type(out_type) else {
                flush(tp, sf, &mut matches, &mut chain, &mut initial_error, &mut output_error);
                continue;
            };
            chain.push(candidate);
            output_error = Some(output_effect.e());
        }

        flush(tp, sf, &mut matches, &mut chain, &mut initial_error, &mut output_error);
    }

    matches
}

// go: internal/rules/catch_chain_to_first_success_of.go:110 analyzeCatchChainToFirstSuccessOfCandidate
pub(crate) fn analyze_catch_chain_to_first_success_of_candidate(
    tp: &mut TypeParser<'_>,
    transformation: Option<&PipingFlowTransformation>,
) -> Option<CatchChainToFirstSuccessOfCandidate> {
    let transformation = transformation?;
    if transformation.args().len() != 1 {
        return None;
    }

    parse_lazy_expression(*transformation.args().at(0), LazyExpressionFlags::Thunk)?;

    if !tp.is_node_reference_to_effect_module_api(transformation.callee(), "catch") {
        return None;
    }

    let handler_type = tp.get_type_at_location(*transformation.args().at(0))?;
    let signatures =
        tp.checker().get_signatures_of_type_exported(handler_type, SignatureKind::Call);
    if signatures.len() != 1 {
        return None;
    }

    let return_type = tp.checker().get_return_type_of_signature_exported(signatures[0]);
    let fallback_type = tp.strict_effect_type(return_type)?;

    Some(CatchChainToFirstSuccessOfCandidate {
        transformation: transformation.clone(),
        error_type: fallback_type.e(),
    })
}

// go: internal/rules/catch_chain_to_first_success_of.go:145 catchChainPreservesFirstSuccessOfError
pub(crate) fn catch_chain_preserves_first_success_of_error(
    c: &mut Checker,
    initial_error: Option<P<Type>>,
    output_error: Option<P<Type>>,
    chain: &[CatchChainToFirstSuccessOfCandidate],
) -> bool {
    let (Some(initial_error), Some(output_error)) = (initial_error, output_error) else {
        return false;
    };
    if chain.len() < 2
        || initial_error.flags().intersects(TypeFlags::Any)
        || output_error.flags().intersects(TypeFlags::Any)
    {
        return false;
    }

    let final_error = chain[chain.len() - 1].error_type;
    // catch retains only the final fallback error, while firstSuccessOf unions all
    // candidate errors. The union is unchanged only when each earlier error fits.
    if final_error.flags().intersects(TypeFlags::Any)
        || !c.is_type_assignable_to_exported(final_error, output_error)
        || !c.is_type_assignable_to_exported(output_error, final_error)
        || !c.is_type_assignable_to_exported(initial_error, output_error)
    {
        return false;
    }

    for candidate in &chain[..chain.len() - 1] {
        if candidate.error_type.flags().intersects(TypeFlags::Any)
            || !c.is_type_assignable_to_exported(candidate.error_type, output_error)
        {
            return false;
        }
    }
    true
}
