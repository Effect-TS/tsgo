package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var PreferSchemaTypeProperty = rule.Rule{
	Name:            "preferSchemaTypeProperty",
	Group:           "style",
	Description:     "Disallows Schema.Schema.Type<typeof X> in favor of typeof X.Type",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Do_not_use_Schema_Schema_Type_typeof_X_to_extract_a_schema_s_type_Use_typeof_X_Type_instead_effect_preferSchemaTypeProperty.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzePreferSchemaTypeProperty(ctx.TypeParser, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(match.SourceFile, match.Location, tsdiag.Do_not_use_Schema_Schema_Type_typeof_X_to_extract_a_schema_s_type_Use_typeof_X_Type_instead_effect_preferSchemaTypeProperty, nil)
		}
		return diags
	},
}

type PreferSchemaTypePropertyMatch struct {
	SourceFile     *ast.SourceFile
	Location       core.TextRange
	TypeReference  *ast.Node
	SchemaName     *ast.Node
	SchemaNameText string
}

func AnalyzePreferSchemaTypeProperty(tp *typeparser.TypeParser, sf *ast.SourceFile) []PreferSchemaTypePropertyMatch {
	var matches []PreferSchemaTypePropertyMatch
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindTypeReference {
			if match := analyzePreferSchemaTypePropertyNode(tp, sf, node); match != nil {
				matches = append(matches, *match)
				return false
			}
		}
		node.ForEachChild(walk)
		return false
	}
	walk(sf.AsNode())
	return matches
}

func analyzePreferSchemaTypePropertyNode(tp *typeparser.TypeParser, sf *ast.SourceFile, node *ast.Node) *PreferSchemaTypePropertyMatch {
	reference := node.AsTypeReferenceNode()
	if reference == nil || reference.TypeName == nil || reference.TypeArguments == nil || len(reference.TypeArguments.Nodes) != 1 {
		return nil
	}

	typeName := reference.TypeName.AsNode()
	if typeName == nil || typeName.Kind != ast.KindQualifiedName {
		return nil
	}
	outerName := typeName.AsQualifiedName()
	if outerName.Right == nil || outerName.Right.Text() != "Type" || outerName.Left == nil {
		return nil
	}
	middleName := outerName.Left.AsNode()
	if middleName == nil || middleName.Kind != ast.KindQualifiedName {
		return nil
	}
	schemaExport := middleName.AsQualifiedName().Right
	if schemaExport == nil || !tp.IsNodeReferenceToEffectSchemaModuleApi(schemaExport, "Schema") {
		return nil
	}

	typeArgument := reference.TypeArguments.Nodes[0]
	if typeArgument == nil || typeArgument.Kind != ast.KindTypeQuery {
		return nil
	}
	query := typeArgument.AsTypeQueryNode()
	if query.ExprName == nil || query.TypeArguments != nil && len(query.TypeArguments.Nodes) > 0 {
		return nil
	}

	return &PreferSchemaTypePropertyMatch{
		SourceFile:     sf,
		Location:       scanner.GetErrorRangeForNode(sf, node),
		TypeReference:  node,
		SchemaName:     query.ExprName.AsNode(),
		SchemaNameText: scanner.GetTextOfNode(query.ExprName.AsNode()),
	}
}
