use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, SourceFile};
use ets_shim::checker::{LiteralValue, Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    Effect, EffectMajorVersion, PipingFlow, PipingFlowTransformation, TypeParser,
};

use crate::catch_handler::{catch_handler_ignores_error, catch_handler_returns_succeed_none};
use crate::map_some_to_as_some::is_option_some_mapper;
use crate::piping_flow::has_explicit_type_arguments;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::USE_EFFECT_CATCH_NO_SUCH_ELEMENT_INSTEAD_OF_WRAPPING_SUCCESSES_IN_OPTION_SOME_AND_CATCHING_NO_SUCH_ELEMENT_ERROR_INTO_OPTION_NONE_EFFECT_MAP_SOME_CATCH_NO_SUCH_ELEMENT;

// go: internal/rules/map_some_catch_no_such_element.go:16 MapSomeCatchNoSuchElement 85d156707fe9a216
pub static MAP_SOME_CATCH_NO_SUCH_ELEMENT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "mapSomeCatchNoSuchElement",
    group: "style",
    description: "Suggests Effect.catchNoSuchElement instead of wrapping successes in Option.some and catching NoSuchElementError into Option.none",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut diagnostics = Vec::new();
        let source_file = ctx.source_file;
        for m in analyze_map_some_catch_no_such_element(ctx.type_parser, Some(source_file)) {
            diagnostics.push(ctx.new_diagnostic(ctx.source_file, m.location, MESSAGE, &[], &[]));
        }
        diagnostics
    },
});

// go: internal/rules/map_some_catch_no_such_element.go:32 MapSomeCatchNoSuchElementMatch 8c5b5bf3ec92d92b
//
// Rust: Go's `*typeparser.PipingFlow` (into the parser's cached flows) is a clone of the flow.
#[derive(Clone)]
pub struct MapSomeCatchNoSuchElementMatch {
    pub flow: PipingFlow,
    pub start: i64,
    pub end: i64,
    pub location: TextRange,
}

// go: internal/rules/map_some_catch_no_such_element.go:40 noSuchElementErrorTypeId b6b0652e88e6effb
// noSuchElementErrorTypeId is the brand key of Cause.NoSuchElementError, the
// property Effect.catchNoSuchElement checks at runtime.
pub(crate) const NO_SUCH_ELEMENT_ERROR_TYPE_ID: &str = "~effect/Cause/NoSuchElementError";

// go: internal/rules/map_some_catch_no_such_element.go:42 AnalyzeMapSomeCatchNoSuchElement 95662b52194c2aba
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`); the parser and checker are never nil.
pub fn analyze_map_some_catch_no_such_element(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<MapSomeCatchNoSuchElementMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for (_, flow) in ets_readonly::new(flows.as_deref()).all() {
        let sequences = PipingFlow::find_transformation_sequences(
            Some(flow),
            tp,
            &[
                &|tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| {
                    map_some_catch_wraps_some(tp, step)
                },
                &|tp: &mut TypeParser<'_>, step: &PipingFlowTransformation| {
                    map_some_catch_catches_no_such_element(tp, step)
                },
            ],
        );
        for (_, sequence) in ets_readonly::new(sequences.as_deref()).all() {
            let start = sequence.start();
            let (wrap, catch) =
                (sequence.transformations().at(0), sequence.transformations().at(1));
            if has_explicit_type_arguments(Some(wrap)) || has_explicit_type_arguments(Some(catch)) {
                continue;
            }
            let handler = ast::skip_parentheses(*catch.args().at(1));
            if !catch_handler_ignores_error(tp, Some(handler))
                || !catch_handler_returns_succeed_none(tp, handler)
            {
                continue;
            }
            let input_type = PipingFlow::transformation_input_type(Some(flow), tp, start);
            let Some(input) = tp.strict_effect_type(input_type) else {
                continue;
            };
            if !no_such_element_error_catchable(tp, Some(input.e())) {
                continue;
            }
            let output_type = catch.out_type(tp);
            let Some(output) = tp.strict_effect_type(output_type) else {
                continue;
            };
            if !map_some_catch_matches_catch_no_such_element(tp, &input, &output) {
                continue;
            }
            matches.push(MapSomeCatchNoSuchElementMatch {
                flow: flow.clone(),
                start,
                end: start + 1,
                location: ets_shim::scanner::get_error_range_for_node(sf, catch.callee()),
            });
        }
    }
    matches
}

// go: internal/rules/map_some_catch_no_such_element.go:78 mapSomeCatchWrapsSome 3ebe81476db50aba
pub(crate) fn map_some_catch_wraps_some(
    tp: &mut TypeParser<'_>,
    step: &PipingFlowTransformation,
) -> bool {
    step.args().len() == 0 && tp.is_node_reference_to_effect_module_api(step.callee(), "asSome")
        || step.args().len() == 1
            && tp.is_node_reference_to_effect_module_api(step.callee(), "map")
            && is_option_some_mapper(tp, *step.args().at(0))
}

// go: internal/rules/map_some_catch_no_such_element.go:83 mapSomeCatchCatchesNoSuchElement 683186b5791664c8
pub(crate) fn map_some_catch_catches_no_such_element(
    tp: &mut TypeParser<'_>,
    step: &PipingFlowTransformation,
) -> bool {
    if step.args().len() != 2 {
        return false;
    }
    let tag = ast::skip_parentheses(*step.args().at(0));
    tag.kind() == Kind::StringLiteral
        && tag.text() == "NoSuchElementError"
        && tp.is_node_reference_to_effect_module_api(step.callee(), "catchTag")
}

// go: internal/rules/map_some_catch_no_such_element.go:96 mapSomeCatchMatchesCatchNoSuchElement 73c512483b3cf4d5
// The fix replaces the pair's result type with catchNoSuchElement's
// Effect<Option<A>, Exclude<E, NoSuchElementError>, R>. Callback annotations can
// change any channel of the pair, so compare the pair's output with that shape.
// Assignability accepts any in place of every type, so use type identity.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub(crate) fn map_some_catch_matches_catch_no_such_element(
    tp: &mut TypeParser<'_>,
    input: &Effect,
    output: &Effect,
) -> bool {
    let identical = |tp: &mut TypeParser<'_>, source: P<Type>, target: P<Type>| -> bool {
        tp.checker().is_type_identical_to(source, target)
    };
    for member in tp.unroll_union_members(output.a()).unwrap_or_default() {
        let Some(tag) = tp.get_type_of_property_by_name(member, "_tag") else {
            return false;
        };
        if !tag.flags().intersects(TypeFlags::StringLiteral) {
            return false;
        }
        match tag.as_literal_type().value() {
            Some(LiteralValue::String(text)) if text == "Some" || text == "None" => {}
            _ => return false,
        }
        // Option.none() and Effect.succeedNone contribute Option<never>.
        let arguments = tp.checker().get_type_arguments(member);
        if arguments.len() != 1
            || !arguments[0].flags().intersects(TypeFlags::Never)
                && !identical(tp, arguments[0], input.a())
        {
            return false;
        }
    }
    let mut remaining: Vec<P<Type>> = Vec::new();
    for member in tp.unroll_union_members(input.e()).unwrap_or_default() {
        if tp.get_type_of_property_by_name(member, NO_SUCH_ELEMENT_ERROR_TYPE_ID).is_none() {
            remaining.push(member);
        }
    }
    let mut produced: Vec<P<Type>> = Vec::new();
    for member in tp.unroll_union_members(output.e()).unwrap_or_default() {
        if !member.flags().intersects(TypeFlags::Never) {
            produced.push(member);
        }
    }
    let contains = |tp: &mut TypeParser<'_>, types: &[P<Type>], target: P<Type>| -> bool {
        types.iter().any(|&candidate| identical(tp, candidate, target))
    };
    for &member in &produced {
        if !contains(tp, &remaining, member) {
            return false;
        }
    }
    for &member in &remaining {
        if !contains(tp, &produced, member) {
            return false;
        }
    }
    identical(tp, output.r(), input.r())
}

// go: internal/rules/map_some_catch_no_such_element.go:145 noSuchElementErrorCatchable 6d835f24c8710296
// catchTag matches by _tag while catchNoSuchElement checks the brand, so every
// member that may carry the tag must be the real Cause.NoSuchElementError.
// Untagged, broadly tagged or generic members fail closed.
pub(crate) fn no_such_element_error_catchable(
    tp: &mut TypeParser<'_>,
    error_type: Option<P<Type>>,
) -> bool {
    let Some(error_type) = error_type.filter(|t| !t.flags().intersects(TypeFlags::Never)) else {
        return false;
    };
    let mut found = false;
    for member in tp.unroll_union_members(error_type).unwrap_or_default() {
        let branded =
            tp.get_type_of_property_by_name(member, NO_SUCH_ELEMENT_ERROR_TYPE_ID).is_some();
        found = found || branded;
        let Some(tag) = tp.get_type_of_property_by_name(member, "_tag") else {
            return false;
        };
        for value in tp.unroll_union_members(tag).unwrap_or_default() {
            if !value.flags().intersects(TypeFlags::StringLiteral) {
                return false;
            }
            let Some(LiteralValue::String(text)) = value.as_literal_type().value() else {
                return false;
            };
            if text == "NoSuchElementError" && !branded {
                return false;
            }
        }
    }
    found
}
