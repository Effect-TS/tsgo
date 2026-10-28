use std::sync::LazyLock;

use ets_etscore::{ResolvedEffectPluginOptions, Severity};
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Node, SourceFile};
use ets_shim::checker::SignatureKind;
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{PipingFlow, PipingFlowTransformation, TransformationKind, TypeParser};

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_NESTED_CALL_STRUCTURE_HAS_A_PIPEABLE_FORM_0_PIPE_REPRESENTS_THE_SAME_CALL_SEQUENCE_IN_PIPE_STYLE_AND_MAY_BE_EASIER_TO_READ_EFFECT_MISSED_PIPEABLE_OPPORTUNITY;

// go: internal/rules/missed_pipeable_opportunity.go:19 MissedPipeableOpportunity d96dffe723cf29e9
// MissedPipeableOpportunity detects nested function call chains that can be converted to .pipe() style.
pub static MISSED_PIPEABLE_OPPORTUNITY: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "missedPipeableOpportunity",
    group: "style",
    description: "Suggests using .pipe() for nested function calls",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let mut min_arg_count = 2;
        let effect_config = ctx.options;
        if effect_config.is_some() {
            min_arg_count = ResolvedEffectPluginOptions::get_pipeable_min_arg_count(effect_config);
        }

        let matches =
            analyze_missed_pipeable_opportunity(ctx.type_parser, ctx.source_file, min_arg_count);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[&m.subject_text],
            ));
        }
        diags
    },
});

// go: internal/rules/missed_pipeable_opportunity.go:50 MissedPipeableOpportunityMatch 694ebc31095f1e5d
// MissedPipeableOpportunityMatch holds the parsed result needed by both the diagnostic rule
// and the quick-fix for the missedPipeableOpportunity pattern.
//
// Rust: `flow` is a copy of the flow (Go's pointer into the cached flows), and `after_transformations` owns the
// transformations Go's read-only view points at (`None` for Go's nil view).
#[derive(Clone)]
pub struct MissedPipeableOpportunityMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub pipeable_start_index: i64,
    pub pipeable_transformations: Vec<PipingFlowTransformation>,
    pub flow: PipingFlow,
    pub subject_text: String,
    pub after_transformations: Option<Vec<PipingFlowTransformation>>,
}

// go: internal/rules/missed_pipeable_opportunity.go:61 AnalyzeMissedPipeableOpportunity d26849fe1b7bce8b
// AnalyzeMissedPipeableOpportunity finds all nested call chains that can be converted to .pipe() style.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub fn analyze_missed_pipeable_opportunity(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    min_arg_count: i64,
) -> Vec<MissedPipeableOpportunityMatch> {
    let flows = tp.piping_flows(sf, false);

    let mut matches = Vec::new();

    for flow in flows.as_deref().unwrap_or_default() {
        // Skip flows with too few transformations
        if flow.transformations().len() < min_arg_count {
            continue;
        }

        // Skip if final output type is callable (has call signatures)
        let last_transformation = flow.transformations().at(flow.transformations().len() - 1);
        if let Some(out_type) = last_transformation.out_type(tp) {
            let call_sigs = tp.checker().get_signatures_of_type(out_type, SignatureKind::Call);
            if !call_sigs.is_empty() {
                continue;
            }
        }

        let transformation_count = flow.transformations().len();
        let runs = safely_pipeable_runs(tp, flow);
        for first_pipeable_index in 0..=transformation_count {
            let run = runs[first_pipeable_index as usize];
            if run.call_count < min_arg_count
                || !is_pipeable_at_index(tp, flow, first_pipeable_index)
            {
                continue;
            }
            let mut pipeable_transformations = Vec::new();
            for i in first_pipeable_index..first_pipeable_index + run.length {
                pipeable_transformations.push(flow.transformations().at(i).clone());
            }
            let pipeable_end_index = first_pipeable_index + pipeable_transformations.len() as i64;

            // Get subject text for the diagnostic message
            let subject_text = get_subject_text(Some(sf), flow, first_pipeable_index);

            let after_transformations = flow
                .transformations()
                .slice(pipeable_end_index, flow.transformations().len())
                .copy();

            matches.push(MissedPipeableOpportunityMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, flow.node()),
                pipeable_start_index: first_pipeable_index,
                pipeable_transformations,
                flow: flow.clone(),
                subject_text,
                after_transformations,
            });

            // Found and reported a valid segment, move past it (no overlapping diagnostics)
            break;
        }
    }

    matches
}

// go: internal/rules/missed_pipeable_opportunity.go:117 safelyPipeableRun 8da24950a3b99dec
#[derive(Clone, Copy, Default)]
pub(crate) struct SafelyPipeableRun {
    pub(crate) length: i64,
    pub(crate) call_count: i64,
}

// go: internal/rules/missed_pipeable_opportunity.go:122 safelyPipeableRuns 98f5683b7e1b2f1b
pub(crate) fn safely_pipeable_runs(
    tp: &mut TypeParser<'_>,
    flow: &PipingFlow,
) -> Vec<SafelyPipeableRun> {
    let transformation_count = flow.transformations().len();
    let mut runs = vec![SafelyPipeableRun::default(); transformation_count as usize + 1];
    let mut i = transformation_count - 1;
    while i >= 0 {
        let t = flow.transformations().at(i);
        if !tp.is_safely_pipeable_callee(t.callee()) {
            i -= 1;
            continue;
        }
        let next = runs[i as usize + 1];
        runs[i as usize] =
            SafelyPipeableRun { length: next.length + 1, call_count: next.call_count };
        if t.kind() == TransformationKind::Call {
            runs[i as usize].call_count += 1;
        }
        i -= 1;
    }
    runs
}

// go: internal/rules/missed_pipeable_opportunity.go:140 isPipeableAtIndex 933d04bdf025ee66
// isPipeableAtIndex checks if the type at a given index in a flow is pipeable.
// Index 0 = subject, index > 0 = transformations[index - 1].outType
pub(crate) fn is_pipeable_at_index(tp: &mut TypeParser<'_>, flow: &PipingFlow, index: i64) -> bool {
    if index == 0 {
        if tp.literal_cannot_be_pipeable(flow.subject().node()) {
            return false;
        }
        let Some(subject_type) = flow.subject().out_type(tp) else {
            return false;
        };
        return tp.is_pipeable_type(subject_type);
    }

    let t = flow.transformations().at(index - 1);
    let Some(out_type) = t.out_type(tp) else {
        return false;
    };
    tp.is_pipeable_type(out_type)
}

// go: internal/rules/missed_pipeable_opportunity.go:162 getSubjectText 2f4d633f0079d4a1
// getSubjectText extracts the subject text for the diagnostic message.
// If the pipeable segment starts at index 0, uses the subject node text directly.
// Otherwise, traverses the flow node to find the node at the right depth.
//
// Rust: Go's unused checker argument is dropped.
pub(crate) fn get_subject_text(
    sf: Option<P<SourceFile>>,
    flow: &PipingFlow,
    first_pipeable_index: i64,
) -> String {
    if first_pipeable_index == 0 {
        return trimmed_node_text(sf, flow.subject().node());
    }

    // Traverse from flow.node into arguments to find the node at the right depth
    let mut current = flow.node();
    let mut i = flow.transformations().len();
    while i > first_pipeable_index {
        let t = flow.transformations().at(i - 1);
        if t.kind() == TransformationKind::Call && ast::is_call_expression(current) {
            let call = current.as_call_expression();
            let arguments = call.arguments().nodes();
            if !arguments.is_empty() {
                current = arguments[0];
            } else {
                return String::new();
            }
        } else {
            return String::new();
        }
        i -= 1;
    }
    trimmed_node_text(sf, Some(current))
}

// go: internal/rules/missed_pipeable_opportunity.go:186 trimmedNodeText fbfa3a4a1c8bc526
// trimmedNodeText extracts the source text of a node, trimmed of leading trivia.
pub(crate) fn trimmed_node_text(sf: Option<P<SourceFile>>, node: Option<P<Node>>) -> String {
    let (Some(node), Some(sf)) = (node, sf) else {
        return String::new();
    };
    let text = sf.text();
    let pos = i64::from(ets_shim::scanner::get_token_pos_of_node(node, sf, false));
    let end = go_end(node);
    if pos >= 0 && end >= pos && end <= text.len() as i64 {
        return ets_go::strings::trim_space(&text[pos as usize..end as usize]).to_string();
    }
    String::new()
}
