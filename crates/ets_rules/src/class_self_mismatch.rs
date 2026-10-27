use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::TypeParser;

// go: internal/rules/class_self_mismatch.go:18 ClassSelfMismatch 8604fa5473ab39b1
// ClassSelfMismatch ensures the Self type parameter matches the class name in
// Effect.Service, Context.Service, Context.Tag, Effect.Tag, Schema.Class,
// Schema.TaggedClass, Schema.TaggedError, Schema.TaggedRequest,
// Schema.RequestClass, and Model.Class declarations.
pub static CLASS_SELF_MISMATCH: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "classSelfMismatch",
    group: "correctness",
    description: "Ensures Self type parameter matches the class name in Context/Service/Tag/Schema classes",
    default_severity: Severity::Error,
    supported_effect: vec!["v3", "v4"],
    codes: vec![
        tsdiag::THE_SELF_TYPE_PARAMETER_FOR_THIS_CLASS_SHOULD_BE_0_EFFECT_CLASS_SELF_MISMATCH
            .code(),
    ],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_class_self_mismatch(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                &tsdiag::THE_SELF_TYPE_PARAMETER_FOR_THIS_CLASS_SHOULD_BE_0_EFFECT_CLASS_SELF_MISMATCH,
                &[],
                &[&m.expected_name],
            ));
        }
        diags
    },
});

// go: internal/rules/class_self_mismatch.go:37 ClassSelfMismatchMatch 63dca53c6a5208b6
// ClassSelfMismatchMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the classSelfMismatch pattern.
#[derive(Clone)]
pub struct ClassSelfMismatchMatch {
    pub source_file: P<SourceFile>, // The source file of the match
    pub location: TextRange,        // The pre-computed error range for the selfTypeNode
    pub self_type_node: P<Node>,    // The Self type argument node
    pub class_name: P<Node>,        // The class name identifier
    pub expected_name: String,      // The expected name (from the class declaration)
    pub actual_name: String,        // The actual name found in the Self type parameter
}

// go: internal/rules/class_self_mismatch.go:48 AnalyzeClassSelfMismatch d7bc187efbd0f71e
// AnalyzeClassSelfMismatch finds all class declarations where the Self type
// parameter does not match the class name.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub fn analyze_class_self_mismatch(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<ClassSelfMismatchMatch> {
    let mut matches = Vec::new();

    let mut node_to_visit: Vec<P<Node>> = Vec::new();
    sf.as_node().for_each_child(&mut |child| {
        node_to_visit.push(child);
        false
    });

    while let Some(node) = node_to_visit.pop() {
        if node.kind() == Kind::ClassDeclaration && node.name().is_some() {
            if let Some(m) = check_class_self_mismatch(tp, sf, node) {
                matches.push(m);
            }
        }

        node.for_each_child(&mut |child| {
            node_to_visit.push(child);
            false
        });
    }

    matches
}

// go: internal/rules/class_self_mismatch.go:74 checkClassSelfMismatch 58b3a2172846397b
pub(crate) fn check_class_self_mismatch(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    class_node: P<Node>,
) -> Option<ClassSelfMismatchMatch> {
    let self_type_node: P<Node>;
    let class_name: P<Node>;

    // Try extends* functions in order, matching the TS reference
    if let Some(result) = tp.extends_effect_v3_service(class_node) {
        self_type_node = result.self_type_node();
        class_name = result.class_name();
    } else if let Some(result) = tp.extends_context_service(class_node) {
        self_type_node = result.self_type_node();
        class_name = result.class_name();
    } else if let Some(result) = tp.extends_context_tag(class_node) {
        self_type_node = result.self_type_node();
        class_name = result.class_name();
    } else if let Some(result) = tp.extends_effect_tag(class_node) {
        self_type_node = result.self_type_node();
        class_name = result.class_name();
    } else if let Some(result) = tp.extends_schema_class(class_node) {
        self_type_node = result.self_type_node();
        class_name = result.class_name();
    } else if let Some(result) = tp.extends_schema_tagged_class(class_node) {
        self_type_node = result.self_type_node();
        class_name = result.class_name();
    } else if let Some(result) = tp.extends_schema_tagged_error(class_node) {
        self_type_node = result.self_type_node();
        class_name = result.class_name();
    } else if let Some(result) = tp.extends_schema_tagged_request(class_node) {
        self_type_node = result.self_type_node();
        class_name = result.class_name();
    } else if let Some(result) = tp.extends_schema_request_class(class_node) {
        self_type_node = result.self_type_node();
        class_name = result.class_name();
    } else if let Some(result) = tp.extends_effect_sql_model_class(class_node) {
        self_type_node = result.self_type_node();
        class_name = result.class_name();
    } else {
        let result = tp.extends_effect_model_class(class_node)?;
        self_type_node = result.self_type_node();
        class_name = result.class_name();
    }

    // Extract actual name from the Self type node
    let actual_name = extract_self_type_name(sf, self_type_node);

    // Get expected name from the class name
    let expected_name = ets_shim::scanner::get_text_of_node(class_name);

    if actual_name == expected_name {
        return None;
    }

    Some(ClassSelfMismatchMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, self_type_node),
        self_type_node,
        class_name,
        expected_name,
        actual_name,
    })
}

// go: internal/rules/class_self_mismatch.go:142 extractSelfTypeName 86a1fee1e6a6cf65
// extractSelfTypeName extracts the name text from a Self type node.
// For TypeReferenceNode with Identifier typeName → identifier text.
// For TypeReferenceNode with QualifiedName typeName → right identifier text.
// Fallback → source text substring between node pos and end.
pub(crate) fn extract_self_type_name(sf: P<SourceFile>, self_type_node: P<Node>) -> String {
    if ast::is_type_reference_node(self_type_node) {
        let type_name = self_type_node.as_type_reference_node().type_name();
        if ast::is_identifier(type_name) {
            return type_name.text().to_string();
        }
        if ast::is_qualified_name(type_name) {
            return type_name.as_qualified_name().right().text().to_string();
        }
    }
    // Fallback: use source text
    let text = sf.text();
    let (Some(pos), Some(end)) =
        (self_type_node.pos().source_offset(), self_type_node.end().source_offset())
    else {
        return String::new();
    };
    let (pos, end) = (pos as usize, end as usize);
    if end >= pos && end <= text.len() {
        return text[pos..end].to_string();
    }
    String::new()
}
