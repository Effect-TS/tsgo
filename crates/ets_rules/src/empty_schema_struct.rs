use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{
    PipingFlow, PipingFlowSubject, PipingFlowTransformation, TransformationKind, TypeParser,
};
use rustc_hash::FxHashMap;

/// Rust-only: the rule's general message (Go spells it out at each use).
static MESSAGE_EMPTY_STRUCT: &Message = &tsdiag::SCHEMA_STRUCT_MODELS_THE_TYPE_SO_ANY_NON_NULL_VALUE_PASSES_DECODING_UNVALIDATED_USE_AN_OBJECT_KEYWORD_SCHEMA_SCHEMA_OBJECT_KEYWORD_IN_V4_SCHEMA_OBJECT_IN_V3_FOR_ANY_OBJECT_A_RECORD_SCHEMA_FOR_OPEN_KEY_SLASHVALUE_MAPS_OR_ADD_THE_INTENDED_FIELDS_EFFECT_EMPTY_SCHEMA_STRUCT;

/// Rust-only: the rule's tool parameters message (Go spells it out at each use).
static MESSAGE_TOOL_PARAMETERS: &Message = &tsdiag::SCHEMA_STRUCT_AS_TOOL_PARAMETERS_BREAKS_STRUCTURED_OUTPUT_SCHEMA_GENERATION_FOR_SOME_PROVIDERS_USE_TOOL_EMPTY_PARAMS_INSTEAD_EFFECT_EMPTY_SCHEMA_STRUCT;

// go: internal/rules/empty_schema_struct.go:17 EmptySchemaStruct c429e81197d84b1b
// EmptySchemaStruct detects Schema.Struct({}) calls. The schema models the
// type {}, so decoding accepts any non-null value without validating it.
pub static EMPTY_SCHEMA_STRUCT: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "emptySchemaStruct",
    group: "correctness",
    description: "Detects Schema.Struct({}), which accepts any non-null value without validation",
    default_severity: Severity::Warning,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE_EMPTY_STRUCT.code(), MESSAGE_TOOL_PARAMETERS.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_empty_schema_struct(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            let mut message = MESSAGE_EMPTY_STRUCT;
            if m.in_tool_parameters {
                message = MESSAGE_TOOL_PARAMETERS;
            }
            diags.push(ctx.new_diagnostic(m.source_file, m.location, message, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/empty_schema_struct.go:43 EmptySchemaStructMatch 230233d1634d061d
// EmptySchemaStructMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the emptySchemaStruct pattern.
#[derive(Clone)]
pub struct EmptySchemaStructMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange, // Pre-computed error range for the diagnostic
    // Flow starts with the subject {} and the Schema.Struct step.
    pub flow: PipingFlow,
    // InToolParameters is set when the flow is the parameters of Tool.make or Tool.dynamic.
    pub in_tool_parameters: bool,
    // ToolNamespace is the Tool module receiver of the tool constructor
    // (Tool in Tool.make), or nil when the constructor is not accessed on it.
    pub tool_namespace: Option<P<Node>>,
}

// go: internal/rules/empty_schema_struct.go:57 AnalyzeEmptySchemaStruct caa77c4840fa8882
// AnalyzeEmptySchemaStruct finds every flow that applies Schema.Struct to an
// empty object literal and returns match structs for each.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`); Go ignores it, so it is dropped.
pub fn analyze_empty_schema_struct(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<EmptySchemaStructMatch> {
    let flows = tp.piping_flows(sf, false);
    let tool_parameters = empty_schema_struct_tool_parameters(tp, flows.as_deref());

    let mut matches = Vec::new();
    for (_, flow) in ets_readonly::new(flows.as_deref()).all() {
        if !PipingFlow::matches_prefix(
            Some(flow),
            tp,
            &|_: &mut TypeParser<'_>, subject: &PipingFlowSubject| {
                // Spreads and computed keys are properties too
                let Some(node) = subject.node() else {
                    return false;
                };
                if node.kind() != Kind::ObjectLiteralExpression {
                    return false;
                }
                node.as_object_literal_expression().properties().nodes().is_empty()
            },
            &[&|tp: &mut TypeParser<'_>, t: &PipingFlowTransformation| {
                t.args().len() == 0
                    && tp.is_node_reference_to_effect_schema_module_api(t.callee(), "Struct")
            }],
        ) {
            continue;
        }

        // The flow node is the Schema.Struct({}) expression itself only when no step follows it
        let node = flow.node();
        let struct_only = flow.transformations().len() == 1;

        // Schema.StructWithRest(Schema.Struct({}), records) adds index signatures to the empty struct
        if struct_only && is_struct_with_rest_fields(tp, node) {
            continue;
        }

        // A call step is reported on its call; other spellings on the callee
        let step = flow.transformations().at(0);
        let mut location = step.callee();
        if step.kind() == TransformationKind::Call {
            location = step.callee().parent().unwrap();
        }

        let mut tool_namespace = None;
        let mut in_tool_parameters = false;
        if struct_only && let Some(namespace) = tool_parameters.get(&node) {
            tool_namespace = *namespace;
            in_tool_parameters = true;
        }
        matches.push(EmptySchemaStructMatch {
            source_file: sf,
            location: ets_shim::scanner::get_error_range_for_node(sf, location),
            flow: flow.clone(),
            in_tool_parameters,
            tool_namespace,
        });
    }
    matches
}

// go: internal/rules/empty_schema_struct.go:114 isStructWithRestFields d440ab44e70507f7
// isStructWithRestFields reports whether node is the fields argument of
// Schema.StructWithRest, a call that never becomes a flow step.
fn is_struct_with_rest_fields(tp: &mut TypeParser<'_>, node: P<Node>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if parent.kind() != Kind::CallExpression {
        return false;
    }
    let call = parent.as_call_expression();
    let arguments = call.arguments().nodes();
    !arguments.is_empty()
        && arguments[0] == node
        && tp.is_node_reference_to_effect_schema_module_api(call.expression(), "StructWithRest")
}

// go: internal/rules/empty_schema_struct.go:127 emptySchemaStructToolParameters 935b905a6d97b7ae
// emptySchemaStructToolParameters maps the parameters value of each Tool.make
// and Tool.dynamic step to the Tool module receiver of its constructor, or nil
// when the constructor is not accessed on it.
fn empty_schema_struct_tool_parameters(
    tp: &mut TypeParser<'_>,
    flows: Option<&[PipingFlow]>,
) -> FxHashMap<P<Node>, Option<P<Node>>> {
    let mut parameters = FxHashMap::default();
    for (_, flow) in ets_readonly::new(flows).all() {
        for (_, t) in flow.transformations().all() {
            // Tool.make(name, options) applies the step to the name with the options as its argument
            if t.args().len() != 1
                || (!tp.is_node_reference_to_effect_ai_tool_module_api(t.callee(), "make")
                    && !tp.is_node_reference_to_effect_ai_tool_module_api(t.callee(), "dynamic"))
            {
                continue;
            }
            let options = t.args().at(0);
            if options.kind() != Kind::ObjectLiteralExpression {
                continue;
            }
            let mut tool_namespace = None;
            if ast::is_property_access_expression(t.callee()) {
                let receiver = t.callee().as_property_access_expression().expression();
                if tp.is_expression_effect_ai_tool_module(receiver) {
                    tool_namespace = Some(receiver);
                }
            }
            for property in options.as_object_literal_expression().properties().nodes() {
                if property.kind() == Kind::PropertyAssignment
                    && ast::get_text_of_property_name(property.as_property_assignment().name())
                        == "parameters"
                {
                    parameters.insert(
                        ast::skip_parentheses(property.as_property_assignment().initializer()),
                        tool_namespace,
                    );
                }
            }
        }
    }
    parameters
}
