use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, SourceFile};
use ets_shim::checker::{SignatureKind, Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::scanner;
use ets_typeparser::{PipingFlow, PipingFlowTransformation, TypeParser};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::EFFECT_0_IS_CALLED_WITHOUT_A_CATCH_HANDLER_SO_FAILURES_SURFACE_AS_THE_GENERIC_1_THE_OBJECT_FORM_TRY_CATCH_MAPS_THE_CAUSE_TO_A_TYPED_DOMAIN_SPECIFIC_ERROR_EFFECT_EFFECT_TRY_WITHOUT_CATCH;

// go: internal/rules/effect_try_without_catch.go:17 EffectTryWithoutCatch 7fe1dc52f2512ca4
// EffectTryWithoutCatch reports short-form Effect.try / Effect.tryPromise
// calls, whose failures surface as the generic UnknownError (v4) or
// UnknownException (v3) instead of a domain-specific error.
pub static EFFECT_TRY_WITHOUT_CATCH: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "effectTryWithoutCatch",
    group: "antipattern",
    description: "Reports Effect.try and Effect.tryPromise calls without a catch handler, whose failures surface as a generic unknown error",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_effect_try_without_catch(ctx.type_parser, Some(source_file));
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            let error_type = ctx.checker().type_to_string_exported(m.error_type);
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[m.api_name, &error_type],
            ));
        }
        diags
    },
});

// go: internal/rules/effect_try_without_catch.go:44 EffectTryWithoutCatchMatch af18a21b49fff545
// EffectTryWithoutCatchMatch holds the nodes needed by the diagnostic.
#[derive(Clone)]
pub struct EffectTryWithoutCatchMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub api_name: &'static str, // try or tryPromise
    pub error_type: P<Type>,
}

// go: internal/rules/effect_try_without_catch.go:55 AnalyzeEffectTryWithoutCatch 43e3ac06cf28f530
// AnalyzeEffectTryWithoutCatch finds piping flow steps that apply Effect.try or
// Effect.tryPromise to a function, i.e. the short form without catch, however
// the step is written: Effect.try(thunk), pipe(thunk, Effect.try) or a later
// step of a longer flow.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`), so it is dropped, and Go's nil checks on
// the parser and checker drop with it. A transformation's callee and an Effect's error type are never nil in Rust,
// so those nil checks drop.
pub fn analyze_effect_try_without_catch(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<EffectTryWithoutCatchMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    let api_name =
        |tp: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| -> &'static str {
            let callee = transformation.callee();
            if transformation.args().len() != 0 {
                return "";
            }
            for name in ["try", "tryPromise"] {
                if tp.is_node_reference_to_effect_module_api(callee, name) {
                    return name;
                }
            }
            ""
        };
    let is_try_step = |tp: &mut TypeParser<'_>,
                       transformation: &PipingFlowTransformation|
     -> bool { !api_name(tp, transformation).is_empty() };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        let sequences = PipingFlow::find_transformation_sequences(Some(flow), tp, &[&is_try_step]);
        for sequence in sequences.iter().flatten() {
            let index = sequence.start();
            let step = flow.transformations().at(index).clone();
            // The short form takes a function; the object form { try, catch }
            // has no call signature, and any cannot tell the forms apart.
            let Some(input) = PipingFlow::transformation_input_type(Some(flow), tp, index) else {
                continue;
            };
            if input.flags().intersects(TypeFlags::Any)
                || tp.checker().get_signatures_of_type(input, SignatureKind::Call).is_empty()
            {
                continue;
            }
            let out_type = step.out_type(tp);
            let Some(effect) = tp.strict_effect_type(out_type) else {
                continue;
            };
            matches.push(EffectTryWithoutCatchMatch {
                source_file: sf,
                location: scanner::get_error_range_for_node(sf, step.callee()),
                api_name: api_name(tp, &step),
                error_type: effect.e(),
            });
        }
    }
    matches
}
