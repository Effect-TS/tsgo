use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::TypeParser;

// go: internal/rules/instance_of_schema.go:16 InstanceOfSchema 7ae4d29f25b10462
// InstanceOfSchema suggests using Schema.is instead of instanceof for Effect Schema types.
// This rule is disabled by default.
pub static INSTANCE_OF_SCHEMA: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
    name: "instanceOfSchema",
    group: "effectNative",
    description: "Suggests using Schema.is instead of instanceof for Effect Schema types",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![tsdiag::THIS_CODE_USES_INSTANCEOF_WITH_AN_EFFECT_SCHEMA_TYPE_SCHEMA_IS_IS_THE_SCHEMA_AWARE_RUNTIME_CHECK_FOR_THIS_CASE_EFFECT_INSTANCE_OF_SCHEMA.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let matches = analyze_instance_of_schema(ctx.type_parser, ctx.source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                &tsdiag::THIS_CODE_USES_INSTANCEOF_WITH_AN_EFFECT_SCHEMA_TYPE_SCHEMA_IS_IS_THE_SCHEMA_AWARE_RUNTIME_CHECK_FOR_THIS_CASE_EFFECT_INSTANCE_OF_SCHEMA,
                &[],
                &[],
            ));
        }
        diags
    },
}
});

// go: internal/rules/instance_of_schema.go:35 InstanceOfSchemaMatch 7c487e071cb18f40
// InstanceOfSchemaMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the instanceOfSchema pattern.
#[derive(Clone, Copy)]
pub struct InstanceOfSchemaMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange, // The pre-computed error range for this match
    pub instance_of_node: P<Node>, // The full BinaryExpression node (the instanceof expression)
    pub left_expr: P<Node>,  // Left operand of instanceof (the value being tested)
    pub right_expr: P<Node>, // Right operand of instanceof (the schema type reference)
}

// go: internal/rules/instance_of_schema.go:45 AnalyzeInstanceOfSchema 2be9e7a9077716c1
// AnalyzeInstanceOfSchema finds all `value instanceof SchemaClass` expressions
// where the right-hand side is an Effect Schema type.
//
// Rust: Go's unused checker argument is dropped (the parser holds the checker).
pub fn analyze_instance_of_schema(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<InstanceOfSchemaMatch> {
    let mut matches = Vec::new();

    // Stack-based traversal
    let mut node_to_visit: Vec<P<Node>> = Vec::new();
    sf.as_node().for_each_child(&mut |child| {
        node_to_visit.push(child);
        false
    });

    while let Some(node) = node_to_visit.pop() {
        if ast::is_instance_of_expression(node) {
            let bin_expr = node.as_binary_expression();
            let right_expr = bin_expr.right();
            let right_type = tp.get_type_at_location(right_expr);
            if let Some(right_type) = right_type
                && tp.is_schema_type(right_type)
            {
                matches.push(InstanceOfSchemaMatch {
                    source_file: sf,
                    location: ets_shim::scanner::get_error_range_for_node(sf, node),
                    instance_of_node: node,
                    left_expr: bin_expr.left(),
                    right_expr,
                });
            }
        }

        // Enqueue children
        node.for_each_child(&mut |child| {
            node_to_visit.push(child);
            false
        });
    }

    matches
}
