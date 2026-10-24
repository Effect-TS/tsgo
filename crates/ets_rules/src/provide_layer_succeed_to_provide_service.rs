use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    EffectMajorVersion, PipingFlow, PipingFlowSubject, PipingFlowTransformation, TypeParser,
};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::Effect_0_provides_this_inline_single_service_layer_directly_effect_provideLayerSucceedToProvideService;

// go: internal/rules/provide_layer_succeed_to_provide_service.go:14 ProvideLayerSucceedToProvideService
pub static PROVIDE_LAYER_SUCCEED_TO_PROVIDE_SERVICE: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "provideLayerSucceedToProvideService",
    group: "style",
    description: "Suggests providing inline Layer.succeed and Layer.effect services directly",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches =
            analyze_provide_layer_succeed_to_provide_service(ctx.type_parser, Some(source_file));
        let mut diagnostics = Vec::with_capacity(matches.len());
        for m in &matches {
            diagnostics.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[m.replacement_method_name],
            ));
        }
        diagnostics
    },
});

// go: internal/rules/provide_layer_succeed_to_provide_service.go:39 ProvideLayerSucceedToProvideServiceMatch
//
// Rust: Go's `*PipingFlowTransformation` (a pointer to a local copy) is an owned copy.
#[derive(Clone)]
pub struct ProvideLayerSucceedToProvideServiceMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub provide_transformation: PipingFlowTransformation,
    pub effect_module_node: Option<P<Node>>,
    pub service_node: P<Node>,
    pub implementation_node: Option<P<Node>>,
    pub replacement_method_name: &'static str,
}

// go: internal/rules/provide_layer_succeed_to_provide_service.go:49 AnalyzeProvideLayerSucceedToProvideService
//
// Rust: Go's checker argument is the parser's checker; Go ignores it, so it is dropped.
pub fn analyze_provide_layer_succeed_to_provide_service(
    tp: &mut TypeParser<'_>,
    sf: Option<P<SourceFile>>,
) -> Vec<ProvideLayerSucceedToProvideServiceMatch> {
    let Some(sf) = sf else {
        return Vec::new();
    };
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }

    let mut matches = Vec::new();
    let flows = tp.piping_flows(sf, true);
    for flow in flows.iter().flat_map(|flows| flows.iter()) {
        for (index, _) in flow.transformations().all() {
            let provide = flow.transformations().at(index).clone();
            if provide.args().len() != 1
                || !tp.is_node_reference_to_effect_module_api(provide.callee(), "provide")
            {
                continue;
            }

            let Some((service, implementation, replacement)) =
                inline_single_service_layer(tp, *provide.args().at(0))
            else {
                continue;
            };
            if tp.node_cannot_be_effect_library_type(service) {
                continue;
            }
            let service_type = tp.get_type_at_location(service);
            if service_type.is_none()
                || (!tp.is_service_type(service_type) && !tp.is_context_tag(service_type))
            {
                continue;
            }

            let mut effect_module_node = None;
            if provide.callee().kind() == Kind::PropertyAccessExpression {
                effect_module_node =
                    Some(provide.callee().as_property_access_expression().expression());
            }
            matches.push(ProvideLayerSucceedToProvideServiceMatch {
                source_file: sf,
                location: ets_shim::scanner::get_error_range_for_node(sf, provide.callee()),
                provide_transformation: provide,
                effect_module_node,
                service_node: service,
                implementation_node: implementation,
                replacement_method_name: replacement,
            });
        }
    }
    matches
}

// go: internal/rules/provide_layer_succeed_to_provide_service.go:90 inlineSingleServiceLayer
//
// Rust: Go's `(service, implementation, replacement, ok)` is an `Option`.
pub(crate) fn inline_single_service_layer(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
) -> Option<(P<Node>, Option<P<Node>>, &'static str)> {
    let node = ast::skip_parentheses(node);
    let flow = tp.longest_piping_flow_at(node, false);
    if flow.as_ref().is_none_or(|flow| flow.node() != node)
        || !PipingFlow::matches_exactly(
            flow.as_ref(),
            tp,
            &|_: &mut TypeParser<'_>, subject: &PipingFlowSubject| subject.node().is_some(),
            &[&|_: &mut TypeParser<'_>, transformation: &PipingFlowTransformation| {
                transformation.args().len() == 1
                    && transformation.type_arguments().is_none_or(|args| args.nodes().is_empty())
            }],
        )
    {
        return None;
    }
    let flow = flow?;
    let transformation = flow.transformations().at(0);
    let replacement = if tp
        .is_node_reference_to_effect_layer_module_api(transformation.callee(), "succeed")
    {
        "provideService"
    } else if tp.is_node_reference_to_effect_layer_module_api(transformation.callee(), "effect") {
        "provideServiceEffect"
    } else {
        return None;
    };
    Some((*transformation.args().at(0), flow.subject().node(), replacement))
}
