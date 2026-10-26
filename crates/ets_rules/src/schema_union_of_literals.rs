use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::{EffectMajorVersion, TypeParser};

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_SCHEMA_UNION_CONTAINS_MULTIPLE_SCHEMA_LITERAL_MEMBERS_AND_CAN_BE_SIMPLIFIED_TO_A_SINGLE_SCHEMA_LITERAL_CALL_EFFECT_SCHEMA_UNION_OF_LITERALS;

// go: internal/rules/schema_union_of_literals.go:17 SchemaUnionOfLiterals
// SchemaUnionOfLiterals detects Schema.Union(...) calls where all arguments
// are Schema.Literal(...) calls and suggests combining them into a single
// Schema.Literal call. This rule is V3-only and disabled by default.
pub static SCHEMA_UNION_OF_LITERALS: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "schemaUnionOfLiterals",
    group: "style",
    description: "Suggests combining multiple Schema.Literal calls in Schema.Union into a single Schema.Literal",
    default_severity: Severity::Off,
    supported_effect: vec!["v3"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_schema_union_of_literals(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/schema_union_of_literals.go:36 SchemaUnionOfLiteralsMatch
// SchemaUnionOfLiteralsMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the schemaUnionOfLiterals pattern.
#[derive(Clone)]
pub struct SchemaUnionOfLiteralsMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange, // Pre-computed error range for the diagnostic
    pub union_call_node: P<Node>, // The full Schema.Union(...) call expression to be replaced
    pub first_literal_expression: Option<P<Node>>, // The callee expression (Schema.Literal) from the first argument
    pub all_literal_args: Vec<P<Node>>, // All arguments collected from every Schema.Literal(...) call, in order
}

// go: internal/rules/schema_union_of_literals.go:46 AnalyzeSchemaUnionOfLiterals
// AnalyzeSchemaUnionOfLiterals finds all Schema.Union(...) calls where every argument
// is a Schema.Literal(...) call, returning matches with captured nodes for diagnostics and fixes.
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`); Go ignores it, so it is dropped.
pub fn analyze_schema_union_of_literals(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<SchemaUnionOfLiteralsMatch> {
    // V3-only rule
    if tp.supported_effect_version() != EffectMajorVersion::V3 {
        return Vec::new();
    }

    let mut matches = Vec::new();

    let mut node_to_visit: Vec<P<Node>> = Vec::new();
    sf.as_node().for_each_child(&mut |child| {
        node_to_visit.push(child);
        false
    });

    while let Some(node) = node_to_visit.pop() {
        if node.kind() == Kind::CallExpression
            && let Some(m) = analyze_schema_union_of_literals_node(tp, sf, node)
        {
            matches.push(m);
        }

        node.for_each_child(&mut |child| {
            node_to_visit.push(child);
            false
        });
    }

    matches
}

// go: internal/rules/schema_union_of_literals.go:79 analyzeSchemaUnionOfLiteralsNode
// analyzeSchemaUnionOfLiteralsNode checks if a call expression is Schema.Union(...)
// where all arguments are Schema.Literal(...) calls.
pub(crate) fn analyze_schema_union_of_literals_node(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
) -> Option<SchemaUnionOfLiteralsMatch> {
    let call = node.as_call_expression();

    // Must have at least 2 arguments
    let arguments = call.arguments().nodes();
    if arguments.len() < 2 {
        return None;
    }

    // Check if this is Schema.Union
    if !tp.is_node_reference_to_effect_schema_module_api(call.expression(), "Union") {
        return None;
    }

    let mut first_literal_expression = None;
    let mut all_literal_args = Vec::new();

    // Check that every argument is a call expression referencing Schema.Literal
    for (i, &arg) in arguments.iter().enumerate() {
        if arg.kind() != Kind::CallExpression {
            return None;
        }
        let arg_call = arg.as_call_expression();
        if !tp.is_node_reference_to_effect_schema_module_api(arg_call.expression(), "Literal") {
            return None;
        }

        if i == 0 {
            first_literal_expression = Some(arg_call.expression());
        }

        // Collect all arguments from this Schema.Literal(...) call
        all_literal_args.extend_from_slice(arg_call.arguments().nodes());
    }

    Some(SchemaUnionOfLiteralsMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, node),
        union_call_node: node,
        first_literal_expression,
        all_literal_args,
    })
}
