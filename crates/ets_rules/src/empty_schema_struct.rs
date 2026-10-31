use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

/// Rust-only: the rule's general message (Go spells it out at each use).
static MESSAGE_EMPTY_STRUCT: &Message = &tsdiag::SCHEMA_STRUCT_MODELS_THE_TYPE_SO_ANY_NON_NULL_VALUE_PASSES_DECODING_UNVALIDATED_USE_AN_OBJECT_KEYWORD_SCHEMA_SCHEMA_OBJECT_KEYWORD_IN_V4_SCHEMA_OBJECT_IN_V3_FOR_ANY_OBJECT_A_RECORD_SCHEMA_FOR_OPEN_KEY_SLASHVALUE_MAPS_OR_ADD_THE_INTENDED_FIELDS_EFFECT_EMPTY_SCHEMA_STRUCT;

/// Rust-only: the rule's tool parameters message (Go spells it out at each use).
static MESSAGE_TOOL_PARAMETERS: &Message = &tsdiag::SCHEMA_STRUCT_AS_TOOL_PARAMETERS_BREAKS_STRUCTURED_OUTPUT_SCHEMA_GENERATION_FOR_SOME_PROVIDERS_USE_TOOL_EMPTY_PARAMS_INSTEAD_EFFECT_EMPTY_SCHEMA_STRUCT;

// go: internal/rules/empty_schema_struct.go:16 EmptySchemaStruct c429e81197d84b1b
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

// go: internal/rules/empty_schema_struct.go:42 EmptySchemaStructMatch dbe3e6694e75ac4a
// EmptySchemaStructMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the emptySchemaStruct pattern.
#[derive(Clone)]
pub struct EmptySchemaStructMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange, // Pre-computed error range for the diagnostic
    pub call: P<Node>,       // The Schema.Struct({}) call expression
    // InToolParameters is set when Call is the parameters of Tool.make or Tool.dynamic.
    pub in_tool_parameters: bool,
    // ToolNamespace is the Tool module receiver of the tool constructor
    // (Tool in Tool.make), or nil when the constructor is not accessed on it.
    pub tool_namespace: Option<P<Node>>,
}

// go: internal/rules/empty_schema_struct.go:55 AnalyzeEmptySchemaStruct 7792dce163d6ae4e
// AnalyzeEmptySchemaStruct finds all Schema.Struct({}) call expressions and
// returns match structs for each.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`); Go ignores it, so it is dropped.
pub fn analyze_empty_schema_struct(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<EmptySchemaStructMatch> {
    let mut matches = Vec::new();

    // Stack-based traversal
    let mut node_to_visit: Vec<P<Node>> = Vec::new();
    sf.as_node().for_each_child(&mut |child| {
        node_to_visit.push(child);
        false
    });

    while let Some(node) = node_to_visit.pop() {
        if node.kind() == Kind::CallExpression
            && let Some(m) = analyze_empty_schema_struct_node(tp, sf, node)
        {
            matches.push(m);
        }

        // Enqueue children
        node.for_each_child(&mut |child| {
            node_to_visit.push(child);
            false
        });
    }

    matches
}

// go: internal/rules/empty_schema_struct.go:84 analyzeEmptySchemaStructNode 87077df0a4cc0b12
// analyzeEmptySchemaStructNode checks a call expression for Schema.Struct({}).
pub(crate) fn analyze_empty_schema_struct_node(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
) -> Option<EmptySchemaStructMatch> {
    let call = node.as_call_expression();

    // A single argument: v3 index-signature records follow the fields as extra arguments
    let arguments = call.arguments().nodes();
    if arguments.len() != 1 {
        return None;
    }

    // The argument must be a literal {}; spreads and computed keys are properties too
    let arg = ast::skip_parentheses(arguments[0]);
    if arg.kind() != Kind::ObjectLiteralExpression {
        return None;
    }
    if !arg.as_object_literal_expression().properties().nodes().is_empty() {
        return None;
    }

    if !tp.is_node_reference_to_effect_schema_module_api(call.expression(), "Struct") {
        return None;
    }

    // Schema.StructWithRest(Schema.Struct({}), records) adds index signatures to the empty struct
    if let Some(parent) = ast::walk_up_parenthesized_expressions(node.parent())
        && parent.kind() == Kind::CallExpression
    {
        let parent_call = parent.as_call_expression();
        let parent_arguments = parent_call.arguments().nodes();
        if !parent_arguments.is_empty()
            && ast::skip_parentheses(parent_arguments[0]) == node
            && tp.is_node_reference_to_effect_schema_module_api(
                parent_call.expression(),
                "StructWithRest",
            )
        {
            return None;
        }
    }

    let (in_tool_parameters, tool_namespace) = empty_schema_struct_tool_context(tp, node);
    Some(EmptySchemaStructMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, node),
        call: node,
        in_tool_parameters,
        tool_namespace,
    })
}

// go: internal/rules/empty_schema_struct.go:127 emptySchemaStructToolContext e9b80ca633c6417e
// emptySchemaStructToolContext reports whether call is the parameters property
// of the options passed to Tool.make or Tool.dynamic, and returns the Tool
// module receiver of the constructor when there is one.
pub(crate) fn empty_schema_struct_tool_context(
    tp: &mut TypeParser<'_>,
    call: P<Node>,
) -> (bool, Option<P<Node>>) {
    // A computed name has a ComputedPropertyName parent, so call is the initializer here
    let Some(property) = ast::walk_up_parenthesized_expressions(call.parent()) else {
        return (false, None);
    };
    if property.kind() != Kind::PropertyAssignment
        || ast::get_text_of_property_name(property.as_property_assignment().name()) != "parameters"
    {
        return (false, None);
    }

    let Some(options) = property.parent() else {
        return (false, None);
    };
    if options.kind() != Kind::ObjectLiteralExpression {
        return (false, None);
    }
    let Some(tool_call) = ast::walk_up_parenthesized_expressions(options.parent()) else {
        return (false, None);
    };
    if tool_call.kind() != Kind::CallExpression {
        return (false, None);
    }
    // Tool.make and Tool.dynamic take (name, options)
    let arguments = tool_call.as_call_expression().arguments().nodes();
    if arguments.len() < 2 || ast::skip_parentheses(arguments[1]) != options {
        return (false, None);
    }
    let callee = tool_call.as_call_expression().expression();
    if !tp.is_node_reference_to_effect_ai_tool_module_api(callee, "make")
        && !tp.is_node_reference_to_effect_ai_tool_module_api(callee, "dynamic")
    {
        return (false, None);
    }

    if ast::is_property_access_expression(callee) {
        let receiver = callee.as_property_access_expression().expression();
        if tp.is_expression_effect_ai_tool_module(receiver) {
            return (true, Some(receiver));
        }
    }
    (true, None)
}
