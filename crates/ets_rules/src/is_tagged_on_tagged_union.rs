use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Node, SourceFile};
use ets_shim::checker::{ContextFlags, LiteralValue, SignatureKind, Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{PipingFlow, TypeParser};

use crate::*;

/// Rust-only: the rule's message for a tag of the union (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::PREDICATE_IS_TAGGED_ACCEPTS_ANY_VALUE_SO_ON_THIS_TAGGED_UNION_IT_NEITHER_CHECKS_THE_TAG_AGAINST_THE_MEMBERS_NOR_NARROWS_AS_A_TAG_COMPARISON_DOES_EFFECT_IS_TAGGED_ON_TAGGED_UNION;

/// Rust-only: the rule's message for a tag outside the union (Go spells it out at each use).
static UNKNOWN_TAG_MESSAGE: &Message = &tsdiag::X_0_IS_NOT_A_TAG_OF_THIS_TAGGED_UNION_SO_THIS_PREDICATE_IS_TAGGED_TEST_IS_ALWAYS_FALSE_EFFECT_IS_TAGGED_ON_TAGGED_UNION;

// go: internal/rules/is_tagged_on_tagged_union.go:19 IsTaggedOnTaggedUnion 6017daf2aaa392e6
// IsTaggedOnTaggedUnion reports Predicate.isTagged on a value whose type is
// already a union of tagged members. isTagged accepts unknown, so a misspelled
// tag compiles, a member with no field but its _tag survives a failed test, and
// a refinement passed as a callback replaces the element type with { _tag: K }.
// A `_tag` comparison has none of these problems.
pub static IS_TAGGED_ON_TAGGED_UNION: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "isTaggedOnTaggedUnion",
    group: "antipattern",
    description: "Reports Predicate.isTagged on a value already typed as a tagged union, where a _tag comparison checks the tag and narrows",
    default_severity: Severity::Warning,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code(), UNKNOWN_TAG_MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let matches = analyze_is_tagged_on_tagged_union(ctx.type_parser, Some(ctx.source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            if m.known_tag {
                diagnostics.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
            } else {
                diagnostics.push(ctx.new_diagnostic(
                    m.source_file,
                    m.location,
                    UNKNOWN_TAG_MESSAGE,
                    &[],
                    &[m.tag.as_str()],
                ));
            }
        }
        diagnostics
    },
});

// go: internal/rules/is_tagged_on_tagged_union.go:44 IsTaggedOnTaggedUnionMatch 5f9c61bfddf17e3e
// IsTaggedOnTaggedUnionMatch holds the nodes needed by the diagnostic and quick fix.
//
// Rust: the flow is never nil (a non-optional field).
#[derive(Clone)]
pub struct IsTaggedOnTaggedUnionMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    // Flow contains the isTagged step at Index.
    pub flow: PipingFlow,
    pub index: i64,
    pub tag: String,
    // KnownTag is false when Tag names no member of the union.
    pub known_tag: bool,
    // Applied is true for isTagged(x, tag), isTagged(tag)(x) and
    // pipe(x, isTagged(tag)), false for isTagged(tag) passed as a callback.
    pub applied: bool,
    // Module is the Effect module that declares the union's members (see
    // TypeParser.TaggedDataTypeModule), or "".
    pub module: &'static str,
}

// go: internal/rules/is_tagged_on_tagged_union.go:63 AnalyzeIsTaggedOnTaggedUnion acfe4d314a33ebab
// AnalyzeIsTaggedOnTaggedUnion finds Predicate.isTagged steps whose tested
// value is typed as a union of members with string literal tags.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`), never nil.
pub fn analyze_is_tagged_on_tagged_union(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<IsTaggedOnTaggedUnionMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        for (index, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(index).clone();
            if transformation.args().len() > 1
                || !tp.is_node_reference_to_effect_predicate_module_api(
                    transformation.callee(),
                    "isTagged",
                )
            {
                continue;
            }
            let applied = transformation.args().len() == 1;
            let (tag_type, value_type) = if applied {
                let tag_type = tp.get_type_at_location(*transformation.args().at(0));
                (tag_type, PipingFlow::transformation_input_type(Some(flow), tp, index))
            } else {
                // isTagged(tag) alone takes the tag as its input and returns a
                // refinement, which tests the parameter of the callback it is passed as.
                let tag_type = PipingFlow::transformation_input_type(Some(flow), tp, index);
                (tag_type, is_tagged_callback_parameter_type(tp, transformation.callee()))
            };
            let Some(tag_type) = tag_type else {
                continue;
            };
            if !tag_type.flags().intersects(TypeFlags::StringLiteral) {
                continue;
            }
            let Some(LiteralValue::String(tag)) = tag_type.as_literal_type().value() else {
                continue;
            };
            let Some(tags) = literal_tagged_union_type_tags(tp, value_type) else {
                continue;
            };
            let known = tags.contains(tag);
            matches.push(IsTaggedOnTaggedUnionMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, transformation.callee()),
                flow: flow.clone(),
                index,
                tag: tag.to_string(),
                known_tag: known,
                applied,
                module: tp.tagged_data_type_module(value_type),
            });
        }
    }
    matches
}

// go: internal/rules/is_tagged_on_tagged_union.go:112 isTaggedCallbackParameterType a0fc0ef091176e19
// isTaggedCallbackParameterType returns the first parameter type of the
// callback signature an unapplied isTagged(tag) call is contextually typed by.
pub(crate) fn is_tagged_callback_parameter_type(
    tp: &mut TypeParser<'_>,
    callee: P<Node>,
) -> Option<P<Type>> {
    let call = callee.parent()?;
    if !ast::is_call_expression(call) || call.as_call_expression().expression() != callee {
        return None;
    }
    let contextual_type = tp.get_contextual_type(call, ContextFlags::None)?;
    let signatures =
        tp.checker().get_signatures_of_type_exported(contextual_type, SignatureKind::Call);
    if signatures.len() != 1 || signatures[0].parameters().is_empty() {
        return None;
    }
    Some(tp.checker().get_type_at_position_exported(signatures[0], 0))
}
