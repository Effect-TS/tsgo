use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{Diagnostic, Kind, Node, SourceFile, Symbol, SymbolFlags};
use ets_shim::checker::{Checker, Type};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message =
    &tsdiag::THIS_TYPEOF_TYPE_QUERY_CAN_BE_REPLACED_WITH_0_EFFECT_UNNECESSARY_TYPEOF_TYPE;

// go: internal/rules/unnecessary_typeof_type.go:16 UnnecessaryTypeofType 34bcb30f26ca5b7d
pub static UNNECESSARY_TYPEOF_TYPE: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "unnecessaryTypeofType",
    group: "style",
    description: "Suggests replacing typeof Schema.Type style annotations with the matching named type when available",
    default_severity: Severity::Suggestion,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_unnecessary_typeof_type(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(
                m.source_file,
                m.location,
                MESSAGE,
                &[],
                &[&m.replacement_text],
            ));
        }
        diags
    },
});

// go: internal/rules/unnecessary_typeof_type.go:33 UnnecessaryTypeofTypeMatch 5f31ff40d29cc11e
#[derive(Clone)]
pub struct UnnecessaryTypeofTypeMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub query_node: P<Node>,
    pub inner_entity_name: P<Node>,
    pub replacement_text: String,
}

// go: internal/rules/unnecessary_typeof_type.go:41 AnalyzeUnnecessaryTypeofType c31a305610937297
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub fn analyze_unnecessary_typeof_type(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<UnnecessaryTypeofTypeMatch> {
    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        node: P<Node>,
        matches: &mut Vec<UnnecessaryTypeofTypeMatch>,
    ) -> bool {
        if node.kind() == Kind::TypeQuery
            && let Some(m) = analyze_unnecessary_typeof_type_node(tp, sf, node)
        {
            matches.push(m);
        }

        node.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}

// go: internal/rules/unnecessary_typeof_type.go:64 analyzeUnnecessaryTypeofTypeNode a3de280768d625a6
pub(crate) fn analyze_unnecessary_typeof_type_node(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
) -> Option<UnnecessaryTypeofTypeMatch> {
    let query = node.as_type_query_node();
    if let Some(type_arguments) = query.type_arguments()
        && !type_arguments.nodes().is_empty()
    {
        return None;
    }

    let expr_name = query.expr_name();
    if expr_name.kind() != Kind::QualifiedName {
        return None;
    }

    let qualified_name = expr_name.as_qualified_name();
    if qualified_name.right().text() != "Type" {
        return None;
    }

    let inner_entity_name = qualified_name.left();

    let query_type = tp.get_type_at_location(node)?;

    let (inner_symbol, inner_type) =
        resolve_entity_name_as_type(tp.checker(), Some(node), Some(qualified_name.left()));
    let inner_type = inner_type?;
    if is_self_referential_type_alias_reference(inner_symbol, Some(node)) {
        return None;
    }

    if !tp.checker().is_type_assignable_to(query_type, inner_type)
        || !tp.checker().is_type_assignable_to(inner_type, query_type)
    {
        return None;
    }

    Some(UnnecessaryTypeofTypeMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, node),
        query_node: node,
        inner_entity_name,
        replacement_text: ets_shim::scanner::get_text_of_node(inner_entity_name),
    })
}

// go: internal/rules/unnecessary_typeof_type.go:114 resolveEntityNameAsType c7c78cb4795a2556
//
// Rust: Go's nil checker never reaches here (the rule always has one).
pub(crate) fn resolve_entity_name_as_type(
    c: &mut Checker,
    location: Option<P<Node>>,
    entity_name: Option<P<Node>>,
) -> (Option<P<Symbol>>, Option<P<Type>>) {
    let (Some(location), Some(entity_node)) = (location, entity_name) else {
        return (None, None);
    };

    match entity_node.kind() {
        Kind::Identifier => {
            let symbol = c.resolve_name_exported(
                entity_node.text(),
                Some(location),
                SymbolFlags::Type | SymbolFlags::Namespace,
                false,
            );
            let symbol = resolve_alias_symbol(c, symbol);
            (symbol, resolved_symbol_type(c, symbol, Some(location)))
        }
        Kind::QualifiedName => {
            let qualified_name = entity_node.as_qualified_name();

            let (left_symbol, left_type) =
                resolve_entity_name_as_type(c, Some(location), Some(qualified_name.left()));
            let member_symbol = resolve_qualified_type_member(
                c,
                Some(location),
                left_symbol,
                left_type,
                qualified_name.right().text(),
            );
            let member_symbol = resolve_alias_symbol(c, member_symbol);
            (member_symbol, resolved_symbol_type(c, member_symbol, Some(location)))
        }
        _ => (None, None),
    }
}

// go: internal/rules/unnecessary_typeof_type.go:144 resolveQualifiedTypeMember e6834ee4b00d9847
pub(crate) fn resolve_qualified_type_member(
    c: &mut Checker,
    location: Option<P<Node>>,
    left_symbol: Option<P<Symbol>>,
    left_type: Option<P<Type>>,
    member_name: &str,
) -> Option<P<Symbol>> {
    if location.is_none() || member_name.is_empty() {
        return None;
    }

    if let Some(left_symbol) = left_symbol {
        if let Some(member) =
            c.try_get_member_in_module_exports_and_properties(member_name, left_symbol)
        {
            return Some(member);
        }

        let declared_type = c.get_declared_type_of_symbol(left_symbol);
        if let Some(member) = c.get_property_of_type(declared_type, member_name) {
            return Some(member);
        }
    }

    if let Some(left_type) = left_type
        && let Some(member) = c.get_property_of_type(left_type, member_name)
    {
        return Some(member);
    }

    None
}

// go: internal/rules/unnecessary_typeof_type.go:170 resolvedSymbolType e7c8347a0c017a9f
pub(crate) fn resolved_symbol_type(
    c: &mut Checker,
    symbol: Option<P<Symbol>>,
    location: Option<P<Node>>,
) -> Option<P<Type>> {
    let (Some(symbol), Some(location)) = (symbol, location) else {
        return None;
    };

    if symbol.flags().intersects(SymbolFlags::Type) {
        return Some(c.get_declared_type_of_symbol(symbol));
    }

    c.get_type_of_symbol_at_location(symbol, Some(location))
}

// go: internal/rules/unnecessary_typeof_type.go:184 resolveAliasSymbol c1da45ce5b1f7678
pub(crate) fn resolve_alias_symbol(
    c: &mut Checker,
    mut symbol: Option<P<Symbol>>,
) -> Option<P<Symbol>> {
    while let Some(s) = symbol
        && s.flags().intersects(SymbolFlags::Alias)
    {
        symbol = Some(c.get_aliased_symbol(s));
    }
    symbol
}

// go: internal/rules/unnecessary_typeof_type.go:191 isSelfReferentialTypeAliasReference 61581e698e4a65e3
pub(crate) fn is_self_referential_type_alias_reference(
    symbol: Option<P<Symbol>>,
    location: Option<P<Node>>,
) -> bool {
    let (Some(symbol), Some(location)) = (symbol, location) else {
        return false;
    };

    let mut current = location.parent();
    while let Some(node) = current {
        current = node.parent();
        if node.kind() != Kind::TypeAliasDeclaration {
            continue;
        }
        if symbol.declarations().contains(&node) {
            return true;
        }
    }

    false
}
