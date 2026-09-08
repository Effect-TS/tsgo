package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var PreferSchemaUnion = rule.Rule{
	Name:            "preferSchemaUnion",
	Group:           "style",
	Description:     "Suggests composing Schema values with Schema.Union instead of unioning their extracted Type properties",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.This_type_alias_unions_decoded_Effect_Schema_types_Prefer_Schema_Union_and_derive_the_type_from_the_resulting_schema_effect_preferSchemaUnion.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzePreferSchemaUnion(ctx.TypeParser, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.This_type_alias_unions_decoded_Effect_Schema_types_Prefer_Schema_Union_and_derive_the_type_from_the_resulting_schema_effect_preferSchemaUnion,
				nil,
			)
		}
		return diags
	},
}

type PreferSchemaUnionMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	Alias      *ast.Node
	Schemas    []*ast.Node
}

func AnalyzePreferSchemaUnion(tp *typeparser.TypeParser, sf *ast.SourceFile) []PreferSchemaUnionMatch {
	if tp == nil || sf == nil || sf.IsDeclarationFile {
		return nil
	}
	if sf.ScriptKind != core.ScriptKindTS && sf.ScriptKind != core.ScriptKindTSX && sf.ScriptKind != core.ScriptKindUnknown {
		return nil
	}
	if tp.DetectEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}

	var matches []PreferSchemaUnionMatch
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindTypeAliasDeclaration {
			if match := analyzePreferSchemaUnionAlias(tp, sf, node); match != nil {
				matches = append(matches, *match)
			}
		}
		node.ForEachChild(walk)
		return false
	}
	walk(sf.AsNode())
	return matches
}

func analyzePreferSchemaUnionAlias(tp *typeparser.TypeParser, sf *ast.SourceFile, node *ast.Node) *PreferSchemaUnionMatch {
	if node.Flags&ast.NodeFlagsAmbient != 0 || ast.HasAmbientModifier(node) {
		return nil
	}

	ta := node.AsTypeAliasDeclaration()
	if ta == nil || ta.Type == nil {
		return nil
	}
	if ta.TypeParameters != nil && len(ta.TypeParameters.Nodes) > 0 {
		return nil
	}

	unwrapped := ast.SkipTypeParentheses(ta.Type.AsNode())
	if unwrapped == nil || unwrapped.Kind != ast.KindUnionType {
		return nil
	}

	leaves := collectUnionLeaves(unwrapped)
	if len(leaves) < 2 {
		return nil
	}

	schemas := make([]*ast.Node, 0, len(leaves))
	for _, leaf := range leaves {
		if leaf.Kind != ast.KindTypeQuery {
			return nil
		}
		query := leaf.AsTypeQueryNode()
		if query == nil || query.ExprName == nil {
			return nil
		}
		if query.TypeArguments != nil && len(query.TypeArguments.Nodes) > 0 {
			return nil
		}

		exprName := query.ExprName.AsNode()
		if exprName == nil || exprName.Kind != ast.KindQualifiedName {
			return nil
		}
		qual := exprName.AsQualifiedName()
		if qual.Right == nil || qual.Right.Text() != "Type" || qual.Left == nil {
			return nil
		}

		schemaNode := qual.Left.AsNode()
		t := getSchemaNodeType(tp, schemaNode)
		if t == nil || t.Flags()&checker.TypeFlagsAnyOrUnknown != 0 {
			return nil
		}
		if !tp.IsSchemaType(t) {
			return nil
		}

		schemas = append(schemas, schemaNode)
	}

	return &PreferSchemaUnionMatch{
		SourceFile: sf,
		Location:   scanner.GetErrorRangeForNode(sf, ta.Type.AsNode()),
		Alias:      node,
		Schemas:    schemas,
	}
}

func collectUnionLeaves(node *ast.Node) []*ast.Node {
	node = ast.SkipTypeParentheses(node)
	if node == nil {
		return nil
	}
	if node.Kind == ast.KindUnionType {
		ut := node.AsUnionTypeNode()
		if ut == nil || ut.Types == nil {
			return nil
		}
		var leaves []*ast.Node
		for _, child := range ut.Types.Nodes {
			childLeaves := collectUnionLeaves(child)
			if childLeaves == nil {
				return nil
			}
			leaves = append(leaves, childLeaves...)
		}
		return leaves
	}
	return []*ast.Node{node}
}

func getSchemaNodeType(tp *typeparser.TypeParser, node *ast.Node) *checker.Type {
	if tp == nil || node == nil {
		return nil
	}
	t := tp.GetTypeAtLocation(node)
	if t != nil && t.Flags()&checker.TypeFlagsAnyOrUnknown == 0 {
		return t
	}
	sym := tp.GetSymbolAtLocation(node)
	if sym == nil && node.Kind == ast.KindQualifiedName {
		qual := node.AsQualifiedName()
		if qual.Right != nil {
			sym = tp.GetSymbolAtLocation(qual.Right.AsNode())
		}
	}
	if sym != nil {
		return tp.GetTypeOfSymbolAtLocation(sym, node)
	}
	return nil
}
