package rules

import (
	"slices"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var TypeUnionToSchemaUnion = rule.Rule{
	Name:            "typeUnionToSchemaUnion",
	Group:           "style",
	Description:     "Suggests composing unions of Schema .Type/.Encoded types with Schema.Union and deriving the type from it",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_type_alias_unions_Schema_0_types_without_a_runtime_schema_compose_them_with_Schema_Union_and_derive_the_type_from_it_effect_typeUnionToSchemaUnion.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeTypeUnionToSchemaUnion(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(match.SourceFile, match.Location, tsdiag.This_type_alias_unions_Schema_0_types_without_a_runtime_schema_compose_them_with_Schema_Union_and_derive_the_type_from_it_effect_typeUnionToSchemaUnion, nil, match.Side)
		}
		return diags
	},
}

// TypeUnionToSchemaUnionMember is one schema of the union: Name is the entity
// name S taken from `typeof S.Type` or `Schema.Schema.Type<typeof S>`.
type TypeUnionToSchemaUnionMember struct {
	Name   *ast.Node
	Symbol *ast.Symbol
}

type TypeUnionToSchemaUnionMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	Alias      *ast.Node
	// Side is the extracted schema side, "Type" or "Encoded".
	Side    string
	Members []TypeUnionToSchemaUnionMember
}

func AnalyzeTypeUnionToSchemaUnion(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []TypeUnionToSchemaUnionMatch {
	if sf == nil || sf.IsDeclarationFile {
		return nil
	}

	var matches []TypeUnionToSchemaUnionMatch
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindTypeAliasDeclaration && node.Flags&ast.NodeFlagsReparsed == 0 {
			if match := analyzeTypeUnionToSchemaUnionAlias(tp, c, sf, node); match != nil {
				matches = append(matches, *match)
			}
		}
		node.ForEachChild(walk)
		return false
	}
	walk(sf.AsNode())
	return matches
}

// typeUnionToSchemaUnionLeaf is a syntactically accepted union member. For a
// `<SchemaRef>.<Namespace>.<Side><typeof S>` helper, helper is the <Namespace>
// identifier and helperName the whole type name; both are nil for `typeof S.<Side>`.
type typeUnionToSchemaUnionLeaf struct {
	name       *ast.Node
	side       string
	helper     *ast.Node
	helperName *ast.Node
}

func analyzeTypeUnionToSchemaUnionAlias(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, node *ast.Node) *TypeUnionToSchemaUnionMatch {
	alias := node.AsTypeAliasDeclaration()
	if alias == nil || alias.Type == nil || node.Flags&ast.NodeFlagsAmbient != 0 {
		return nil
	}
	if alias.TypeParameters != nil && len(alias.TypeParameters.Nodes) > 0 {
		return nil
	}

	root := ast.SkipTypeParentheses(alias.Type)
	if root == nil || root.Kind != ast.KindUnionType {
		return nil
	}
	var types []*ast.Node
	flattenTypeUnionToSchemaUnionMembers(root, &types)
	if len(types) < 2 {
		return nil
	}

	// Syntactic checks first: every member must be an accepted form and all
	// members must extract the same side.
	leaves := make([]typeUnionToSchemaUnionLeaf, len(types))
	for i, member := range types {
		leaf, ok := parseTypeUnionToSchemaUnionLeaf(member)
		if !ok || i > 0 && leaf.side != leaves[0].side {
			return nil
		}
		leaves[i] = leaf
	}

	// Checker-based checks last: helpers must resolve to Effect's Schema module
	// and every member must be a Schema.
	members := make([]TypeUnionToSchemaUnionMember, len(leaves))
	for i, leaf := range leaves {
		if leaf.helper != nil && !isEffectSchemaPropertyHelper(tp, leaf) {
			return nil
		}
		symbol := tp.GetSymbolAtLocation(leaf.name)
		if symbol == nil {
			return nil
		}
		if tp.EffectSchemaTypes(c.GetTypeOfSymbolAtLocation(symbol, leaf.name)) == nil {
			return nil
		}
		members[i] = TypeUnionToSchemaUnionMember{Name: leaf.name, Symbol: symbol}
	}

	return &TypeUnionToSchemaUnionMatch{
		SourceFile: sf,
		Location:   scanner.GetErrorRangeForNode(sf, node),
		Alias:      node,
		Side:       leaves[0].side,
		Members:    members,
	}
}

// isEffectSchemaPropertyHelper reports whether the helper namespace is Effect's
// and the helper itself exists in the installed version: Schema.Schema.Encoded
// only exists in v3 and Schema.Codec.Encoded only in v4.
func isEffectSchemaPropertyHelper(tp *typeparser.TypeParser, leaf typeUnionToSchemaUnionLeaf) bool {
	if !slices.ContainsFunc(schemaPropertyHelperNamespaces[leaf.side], func(namespace string) bool {
		return tp.IsNodeReferenceToEffectSchemaModuleApi(leaf.helper, namespace)
	}) {
		return false
	}
	symbol := tp.GetSymbolAtLocation(leaf.helperName)
	return symbol != nil && len(ast.TSGetSymbolDeclarations(symbol)) > 0
}

func flattenTypeUnionToSchemaUnionMembers(node *ast.Node, out *[]*ast.Node) {
	node = ast.SkipTypeParentheses(node)
	if node.Kind != ast.KindUnionType {
		*out = append(*out, node)
		return
	}
	for _, member := range node.AsUnionTypeNode().Types.Nodes {
		flattenTypeUnionToSchemaUnionMembers(member, out)
	}
}

func parseTypeUnionToSchemaUnionLeaf(node *ast.Node) (typeUnionToSchemaUnionLeaf, bool) {
	switch node.Kind {
	case ast.KindTypeQuery:
		// typeof S.Type / typeof S.Encoded
		query := node.AsTypeQueryNode()
		if query.ExprName == nil || query.TypeArguments != nil && len(query.TypeArguments.Nodes) > 0 {
			return typeUnionToSchemaUnionLeaf{}, false
		}
		exprName := query.ExprName.AsNode()
		if exprName.Kind != ast.KindQualifiedName {
			return typeUnionToSchemaUnionLeaf{}, false
		}
		qualified := exprName.AsQualifiedName()
		side, ok := typeUnionToSchemaUnionSide(qualified.Right)
		if !ok || !isIdentifierRootedEntityName(qualified.Left.AsNode()) {
			return typeUnionToSchemaUnionLeaf{}, false
		}
		return typeUnionToSchemaUnionLeaf{name: qualified.Left.AsNode(), side: side}, true
	case ast.KindTypeReference:
		// <SchemaRef>.Schema.Type<typeof S> / <SchemaRef>.Codec.Encoded<typeof S>
		reference := node.AsTypeReferenceNode()
		if reference.TypeName == nil || reference.TypeArguments == nil || len(reference.TypeArguments.Nodes) != 1 {
			return typeUnionToSchemaUnionLeaf{}, false
		}
		typeName := reference.TypeName.AsNode()
		if typeName.Kind != ast.KindQualifiedName {
			return typeUnionToSchemaUnionLeaf{}, false
		}
		qualified := typeName.AsQualifiedName()
		side, ok := typeUnionToSchemaUnionSide(qualified.Right)
		if !ok {
			return typeUnionToSchemaUnionLeaf{}, false
		}
		helper := qualified.Left.AsNode()
		if helper.Kind == ast.KindQualifiedName {
			helper = helper.AsQualifiedName().Right
		}
		if helper == nil || helper.Kind != ast.KindIdentifier {
			return typeUnionToSchemaUnionLeaf{}, false
		}
		argument := reference.TypeArguments.Nodes[0]
		if argument.Kind != ast.KindTypeQuery {
			return typeUnionToSchemaUnionLeaf{}, false
		}
		query := argument.AsTypeQueryNode()
		if query.ExprName == nil || query.TypeArguments != nil && len(query.TypeArguments.Nodes) > 0 {
			return typeUnionToSchemaUnionLeaf{}, false
		}
		if !isIdentifierRootedEntityName(query.ExprName.AsNode()) {
			return typeUnionToSchemaUnionLeaf{}, false
		}
		return typeUnionToSchemaUnionLeaf{name: query.ExprName.AsNode(), side: side, helper: helper, helperName: typeName}, true
	}
	return typeUnionToSchemaUnionLeaf{}, false
}

func typeUnionToSchemaUnionSide(name *ast.Node) (string, bool) {
	if name == nil || name.Kind != ast.KindIdentifier {
		return "", false
	}
	switch side := name.Text(); side {
	case "Type", "Encoded":
		return side, true
	}
	return "", false
}

func isIdentifierRootedEntityName(node *ast.Node) bool {
	for node != nil && node.Kind == ast.KindQualifiedName {
		node = node.AsQualifiedName().Left.AsNode()
	}
	return node != nil && node.Kind == ast.KindIdentifier
}

// TypeUnionToSchemaUnionNameParts returns the names of an entity name: a.b.c
// becomes ["a", "b", "c"].
func TypeUnionToSchemaUnionNameParts(node *ast.Node) []string {
	if node.Kind == ast.KindQualifiedName {
		qualified := node.AsQualifiedName()
		return append(TypeUnionToSchemaUnionNameParts(qualified.Left.AsNode()), qualified.Right.Text())
	}
	return []string{node.Text()}
}
