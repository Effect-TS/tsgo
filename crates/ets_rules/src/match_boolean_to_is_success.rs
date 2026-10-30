use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{LazyExpressionFlags, PipingFlow, TransformationKind, TypeParser};

use crate::is_ignorable_map_parameter_list;

/// Rust-only: the rule's `Effect.isSuccess` message (Go spells it out at each use).
static IS_SUCCESS_MESSAGE: &Message = &tsdiag::EFFECT_IS_SUCCESS_EXPRESSES_THIS_CHECK_MORE_DIRECTLY_THAN_EFFECT_0_FOLDING_TO_COMPLEMENTARY_BOOLEAN_LITERALS_EFFECT_MATCH_BOOLEAN_TO_IS_SUCCESS;

/// Rust-only: the rule's `Effect.isFailure` message (Go spells it out at each use).
static IS_FAILURE_MESSAGE: &Message = &tsdiag::EFFECT_IS_FAILURE_EXPRESSES_THIS_CHECK_MORE_DIRECTLY_THAN_EFFECT_0_FOLDING_TO_COMPLEMENTARY_BOOLEAN_LITERALS_EFFECT_MATCH_BOOLEAN_TO_IS_SUCCESS;

// go: internal/rules/match_boolean_to_is_success.go:18 MatchBooleanToIsSuccess 932b7dce6a36de6d
// MatchBooleanToIsSuccess suggests Effect.isSuccess or Effect.isFailure when
// Effect.match or Effect.matchEager only folds the result to complementary
// boolean literals. Both keep defects and interruptions propagating, unlike the
// matchCause variants or Effect.exit, which are not reported.
pub static MATCH_BOOLEAN_TO_IS_SUCCESS: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "matchBooleanToIsSuccess",
    group: "style",
    description: "Suggests Effect.isSuccess or Effect.isFailure when Effect.match or Effect.matchEager folds to complementary boolean literals",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![IS_SUCCESS_MESSAGE.code(), IS_FAILURE_MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let matches = analyze_match_boolean_to_is_success(ctx.type_parser, Some(ctx.source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            let mut message = IS_SUCCESS_MESSAGE;
            if m.replacement_name == "isFailure" {
                message = IS_FAILURE_MESSAGE;
            }
            diagnostics.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                message,
                &[],
                &[m.match_name],
            ));
        }
        diagnostics
    },
});

// go: internal/rules/match_boolean_to_is_success.go:43 MatchBooleanToIsSuccessMatch cca1160de2972958
// MatchBooleanToIsSuccessMatch holds the nodes needed by the diagnostic and quick fix.
//
// Rust: the callee and the flow are never nil (non-optional fields).
#[derive(Clone)]
pub struct MatchBooleanToIsSuccessMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub callee: P<Node>,
    // Flow contains the match step at Index.
    pub flow: PipingFlow,
    pub index: i64,
    // PointFree is true for a standalone match({...}) call: the options are the
    // subject of Flow and the call is its first step, applied to no input.
    pub point_free: bool,
    pub match_name: &'static str,       // "match" or "matchEager"
    pub replacement_name: &'static str, // "isSuccess" or "isFailure"
}

// go: internal/rules/match_boolean_to_is_success.go:60 AnalyzeMatchBooleanToIsSuccess 2659346c380487c7
// AnalyzeMatchBooleanToIsSuccess finds Effect.match and Effect.matchEager
// calls whose inline handlers ignore their input and return complementary
// boolean literals.
//
// Rust: Go's unused checker argument is dropped (the parser holds the checker); Go's nil parser check is the type
// system's, and so is the `callee == nil` check (the callee is non-optional).
pub fn analyze_match_boolean_to_is_success(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<MatchBooleanToIsSuccessMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        for (index, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(index);
            let callee = transformation.callee();

            let options;
            let mut point_free = false;
            if transformation.args().len() == 1 {
                options = *transformation.args().at(0);
            } else if index == 0
                && transformation.kind() == TransformationKind::Call
                && transformation.args().len() == 0
            {
                // A standalone match({...}): the options are the subject of the flow.
                let Some(subject) = flow.subject().node() else {
                    continue;
                };
                options = subject;
                point_free = true;
            } else {
                continue;
            }

            let Some((on_success, on_failure)) = match_boolean_handlers(options) else {
                continue;
            };
            let Some(success_value) = boolean_literal_handler(Some(on_success)) else {
                continue;
            };
            let Some(failure_value) = boolean_literal_handler(Some(on_failure)) else {
                continue;
            };
            if success_value == failure_value {
                continue;
            }

            let match_name = if tp.is_node_reference_to_effect_module_api(callee, "match") {
                "match"
            } else if tp.is_node_reference_to_effect_module_api(callee, "matchEager") {
                "matchEager"
            } else {
                continue;
            };
            let mut replacement_name = "isFailure";
            if success_value {
                replacement_name = "isSuccess";
            }

            matches.push(MatchBooleanToIsSuccessMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, callee),
                callee,
                flow: flow.clone(),
                index,
                point_free,
                match_name,
                replacement_name,
            });
        }
    }
    matches
}

// go: internal/rules/match_boolean_to_is_success.go:130 matchBooleanHandlers 882005de929e4810
// matchBooleanHandlers returns the onSuccess and onFailure initializers of an
// inline object literal made of exactly those two property assignments.
//
// Rust: Go's `(onSuccess, onFailure, ok)` is `Option<(onSuccess, onFailure)>`.
pub(crate) fn match_boolean_handlers(options: P<Node>) -> Option<(P<Node>, P<Node>)> {
    let object = ast::skip_parentheses(options);
    if object.kind() != Kind::ObjectLiteralExpression {
        return None;
    }
    let properties = object.as_object_literal_expression().properties().nodes();
    if properties.len() != 2 {
        return None;
    }
    for property in properties {
        if property.kind() != Kind::PropertyAssignment {
            return None;
        }
    }
    let on_success = ets_typeparser::object_literal_property_initializer(object, "onSuccess");
    let on_failure = ets_typeparser::object_literal_property_initializer(object, "onFailure");
    Some((on_success?, on_failure?))
}

// go: internal/rules/match_boolean_to_is_success.go:151 booleanLiteralHandler e7800bbfb724146e
// booleanLiteralHandler accepts an expression-bodied arrow function that
// ignores its input and returns a bare (possibly parenthesized) boolean literal.
//
// Rust: Go's `(value, ok)` is `Option<value>`; the lazy expression's body and expression are non-optional, so
// their nil checks drop.
pub(crate) fn boolean_literal_handler(handler: Option<P<Node>>) -> Option<bool> {
    let handler = handler?;
    if ast::skip_parentheses(handler).kind() != Kind::ArrowFunction {
        return None;
    }
    let lazy = ets_typeparser::parse_lazy_expression(handler, LazyExpressionFlags::None)?;
    if lazy.body().kind() == Kind::Block || !is_ignorable_map_parameter_list(lazy.params()) {
        return None;
    }
    match ast::skip_parentheses(lazy.expression()).kind() {
        Kind::TrueKeyword => return Some(true),
        Kind::FalseKeyword => return Some(false),
        _ => {}
    }
    None
}
