use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Node, SourceFile};
use ets_shim::checker::{SignatureKind, Type, TypeFlags};
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{PipingFlowTransformation, TypeParser};

use crate::*;

// go: internal/rules/multiple_catch_tag.go:16 MultipleCatchTag 0bd9d252458e761d
pub static MULTIPLE_CATCH_TAG: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
        name: "multipleCatchTag",
        group: "style",
        description: "Suggests collapsing consecutive Effect.catchTag transformations into a single Effect.catchTags call when semantics stay equivalent",
        default_severity: Severity::Suggestion,
        supported_effect: vec!["v4"],
        codes: vec![
            tsdiag::THESE_0_CONSECUTIVE_CATCH_TAG_TRANSFORMATIONS_CAN_BE_COLLAPSED_INTO_A_SINGLE_CATCH_TAGS_CALL_EFFECT_MULTIPLE_CATCH_TAG.code(),
        ],
        run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
            let matches = analyze_multiple_catch_tag(ctx.type_parser, Some(ctx.source_file));
            let mut diags = Vec::with_capacity(matches.len());
            for m in &matches {
                let related = multiple_catch_tag_related_information(ctx, m);
                diags.push(ctx.new_diagnostic(
                    m.source_file,
                    m.location,
                    &tsdiag::THESE_0_CONSECUTIVE_CATCH_TAG_TRANSFORMATIONS_CAN_BE_COLLAPSED_INTO_A_SINGLE_CATCH_TAGS_CALL_EFFECT_MULTIPLE_CATCH_TAG,
                    &related,
                    &[&m.catch_nodes.len().to_string()],
                ));
            }
            diags
        },
    }
});

// go: internal/rules/multiple_catch_tag.go:41 MultipleCatchTagMatch 3c52fa6a586c2328
#[derive(Clone)]
pub struct MultipleCatchTagMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub node: P<Node>,
    pub catch_nodes: Vec<P<Node>>,
}

// go: internal/rules/multiple_catch_tag.go:48 multipleCatchTagCandidate f0bfc9063fb2f957
//
// Rust: Go's `*typeparser.PipingFlowTransformation` points into the flow being scanned, so it is a borrow.
pub(crate) struct MultipleCatchTagCandidate<'a> {
    pub(crate) transformation: &'a PipingFlowTransformation,
    pub(crate) handled_tag_type: P<Type>,
    pub(crate) introduced_tags: Vec<P<Type>>,
}

// go: internal/rules/multiple_catch_tag.go:54 AnalyzeMultipleCatchTag 546d1d72eb3e0ff3
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`), so it is dropped; Go's nil checks of the
// parser and checker cannot fail.
pub fn analyze_multiple_catch_tag(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<MultipleCatchTagMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.as_deref().unwrap_or_default() {
        let mut chain: Vec<MultipleCatchTagCandidate<'_>> = Vec::new();
        let mut introduced_tags: Vec<P<Type>> = Vec::new();

        // Rust: Go's `flush` closure over the loop's state.
        fn flush(
            sf: P<SourceFile>,
            matches: &mut Vec<MultipleCatchTagMatch>,
            chain: &mut Vec<MultipleCatchTagCandidate<'_>>,
            introduced_tags: &mut Vec<P<Type>>,
        ) {
            if chain.len() >= 2 {
                matches.push(MultipleCatchTagMatch {
                    source_file: sf,
                    location: ets_shim::scanner::get_error_range_for_node(
                        sf,
                        chain[0].transformation.callee(),
                    ),
                    node: chain[0].transformation.callee(),
                    catch_nodes: collect_multiple_catch_tag_nodes(chain),
                });
            }
            chain.clear();
            introduced_tags.clear();
        }

        for (i, _) in flow.transformations().all() {
            let transformation = flow.transformations().at(i);
            if !tp.is_node_reference_to_effect_package_export(transformation.callee(), "catchTag") {
                flush(sf, &mut matches, &mut chain, &mut introduced_tags);
                continue;
            }

            let Some(candidate) = analyze_multiple_catch_tag_candidate(tp, Some(transformation))
            else {
                flush(sf, &mut matches, &mut chain, &mut introduced_tags);
                continue;
            };

            if overlaps_any_tag_type(tp, Some(candidate.handled_tag_type), &introduced_tags) {
                flush(sf, &mut matches, &mut chain, &mut introduced_tags);
            }

            introduced_tags.extend_from_slice(&candidate.introduced_tags);
            chain.push(candidate);
        }

        flush(sf, &mut matches, &mut chain, &mut introduced_tags);
    }

    matches
}

// go: internal/rules/multiple_catch_tag.go:104 analyzeMultipleCatchTagCandidate bfae1b4b99765b85
//
// Rust: Go's `(candidate, ok)` is an `Option`; the checker is the parser's.
pub(crate) fn analyze_multiple_catch_tag_candidate<'a>(
    tp: &mut TypeParser<'_>,
    transformation: Option<&'a PipingFlowTransformation>,
) -> Option<MultipleCatchTagCandidate<'a>> {
    let transformation = transformation?;
    if transformation.args().len() != 2 {
        return None;
    }

    let tag_arg = unwrap_transparent_expression(Some(*transformation.args().at(0)))?;
    if !ast::is_string_literal(tag_arg) {
        return None;
    }

    let handled_tag_type = tp.get_type_at_location(tag_arg)?;

    let handler_type = tp.get_type_at_location(*transformation.args().at(1))?;

    let signatures = tp.checker().get_signatures_of_type(handler_type, SignatureKind::Call);
    if signatures.len() != 1 {
        return None;
    }

    let return_type = tp.checker().get_return_type_of_signature(signatures[0]);

    let error_channel = catch_tag_return_error_channel(tp, Some(return_type))?;

    Some(MultipleCatchTagCandidate {
        transformation,
        handled_tag_type,
        introduced_tags: collect_error_tag_types(tp, Some(error_channel)),
    })
}

// go: internal/rules/multiple_catch_tag.go:146 catchTagReturnErrorChannel 896e0f723fcd32e6
pub(crate) fn catch_tag_return_error_channel(
    tp: &mut TypeParser<'_>,
    return_type: Option<P<Type>>,
) -> Option<P<Type>> {
    let return_type = return_type?;
    if let Some(effect_type) = tp.effect_type(return_type) {
        return Some(effect_type.e());
    }
    if let Some(stream_type) = tp.stream_type(return_type) {
        return Some(stream_type.e());
    }
    None
}

// go: internal/rules/multiple_catch_tag.go:159 collectErrorTagTypes dcc2c3a4d600adf5
//
// Rust: the checker is the parser's.
pub(crate) fn collect_error_tag_types(
    tp: &mut TypeParser<'_>,
    error_type: Option<P<Type>>,
) -> Vec<P<Type>> {
    let mut tags = Vec::new();
    let Some(error_type) = error_type else {
        return tags;
    };

    for member in tp.unroll_union_members(error_type).unwrap_or_default() {
        if member.flags().intersects(TypeFlags::Never) {
            continue;
        }

        let mut tag_type = tp.checker().get_type_of_property_of_type_exported(member, "_tag");
        if tag_type.is_none() {
            tag_type = tp.get_type_of_property_by_name(member, "_tag");
        }
        let Some(tag_type) = tag_type else {
            continue;
        };
        tags.push(tag_type);
    }

    tags
}

// go: internal/rules/multiple_catch_tag.go:183 overlapsAnyTagType 098b161c51172d87
//
// Rust: the checker is the parser's.
pub(crate) fn overlaps_any_tag_type(
    tp: &mut TypeParser<'_>,
    current: Option<P<Type>>,
    previous: &[P<Type>],
) -> bool {
    let Some(current) = current else {
        return false;
    };

    for &prior in previous {
        if tp.checker().is_type_assignable_to(current, prior)
            && tp.checker().is_type_assignable_to(prior, current)
        {
            return true;
        }
    }

    false
}

// go: internal/rules/multiple_catch_tag.go:200 collectMultipleCatchTagNodes e1fb5a1ead8cec96
pub(crate) fn collect_multiple_catch_tag_nodes(
    chain: &[MultipleCatchTagCandidate<'_>],
) -> Vec<P<Node>> {
    let mut nodes = Vec::with_capacity(chain.len());
    for candidate in chain {
        nodes.push(candidate.transformation.callee());
    }
    nodes
}

// go: internal/rules/multiple_catch_tag.go:211 multipleCatchTagRelatedInformation ef87f456a3fb88d7
pub(crate) fn multiple_catch_tag_related_information(
    ctx: &Context<'_, '_>,
    m: &MultipleCatchTagMatch,
) -> Vec<P<Diagnostic>> {
    if m.catch_nodes.len() < 2 {
        return Vec::new();
    }

    let mut related = Vec::with_capacity(m.catch_nodes.len() - 1);
    for &node in &m.catch_nodes[1..] {
        related.push(ctx.new_diagnostic(
            m.source_file,
            ets_shim::scanner::get_error_range_for_node(m.source_file, node),
            &tsdiag::THIS_CATCH_TAG_TRANSFORMATION_IS_PART_OF_A_CONSECUTIVE_CHAIN_THAT_CAN_BE_COLLAPSED_INTO_CATCH_TAGS_EFFECT_MULTIPLE_CATCH_TAG,
            &[],
            &[],
        ));
    }
    related
}
