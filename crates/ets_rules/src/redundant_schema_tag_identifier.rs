use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::IDENTIFIER_0_IS_REDUNDANT_SINCE_IT_EQUALS_THE_TAG_VALUE_EFFECT_REDUNDANT_SCHEMA_TAG_IDENTIFIER;

// go: internal/rules/redundant_schema_tag_identifier.go:14 RedundantSchemaTagIdentifier
pub static REDUNDANT_SCHEMA_TAG_IDENTIFIER: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "redundantSchemaTagIdentifier",
    group: "style",
    description: "Suggests removing redundant identifier argument when it equals the tag value in Schema.TaggedClass/TaggedError/TaggedRequest",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_redundant_schema_tag_identifier(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            if m.key_string_literal.kind() != Kind::StringLiteral {
                continue;
            }
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[m.key_string_literal.as_string_literal().text()],
            ));
        }
        diags
    },
});

// go: internal/rules/redundant_schema_tag_identifier.go:36 RedundantSchemaTagIdentifierMatch
// RedundantSchemaTagIdentifierMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the redundantSchemaTagIdentifier pattern.
#[derive(Clone)]
pub struct RedundantSchemaTagIdentifierMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange, // Pre-computed error range for the diagnostic (on KeyStringLiteral)
    pub key_string_literal: P<Node>, // The redundant identifier string literal in the inner call
}

// go: internal/rules/redundant_schema_tag_identifier.go:44 AnalyzeRedundantSchemaTagIdentifier
// AnalyzeRedundantSchemaTagIdentifier finds all class declarations where the identifier
// string literal equals the tag value in Schema.TaggedClass/TaggedError/TaggedRequest.
//
// Rust: Go's checker argument is the parser's checker; Go ignores it, so it is dropped.
pub fn analyze_redundant_schema_tag_identifier(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<RedundantSchemaTagIdentifierMatch> {
    let mut matches = Vec::new();

    let mut node_to_visit: Vec<P<Node>> = Vec::new();
    sf.as_node().for_each_child(&mut |child| {
        node_to_visit.push(child);
        false
    });

    while let Some(node) = node_to_visit.pop() {
        if node.kind() == Kind::ClassDeclaration
            && let Some(m) = analyze_redundant_schema_tag_identifier_node(tp, sf, node)
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

// go: internal/rules/redundant_schema_tag_identifier.go:70 analyzeRedundantSchemaTagIdentifierNode
//
// Rust: Go's unused checker parameter is dropped; `(match, ok)` is an `Option`.
pub(crate) fn analyze_redundant_schema_tag_identifier_node(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
) -> Option<RedundantSchemaTagIdentifierMatch> {
    // Try ExtendsSchemaTaggedClass, then TaggedError, then TaggedRequest (short-circuit on first match)
    let mut result = tp.extends_schema_tagged_class(node);
    if result.is_none() {
        result = tp.extends_schema_tagged_error(node);
    }
    if result.is_none() {
        result = tp.extends_schema_tagged_request(node);
    }
    let result = result?;

    // Both key and tag must be present and must be string literals
    let (Some(key), Some(tag)) = (result.key_string_literal(), result.tag_string_literal()) else {
        return None;
    };
    if key.kind() != Kind::StringLiteral || tag.kind() != Kind::StringLiteral {
        return None;
    }

    let key_text = key.as_string_literal().text();
    let tag_text = tag.as_string_literal().text();

    if key_text != tag_text {
        return None;
    }

    Some(RedundantSchemaTagIdentifierMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, key),
        key_string_literal: key,
    })
}
