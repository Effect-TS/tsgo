use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::This_Schema_Struct_includes_a_tag_field_Schema_TaggedStruct_is_the_tagged_struct_form_for_this_pattern_and_makes_the_tag_optional_in_the_constructor_effect_schemaStructWithTag;

// go: internal/rules/schema_struct_with_tag.go:17 SchemaStructWithTag
// SchemaStructWithTag detects Schema.Struct(...) calls where the argument
// contains a _tag property assigned to Schema.Literal("someString"), and
// suggests using Schema.TaggedStruct instead.
pub static SCHEMA_STRUCT_WITH_TAG: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "schemaStructWithTag",
    group: "style",
    description: "Suggests using Schema.TaggedStruct instead of Schema.Struct with _tag field",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_schema_struct_with_tag(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/schema_struct_with_tag.go:36 SchemaStructWithTagMatch
// SchemaStructWithTagMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the schemaStructWithTag pattern.
#[derive(Clone)]
pub struct SchemaStructWithTagMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange, // Pre-computed error range for the diagnostic
    pub call_node: P<Node>,  // The full Schema.Struct(...) call expression
    pub tag_value: String,   // The string value extracted from Schema.Literal("...")
    pub other_properties: Vec<P<Node>>, // All property assignment nodes in the object literal except _tag
    pub schema_expr: Option<P<Node>>,   // The expression node for the Schema part of Schema.Struct
}

// go: internal/rules/schema_struct_with_tag.go:47 AnalyzeSchemaStructWithTag
// AnalyzeSchemaStructWithTag finds all Schema.Struct({ _tag: Schema.Literal("..."), ... })
// call expressions and returns match structs for each.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`); Go ignores it, so it is dropped.
pub fn analyze_schema_struct_with_tag(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<SchemaStructWithTagMatch> {
    let mut matches = Vec::new();

    // Stack-based traversal
    let mut node_to_visit: Vec<P<Node>> = Vec::new();
    sf.as_node().for_each_child(&mut |child| {
        node_to_visit.push(child);
        false
    });

    while let Some(node) = node_to_visit.pop() {
        if node.kind() == Kind::CallExpression
            && let Some(m) = analyze_schema_struct_with_tag_node(tp, sf, node)
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

// go: internal/rules/schema_struct_with_tag.go:76 analyzeSchemaStructWithTagNode
// analyzeSchemaStructWithTagNode checks a call expression for Schema.Struct({ _tag: Schema.Literal("...") }).
pub(crate) fn analyze_schema_struct_with_tag_node(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
) -> Option<SchemaStructWithTagMatch> {
    if node.kind() != Kind::CallExpression {
        return None;
    }
    let call = node.as_call_expression();

    // Must have exactly 1 argument
    let arguments = call.arguments().nodes();
    if arguments.len() != 1 {
        return None;
    }

    let arg = arguments[0];
    if arg.kind() != Kind::ObjectLiteralExpression {
        return None;
    }

    let obj_lit = arg.as_object_literal_expression();
    let properties = obj_lit.properties().nodes();

    // Check if this is Schema.Struct
    if !tp.is_node_reference_to_effect_schema_module_api(call.expression(), "Struct") {
        return None;
    }

    // Look for a _tag property assignment
    for &prop in properties {
        if prop.kind() != Kind::PropertyAssignment {
            continue;
        }
        let pa = prop.as_property_assignment();
        let pa_name = pa.name();
        if pa_name.kind() != Kind::Identifier
            || ets_shim::scanner::get_text_of_node(pa_name) != "_tag"
        {
            continue;
        }

        // Found _tag property — check if its initializer is Schema.Literal("...")
        let init = pa.initializer();
        if init.kind() != Kind::CallExpression {
            return None;
        }

        let literal_call = init.as_call_expression();

        // Schema.Literal must have exactly 1 argument that is a string literal
        let literal_arguments = literal_call.arguments().nodes();
        if literal_arguments.len() != 1 {
            return None;
        }
        if literal_arguments[0].kind() != Kind::StringLiteral {
            return None;
        }
        if !tp.is_node_reference_to_effect_schema_module_api(literal_call.expression(), "Literal") {
            return None;
        }

        let tag_value = literal_arguments[0].text().to_string();

        // Collect all properties except _tag
        let mut other_props = Vec::new();
        for &p in properties {
            if p == prop {
                continue;
            }
            other_props.push(p);
        }

        // Extract the Schema expression from Schema.Struct (the left side of the property access)
        let mut schema_expr = None;
        if call.expression().kind() == Kind::PropertyAccessExpression {
            schema_expr = Some(call.expression().as_property_access_expression().expression());
        }

        return Some(SchemaStructWithTagMatch {
            source_file: sf,
            location: ets_shim::scanner::get_error_range_for_node(sf, node),
            call_node: node,
            tag_value,
            other_properties: other_props,
            schema_expr,
        });
    }

    None
}
