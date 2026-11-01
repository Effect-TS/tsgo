use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{LazyExpressionFlags, PipingFlow, TypeParser};

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

// go: internal/rules/match_boolean_to_is_success.go:43 MatchBooleanToIsSuccessMatch 2d25367fac14986d
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
    pub match_name: &'static str,       // "match" or "matchEager"
    pub replacement_name: &'static str, // "isSuccess" or "isFailure"
}

// go: internal/rules/match_boolean_to_is_success.go:57 AnalyzeMatchBooleanToIsSuccess d94afe611676f4e0
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

            if transformation.args().len() != 1 {
                continue;
            }
            let options = *transformation.args().at(0);
            let Some(success_value) = boolean_literal_handler(
                ets_typeparser::object_literal_property_initializer(options, "onSuccess"),
            ) else {
                continue;
            };
            let Some(failure_value) = boolean_literal_handler(
                ets_typeparser::object_literal_property_initializer(options, "onFailure"),
            ) else {
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
                match_name,
                replacement_name,
            });
        }
    }
    matches
}

// go: internal/rules/match_boolean_to_is_success.go:110 booleanLiteralHandler 24b0c4628824c0a0
// booleanLiteralHandler accepts a lazy expression (see ParseLazyExpression)
// that ignores its input and returns a bare (possibly parenthesized) boolean
// literal.
//
// Rust: Go's `(value, ok)` is `Option<value>`.
pub(crate) fn boolean_literal_handler(handler: Option<P<Node>>) -> Option<bool> {
    let handler = handler?;
    let lazy = ets_typeparser::parse_lazy_expression(handler, LazyExpressionFlags::None)?;
    if !is_ignorable_map_parameter_list(lazy.params()) {
        return None;
    }
    match ast::skip_parentheses(lazy.expression()).kind() {
        Kind::TrueKeyword => return Some(true),
        Kind::FalseKeyword => return Some(false),
        _ => {}
    }
    None
}
