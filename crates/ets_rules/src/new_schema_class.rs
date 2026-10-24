use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::checker::SignatureKind;
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{EffectMajorVersion, TypeParser};

// go: internal/rules/new_schema_class.go:16 NewSchemaClass
// NewSchemaClass suggests using the Schema make API instead of direct
// construction with new for Schema classes.
pub static NEW_SCHEMA_CLASS: LazyLock<Rule> = LazyLock::new(|| {
    Rule {
        name: "newSchemaClass",
        group: "style",
        description: "Suggests using Schema make instead of new for Schema classes",
        default_severity: Severity::Off,
        supported_effect: vec!["v4"],
        codes: vec![tsdiag::This_Schema_class_is_constructed_with_new_0_make_can_be_used_to_construct_the_Schema_class_instance_effect_newSchemaClass.code()],
        run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
            let matches = analyze_new_schema_class(ctx.type_parser, ctx.source_file);
            let mut diags = Vec::with_capacity(matches.len());
            for m in &matches {
                diags.push(ctx.new_diagnostic(
                    m.source_file,
                    m.location,
                    &tsdiag::This_Schema_class_is_constructed_with_new_0_make_can_be_used_to_construct_the_Schema_class_instance_effect_newSchemaClass,
                    &[],
                    &[&m.class_text],
                ));
            }
            diags
        },
    }
});

// go: internal/rules/new_schema_class.go:39 NewSchemaClassMatch
#[derive(Clone)]
pub struct NewSchemaClassMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub new_node: P<Node>,
    pub class_expr: P<Node>,
    pub class_text: String,
    pub arguments: Vec<P<Node>>,
}

// go: internal/rules/new_schema_class.go:48 AnalyzeNewSchemaClass
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`), so it is dropped.
pub fn analyze_new_schema_class(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<NewSchemaClassMatch> {
    if tp.supported_effect_version() != EffectMajorVersion::V4 {
        return Vec::new();
    }

    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        node: P<Node>,
        matches: &mut Vec<NewSchemaClassMatch>,
    ) -> bool {
        if node.kind() == Kind::NewExpression {
            let new_expr = node.as_new_expression();
            if is_v4_schema_class_expression(tp, Some(new_expr.expression)) {
                let mut args = Vec::new();
                if let Some(arguments) = new_expr.arguments {
                    args = arguments.nodes().to_vec();
                }
                matches.push(NewSchemaClassMatch {
                    source_file: sf,
                    location: ets_shim::scanner::get_error_range_for_node(sf, new_expr.expression),
                    new_node: node,
                    class_expr: new_expr.expression,
                    class_text: ets_shim::scanner::get_source_text_of_node_from_source_file(
                        sf,
                        new_expr.expression,
                        false,
                    ),
                    arguments: args,
                });
            }
        }

        node.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}

// go: internal/rules/new_schema_class.go:87 isV4SchemaClassExpression
//
// Rust: the checker is the parser's.
pub(crate) fn is_v4_schema_class_expression(
    tp: &mut TypeParser<'_>,
    expr: Option<P<Node>>,
) -> bool {
    let Some(expr) = expr else {
        return false;
    };
    if tp.node_cannot_be_effect_library_type(expr) {
        return false;
    }

    let Some(t) = tp.get_type_at_location(expr) else {
        return false;
    };
    if !tp.is_schema_type(t) {
        return false;
    }

    if tp.checker().get_signatures_of_type(t, SignatureKind::Construct).is_empty() {
        return false;
    }

    tp.get_type_of_property_by_name(t, "make").is_some()
}
