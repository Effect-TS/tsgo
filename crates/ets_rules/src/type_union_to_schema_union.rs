use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, NodeFlags, SourceFile, Symbol};
use ets_shim::core::TextRange;
use ets_shim::diagnostics::{self as tsdiag, Message};
use ets_typeparser::TypeParser;

use crate::*;

/// Rust-only: the rule's message (Go spells it out at each use).
static MESSAGE: &Message = &tsdiag::THIS_TYPE_ALIAS_UNIONS_SCHEMA_0_TYPES_WITHOUT_A_RUNTIME_SCHEMA_COMPOSE_THEM_WITH_SCHEMA_UNION_AND_DERIVE_THE_TYPE_FROM_IT_EFFECT_TYPE_UNION_TO_SCHEMA_UNION;

// go: internal/rules/type_union_to_schema_union.go:16 TypeUnionToSchemaUnion c9a44cfb38272045
pub static TYPE_UNION_TO_SCHEMA_UNION: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "typeUnionToSchemaUnion",
    group: "style",
    description: "Suggests composing unions of Schema .Type/.Encoded types with Schema.Union and deriving the type from it",
    default_severity: Severity::Off,
    supported_effect: vec!["v3", "v4"],
    codes: vec![MESSAGE.code()],
    replaced_by: vec![],
    run: |ctx: &mut Context<'_, '_>| -> Vec<P<Diagnostic>> {
        let source_file = ctx.source_file;
        let matches = analyze_type_union_to_schema_union(ctx.type_parser, source_file);
        let mut diags = Vec::with_capacity(matches.len());
        for m in &matches {
            diags.push(ctx.new_diagnostic(m.source_file, m.location, MESSAGE, &[], &[m.side]));
        }
        diags
    },
});

// go: internal/rules/type_union_to_schema_union.go:35 TypeUnionToSchemaUnionMember 640a9a50ae327973
// TypeUnionToSchemaUnionMember is one schema of the union: Name is the entity
// name S taken from `typeof S.Type` or `Schema.Schema.Type<typeof S>`.
#[derive(Clone)]
pub struct TypeUnionToSchemaUnionMember {
    pub name: P<Node>,
    pub symbol: P<Symbol>,
}

// go: internal/rules/type_union_to_schema_union.go:40 TypeUnionToSchemaUnionMatch 834ec4a1a4a2dd58
#[derive(Clone)]
pub struct TypeUnionToSchemaUnionMatch {
    pub source_file: P<SourceFile>,
    pub location: TextRange,
    pub alias: P<Node>,
    // Side is the extracted schema side, "Type" or "Encoded".
    pub side: &'static str,
    pub members: Vec<TypeUnionToSchemaUnionMember>,
}

// go: internal/rules/type_union_to_schema_union.go:49 AnalyzeTypeUnionToSchemaUnion 898b2abf15c14d10
// Rust: Go's `sf == nil` check drops: the source file is not optional.
pub fn analyze_type_union_to_schema_union(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
) -> Vec<TypeUnionToSchemaUnionMatch> {
    if sf.is_declaration_file() {
        return Vec::new();
    }

    let mut matches = Vec::new();

    // Rust: Go's recursive `walk` closure is a nested function.
    fn walk(
        tp: &mut TypeParser<'_>,
        sf: P<SourceFile>,
        node: P<Node>,
        matches: &mut Vec<TypeUnionToSchemaUnionMatch>,
    ) -> bool {
        if node.kind() == Kind::TypeAliasDeclaration
            && !node.flags().intersects(NodeFlags::Reparsed)
            && let Some(m) = analyze_type_union_to_schema_union_alias(tp, sf, node)
        {
            matches.push(m);
        }
        node.for_each_child(&mut |child| walk(tp, sf, child, matches));
        false
    }

    walk(tp, sf, sf.as_node(), &mut matches);
    matches
}

// go: internal/rules/type_union_to_schema_union.go:75 typeUnionToSchemaUnionLeaf 9fc4bd9b77eae93f
// typeUnionToSchemaUnionLeaf is a syntactically accepted union member. For a
// `<SchemaRef>.<Namespace>.<Side><typeof S>` helper, helper is the <Namespace>
// identifier and helperName the whole type name; both are nil for `typeof S.<Side>`.
#[derive(Clone, Copy)]
pub(crate) struct TypeUnionToSchemaUnionLeaf {
    name: P<Node>,
    side: &'static str,
    helper: Option<P<Node>>,
    helper_name: Option<P<Node>>,
}

// go: internal/rules/type_union_to_schema_union.go:82 analyzeTypeUnionToSchemaUnionAlias 8bb6cc1ab85d068b
// Rust: Go's `alias == nil` check drops (the kind was checked by the caller).
pub(crate) fn analyze_type_union_to_schema_union_alias(
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    node: P<Node>,
) -> Option<TypeUnionToSchemaUnionMatch> {
    let alias = node.as_type_alias_declaration();
    let alias_type = alias.type_()?;
    if node.flags().intersects(NodeFlags::Ambient) {
        return None;
    }
    if alias.type_parameters().is_some_and(|params| !params.nodes().is_empty()) {
        return None;
    }

    let root = ast::skip_type_parentheses(alias_type);
    if root.kind() != Kind::UnionType {
        return None;
    }
    let mut types = Vec::new();
    flatten_type_union_to_schema_union_members(root, &mut types);
    if types.len() < 2 {
        return None;
    }

    // Syntactic checks first: every member must be an accepted form and all
    // members must extract the same side.
    let mut leaves: Vec<TypeUnionToSchemaUnionLeaf> = Vec::with_capacity(types.len());
    for (i, &member) in types.iter().enumerate() {
        let leaf = parse_type_union_to_schema_union_leaf(member)?;
        if i > 0 && leaf.side != leaves[0].side {
            return None;
        }
        leaves.push(leaf);
    }

    // Checker-based checks last: helpers must resolve to Effect's Schema module
    // and every member must be a Schema.
    let mut members = Vec::with_capacity(leaves.len());
    for leaf in &leaves {
        if leaf.helper.is_some() && !is_effect_schema_property_helper(tp, leaf) {
            return None;
        }
        let symbol = tp.get_symbol_at_location(leaf.name)?;
        let t = tp.checker().get_type_of_symbol_at_location(symbol, Some(leaf.name));
        tp.effect_schema_types(t)?;
        members.push(TypeUnionToSchemaUnionMember { name: leaf.name, symbol });
    }

    Some(TypeUnionToSchemaUnionMatch {
        source_file: sf,
        location: ets_shim::scanner::get_error_range_for_node(sf, node),
        alias: node,
        side: leaves[0].side,
        members,
    })
}

// go: internal/rules/type_union_to_schema_union.go:141 isEffectSchemaPropertyHelper 5f6959b30f43f6ca
// isEffectSchemaPropertyHelper reports whether the helper namespace is Effect's
// and the helper itself exists in the installed version: Schema.Schema.Encoded
// only exists in v3 and Schema.Codec.Encoded only in v4.
pub(crate) fn is_effect_schema_property_helper(
    tp: &mut TypeParser<'_>,
    leaf: &TypeUnionToSchemaUnionLeaf,
) -> bool {
    let helper = leaf.helper.expect("a helper leaf");
    // Rust: Go's map read gives a nil slice for a missing side.
    let namespaces = SCHEMA_PROPERTY_HELPER_NAMESPACES.get(leaf.side).copied().unwrap_or_default();
    if !namespaces
        .iter()
        .any(|namespace| tp.is_node_reference_to_effect_schema_module_api(helper, namespace))
    {
        return false;
    }
    let symbol = tp.get_symbol_at_location(leaf.helper_name);
    symbol.is_some_and(|symbol| !symbol.declarations().is_empty())
}

// go: internal/rules/type_union_to_schema_union.go:151 flattenTypeUnionToSchemaUnionMembers 4b83a8097722982d
pub(crate) fn flatten_type_union_to_schema_union_members(node: P<Node>, out: &mut Vec<P<Node>>) {
    let node = ast::skip_type_parentheses(node);
    if node.kind() != Kind::UnionType {
        out.push(node);
        return;
    }
    for &member in node.as_union_type_node().types().nodes() {
        flatten_type_union_to_schema_union_members(member, out);
    }
}

// go: internal/rules/type_union_to_schema_union.go:162 parseTypeUnionToSchemaUnionLeaf 4d138e4814d12d3c
// Rust: Go's nil checks on `ExprName`, `TypeName` and the helper drop: tsrs holds them as non-optional nodes.
pub(crate) fn parse_type_union_to_schema_union_leaf(
    node: P<Node>,
) -> Option<TypeUnionToSchemaUnionLeaf> {
    match node.kind() {
        Kind::TypeQuery => {
            // typeof S.Type / typeof S.Encoded
            let query = node.as_type_query_node();
            if query.type_arguments().is_some_and(|args| !args.nodes().is_empty()) {
                return None;
            }
            let expr_name = query.expr_name();
            if expr_name.kind() != Kind::QualifiedName {
                return None;
            }
            let qualified = expr_name.as_qualified_name();
            let side = type_union_to_schema_union_side(qualified.right())?;
            if !is_identifier_rooted_entity_name(qualified.left()) {
                return None;
            }
            Some(TypeUnionToSchemaUnionLeaf {
                name: qualified.left(),
                side,
                helper: None,
                helper_name: None,
            })
        }
        Kind::TypeReference => {
            // <SchemaRef>.Schema.Type<typeof S> / <SchemaRef>.Codec.Encoded<typeof S>
            let reference = node.as_type_reference_node();
            let type_arguments = reference.type_arguments()?;
            if type_arguments.nodes().len() != 1 {
                return None;
            }
            let type_name = reference.type_name();
            if type_name.kind() != Kind::QualifiedName {
                return None;
            }
            let qualified = type_name.as_qualified_name();
            let side = type_union_to_schema_union_side(qualified.right())?;
            let mut helper = qualified.left();
            if helper.kind() == Kind::QualifiedName {
                helper = helper.as_qualified_name().right();
            }
            if helper.kind() != Kind::Identifier {
                return None;
            }
            let argument = type_arguments.nodes()[0];
            if argument.kind() != Kind::TypeQuery {
                return None;
            }
            let query = argument.as_type_query_node();
            if query.type_arguments().is_some_and(|args| !args.nodes().is_empty()) {
                return None;
            }
            if !is_identifier_rooted_entity_name(query.expr_name()) {
                return None;
            }
            Some(TypeUnionToSchemaUnionLeaf {
                name: query.expr_name(),
                side,
                helper: Some(helper),
                helper_name: Some(type_name),
            })
        }
        _ => None,
    }
}

// go: internal/rules/type_union_to_schema_union.go:218 typeUnionToSchemaUnionSide fab1f01a2fd501a8
pub(crate) fn type_union_to_schema_union_side(name: P<Node>) -> Option<&'static str> {
    if name.kind() != Kind::Identifier {
        return None;
    }
    match name.text() {
        side @ ("Type" | "Encoded") => Some(side),
        _ => None,
    }
}

// go: internal/rules/type_union_to_schema_union.go:229 isIdentifierRootedEntityName f1febf8b28c4ec7a
pub(crate) fn is_identifier_rooted_entity_name(mut node: P<Node>) -> bool {
    while node.kind() == Kind::QualifiedName {
        node = node.as_qualified_name().left();
    }
    node.kind() == Kind::Identifier
}

// go: internal/rules/type_union_to_schema_union.go:238 TypeUnionToSchemaUnionNameParts 3806d6d04bf9a6e2
// TypeUnionToSchemaUnionNameParts returns the names of an entity name: a.b.c
// becomes ["a", "b", "c"].
pub fn type_union_to_schema_union_name_parts(node: P<Node>) -> Vec<String> {
    if node.kind() == Kind::QualifiedName {
        let qualified = node.as_qualified_name();
        let mut parts = type_union_to_schema_union_name_parts(qualified.left());
        parts.push(qualified.right().text().to_string());
        return parts;
    }
    vec![node.text().to_string()]
}
