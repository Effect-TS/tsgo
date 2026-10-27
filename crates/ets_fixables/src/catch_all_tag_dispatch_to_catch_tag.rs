use std::sync::LazyLock;

use ets_fixable::{Context, FixAction, Fixable};
use ets_rewriter::{PipingFlowTransformationReplacement, Tracker};
use ets_rules::{CatchAllTagDispatchBranch, CatchAllTagDispatchMatch};
use ets_shim::P;
use ets_shim::ast::{Kind, Node, NodeList, SourceFile, TokenFlags};
use ets_shim::core::{LanguageVariant, alloc_str};
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_shim::ls::CodeAction;
use ets_shim::scanner;

/// Rust-only: the fixable's message (Go spells it out in `ErrorCodes`).
static MESSAGE: &Message = &tsdiag::BRANCHING_ON_0_TAG_INSIDE_EFFECT_1_HAND_ROLLS_TAGGED_ERROR_DISPATCH_USE_EFFECT_CATCH_TAG_OR_EFFECT_CATCH_TAGS_WHICH_RE_FAIL_UNMATCHED_ERRORS_AUTOMATICALLY_EFFECT_CATCH_ALL_TAG_DISPATCH_TO_CATCH_TAG;

// go: internal/fixables/catch_all_tag_dispatch_to_catch_tag.go:15 CatchAllTagDispatchToCatchTagFix
pub static CATCH_ALL_TAG_DISPATCH_TO_CATCH_TAG_FIX: LazyLock<Fixable> = LazyLock::new(|| Fixable {
    name: "catchAllTagDispatchToCatchTag",
    description: "Replace hand-rolled tagged error dispatch with Effect.catchTag or Effect.catchTags",
    error_codes: vec![MESSAGE.code()],
    fix_ids: vec!["catchAllTagDispatchToCatchTag_fix"],
    run: run_catch_all_tag_dispatch_to_catch_tag_fix,
});

// go: internal/fixables/catch_all_tag_dispatch_to_catch_tag.go:21 runCatchAllTagDispatchToCatchTagFix
fn run_catch_all_tag_dispatch_to_catch_tag_fix(ctx: &mut Context<'_, '_>) -> Vec<CodeAction> {
    let source_file = ctx.source_file;
    for m in ets_rules::analyze_catch_all_tag_dispatch_to_catch_tag(
        &mut ctx.type_parser,
        Some(source_file),
    ) {
        if !m.can_fix || (!m.location.intersects(ctx.span) && !ctx.span.contained_by(m.location)) {
            continue;
        }
        let action = ctx.new_fix_action(FixAction {
            description: "Replace with Effect.catchTag or Effect.catchTags".to_string(),
            run: Box::new(move |tracker| {
                let (callee, arguments) =
                    build_catch_all_tag_dispatch_replacement(tracker, source_file, &m);
                if callee.is_some() && arguments.is_some() {
                    tracker.replace_piping_flow_transformation(
                        Some(source_file),
                        Some(&m.transformation),
                        PipingFlowTransformationReplacement {
                            callee,
                            type_arguments: None,
                            arguments,
                        },
                    );
                }
            }),
        });
        if let Some(action) = action {
            return vec![action];
        }
    }
    Vec::new()
}

// go: internal/fixables/catch_all_tag_dispatch_to_catch_tag.go:39 buildCatchAllTagDispatchReplacement
//
// Rust: the tracker and the match's callee cannot be nil, so only Go's empty-branches check remains.
fn build_catch_all_tag_dispatch_replacement(
    tracker: &Tracker,
    sf: P<SourceFile>,
    m: &CatchAllTagDispatchMatch,
) -> (Option<P<Node>>, Option<P<NodeList>>) {
    if m.branches.is_empty() {
        return (None, None);
    }
    let mut method_name = "catchTag";
    if m.branches.len() > 1 {
        method_name = "catchTags";
    }
    let callee = tracker.new_property_access_path(
        &ets_typeparser::find_effect_module_reference(Some(sf)).append(&[method_name]),
    );
    if m.branches.len() == 1 {
        let branch = &m.branches[0];
        return (
            Some(callee),
            Some(tracker.new_node_list(vec![
                tracker.new_string_literal(alloc_str(&branch.tag), TokenFlags::None),
                new_catch_tag_handler(tracker, &m.parameter_name, branch),
            ])),
        );
    }
    let mut properties = Vec::with_capacity(m.branches.len());
    for branch in &m.branches {
        let mut name = tracker.new_string_literal(alloc_str(&branch.tag), TokenFlags::None);
        if scanner::is_identifier_text(&branch.tag, LanguageVariant::Standard) {
            name = tracker.new_identifier(alloc_str(&branch.tag));
        }
        properties.push(tracker.new_property_assignment(
            None,
            name,
            None,
            None,
            new_catch_tag_handler(tracker, &m.parameter_name, branch),
        ));
    }
    (
        Some(callee),
        Some(tracker.new_node_list(vec![
            tracker.new_object_literal_expression(tracker.new_node_list(properties), false),
        ])),
    )
}

// go: internal/fixables/catch_all_tag_dispatch_to_catch_tag.go:63 newCatchTagHandler
fn new_catch_tag_handler(
    tracker: &Tracker,
    parameter_name: &str,
    branch: &CatchAllTagDispatchBranch,
) -> P<Node> {
    let mut parameters = Vec::new();
    if branch.uses_parameter {
        parameters = vec![tracker.new_parameter_declaration(
            None,
            None,
            tracker.new_identifier(alloc_str(parameter_name)),
            None,
            None,
            None,
        )];
    }
    tracker.new_arrow_function(
        None,
        None,
        Some(tracker.new_node_list(parameters)),
        None,
        None,
        Some(tracker.new_token(Kind::EqualsGreaterThanToken)),
        tracker.deep_clone_node(Some(branch.result)),
    )
}
