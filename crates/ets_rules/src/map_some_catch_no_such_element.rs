use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::checker::{LiteralValue, Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    EffectMajorVersion, LazyExpressionFlags, PipingFlow, PipingFlowTransformation, TypeParser,
    parse_lazy_expression,
};

use crate::catch_handler::catch_handler_returns_succeed_none;
use crate::map_some_to_as_some::is_option_some_mapper;
use crate::piping_flow::has_explicit_type_arguments;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::USE_EFFECT_CATCH_NO_SUCH_ELEMENT_INSTEAD_OF_WRAPPING_SUCCESSES_IN_OPTION_SOME_AND_CATCHING_NO_SUCH_ELEMENT_ERROR_INTO_OPTION_NONE_EFFECT_MAP_SOME_CATCH_NO_SUCH_ELEMENT;

// go: internal/rules/map_some_catch_no_such_element.go:14 MapSomeCatchNoSuchElement dd1475d0f52ec20e
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

// go: internal/rules/map_some_catch_no_such_element.go:30 MapSomeCatchNoSuchElementMatch 8c5b5bf3ec92d92b
//
// Rust: Go's `*typeparser.PipingFlow` (into the parser's cached flows) is a clone of the flow.
#[derive(Clone)]
pub struct MapSomeCatchNoSuchElementMatch {
    pub flow: PipingFlow,
    pub start: i64,
    pub end: i64,
    pub location: TextRange,
}

// go: internal/rules/map_some_catch_no_such_element.go:38 noSuchElementErrorTypeId b6b0652e88e6effb
// noSuchElementErrorTypeId is the brand key of Cause.NoSuchElementError, the
// property Effect.catchNoSuchElement checks at runtime.
pub(crate) const NO_SUCH_ELEMENT_ERROR_TYPE_ID: &str = "~effect/Cause/NoSuchElementError";

// go: internal/rules/map_some_catch_no_such_element.go:40 AnalyzeMapSomeCatchNoSuchElement c304f29ae042eef6
//
// Rust: the parser is never nil.
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
            if !map_some_catch_handler_is_plain(*catch.args().at(1))
                || !catch_handler_returns_succeed_none(tp, *catch.args().at(1))
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

// go: internal/rules/map_some_catch_no_such_element.go:73 mapSomeCatchWrapsSome 392f82e15c29668f
// Return type annotations can change any channel of the pair's result, which
// Effect.catchNoSuchElement would replace with Effect<Option<A>, Exclude<E, NoSuchElementError>, R>.
pub(crate) fn map_some_catch_wraps_some(
    tp: &mut TypeParser<'_>,
    step: &PipingFlowTransformation,
) -> bool {
    if step.args().len() == 0 {
        return tp.is_node_reference_to_effect_module_api(step.callee(), "asSome");
    }
    if step.args().len() != 1
        || !tp.is_node_reference_to_effect_module_api(step.callee(), "map")
        || !is_option_some_mapper(tp, *step.args().at(0))
    {
        return false;
    }
    let lazy = parse_lazy_expression(*step.args().at(0), LazyExpressionFlags::None);
    lazy.is_none_or(|lazy| lazy.node().type_node().is_none())
}

// go: internal/rules/map_some_catch_no_such_element.go:84 mapSomeCatchCatchesNoSuchElement 683186b5791664c8
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

// go: internal/rules/map_some_catch_no_such_element.go:96 mapSomeCatchHandlerIsPlain 4a679e773b271fba
// The handler's returned flow is checked to be exactly a successful None, so
// it cannot use the error; destructuring would still read it. A return type
// annotation could change the pair's result type.
pub(crate) fn map_some_catch_handler_is_plain(handler: P<Node>) -> bool {
    let Some(lazy) = parse_lazy_expression(handler, LazyExpressionFlags::None) else {
        return false;
    };
    if lazy.node().type_node().is_some() {
        return false;
    }
    for (_, param) in lazy.params().all() {
        if param.name().is_none_or(|name| name.kind() != Kind::Identifier) {
            return false;
        }
    }
    true
}

// go: internal/rules/map_some_catch_no_such_element.go:112 noSuchElementErrorCatchable 6d835f24c8710296
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
