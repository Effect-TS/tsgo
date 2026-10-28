use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_ARROW_FUNCTION_BLOCK_ONLY_RETURNS_AN_EXPRESSION_AND_CAN_USE_A_CONCISE_BODY_EFFECT_UNNECESSARY_ARROW_BLOCK;

// go: internal/rules/unnecessary_arrow_block.go:16 UnnecessaryArrowBlock cfd853d53f92f8ec
// UnnecessaryArrowBlock suggests using a concise arrow body when a block only
// contains a single return statement.
pub static UNNECESSARY_ARROW_BLOCK: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "unnecessaryArrowBlock",
    group: "style",
    description: "Suggests using a concise arrow body when the block only returns an expression",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_unnecessary_arrow_block(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[]));
        }
        diags
    },
});

// go: internal/rules/unnecessary_arrow_block.go:40 UnnecessaryArrowBlockMatch 4752c679c73f8efa
#[derive(Clone)]
pub struct UnnecessaryArrowBlockMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub arrow_function: P<Node>,
    pub body_block: P<Node>,
    pub returned_expression: P<Node>,
}

// go: internal/rules/unnecessary_arrow_block.go:48 AnalyzeUnnecessaryArrowBlock 77d4f6ca19cddcf4
//
// Rust: Go ignores the parser and checker; the parser is kept so every Analyze* has the same shape.
pub fn analyze_unnecessary_arrow_block(
    _tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<UnnecessaryArrowBlockMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(sf: P<SourceFile>, n: P<Node>, matches: &mut Vec<UnnecessaryArrowBlockMatch>) -> bool {
        if n.kind() == Kind::ArrowFunction
            && let Some(m) = analyze_unnecessary_arrow_block_node(sf, n)
        {
            matches.push(m);
        }

        n.for_each_child(&mut |child| walk(sf, child, matches));
        false
    }

    walk(sf, sf.as_node(), &mut matches);
    matches
}

// go: internal/rules/unnecessary_arrow_block.go:71 analyzeUnnecessaryArrowBlockNode 6336a9cc5e8ea697
pub(crate) fn analyze_unnecessary_arrow_block_node(
    sf: P<SourceFile>,
    n: P<Node>,
) -> Option<UnnecessaryArrowBlockMatch> {
    let arrow_body = n.body()?;
    if arrow_body.kind() != Kind::Block {
        return None;
    }

    let statements = arrow_body.statements();
    if statements.len() != 1 {
        return None;
    }

    let stmt = statements[0];
    if stmt.kind() != Kind::ReturnStatement {
        return None;
    }

    let returned_expression = stmt.as_return_statement().expression()?;

    Some(UnnecessaryArrowBlockMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, arrow_body),
        arrow_function: n,
        body_block: arrow_body,
        returned_expression,
    })
}
