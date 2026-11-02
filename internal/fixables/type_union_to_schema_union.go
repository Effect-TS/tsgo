package fixables

import (
	"strconv"

	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var TypeUnionToSchemaUnionFix = fixable.Fixable{
	Name:        "typeUnionToSchemaUnion",
	Description: "Convert to Schema.Union",
	ErrorCodes:  []int32{tsdiag.This_type_alias_unions_Schema_0_types_without_a_runtime_schema_compose_them_with_Schema_Union_and_derive_the_type_from_it_effect_typeUnionToSchemaUnion.Code()},
	FixIDs:      []string{"typeUnionToSchemaUnion_fix"},
	Run:         runTypeUnionToSchemaUnionFix,
}

func runTypeUnionToSchemaUnionFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile
	matches := rules.AnalyzeTypeUnionToSchemaUnion(ctx.TypeParser, ctx.Checker, sf)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		// The const is inserted before the alias, so a member declared later in
		// the file would be used before its declaration (TS2448).
		if typeUnionToSchemaUnionHasLateMember(ctx.TypeParser, sf, match) {
			return nil
		}

		alias := match.Alias
		name := typeUnionToSchemaUnionFreeName(ctx.Checker, alias)
		isV4 := ctx.TypeParser.SupportedEffectVersion() == typeparser.EffectMajorV4

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Convert to Schema.Union",
			Run: func(tracker *rewriter.Tracker) {
				schemas := make([]*ast.Node, len(match.Members))
				for i, member := range match.Members {
					schemas[i] = tracker.NewPropertyAccessPath(rules.TypeUnionToSchemaUnionNameParts(member.Name)...)
				}
				arguments := schemas
				if isV4 {
					arguments = []*ast.Node{tracker.NewArrayLiteralExpression(tracker.NewNodeList(schemas), false)}
				}
				union := tracker.NewCallExpression(
					tracker.NewPropertyAccessPath(typeparser.FindModuleReference(sf, "Schema").Append("Union")...),
					nil,
					nil,
					tracker.NewNodeList(arguments),
					ast.NodeFlagsNone,
				)

				var modifiers *ast.ModifierList
				if ast.HasSyntacticModifier(alias, ast.ModifierFlagsExport) {
					modifiers = tracker.NewModifierList([]*ast.Node{tracker.NewModifier(ast.KindExportKeyword)})
				}
				declaration := tracker.NewVariableDeclaration(tracker.NewIdentifier(name), nil, nil, union)
				statement := tracker.NewVariableStatement(
					modifiers,
					tracker.NewVariableDeclarationList(tracker.NewNodeList([]*ast.Node{declaration}), ast.NodeFlagsConst),
				)
				ast.SetParentInChildren(statement)

				tracker.InsertNodeBefore(sf, alias, statement, false, rewriter.LeadingTriviaOptionNone)

				replacement := tracker.NewTypeQueryNode(
					tracker.NewQualifiedName(tracker.NewIdentifier(name), tracker.NewIdentifier(match.Side)),
					nil,
				)
				tracker.ReplaceNode(sf, alias.AsTypeAliasDeclaration().Type, replacement, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}
	return nil
}

// typeUnionToSchemaUnionHasLateMember reports whether any member, or any prefix
// of a qualified member such as Models in Models.Cat, is declared in this file
// after the alias. The local symbol is checked, so `const Late = Imported`
// declared after the alias counts as late.
func typeUnionToSchemaUnionHasLateMember(tp *typeparser.TypeParser, sf *ast.SourceFile, match rules.TypeUnionToSchemaUnionMatch) bool {
	aliasEnd := match.Alias.End()
	for _, member := range match.Members {
		for name := member.Name; name != nil; {
			if symbol := tp.GetSymbolAtLocation(name); symbol != nil {
				for _, declaration := range ast.TSGetSymbolDeclarations(symbol) {
					if ast.GetSourceFileOfNode(declaration) == sf && declaration.Pos() >= aliasEnd {
						return true
					}
				}
			}
			if name.Kind != ast.KindQualifiedName {
				break
			}
			name = name.AsQualifiedName().Left.AsNode()
		}
	}
	return false
}

// typeUnionToSchemaUnionFreeName picks the const name: the alias name, or
// <Alias>Schema, <Alias>Schema2, ... when a value with that name is in scope.
func typeUnionToSchemaUnionFreeName(c *checker.Checker, alias *ast.Node) string {
	base := alias.Name().Text()
	isFree := func(candidate string) bool {
		return c.ResolveName(candidate, alias, ast.SymbolFlagsValue|ast.SymbolFlagsAlias, false) == nil
	}
	if isFree(base) {
		return base
	}
	if candidate := base + "Schema"; isFree(candidate) {
		return candidate
	}
	for i := 2; ; i++ {
		if candidate := base + "Schema" + strconv.Itoa(i); isFree(candidate) {
			return candidate
		}
	}
}
