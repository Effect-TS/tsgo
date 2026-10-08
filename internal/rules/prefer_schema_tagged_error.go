package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var PreferSchemaTaggedError = rule.Rule{
	Name:            "preferSchemaTaggedError",
	Group:           "style",
	Description:     "Suggests using Schema.TaggedError instead of Data.TaggedError",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Prefer_Schema_TaggedError_over_Data_TaggedError_for_an_explicit_schema_contract_effect_preferSchemaTaggedError.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzePreferSchemaTaggedError(ctx.TypeParser, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(match.SourceFile, match.Location, tsdiag.Prefer_Schema_TaggedError_over_Data_TaggedError_for_an_explicit_schema_contract_effect_preferSchemaTaggedError, nil)
		}
		return diags
	},
}

type PreferSchemaTaggedErrorMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
}

func AnalyzePreferSchemaTaggedError(tp *typeparser.TypeParser, sf *ast.SourceFile) []PreferSchemaTaggedErrorMatch {
	var matches []PreferSchemaTaggedErrorMatch
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindClassDeclaration {
			if result := tp.ExtendsDataTaggedError(node); result != nil {
				locationNode := result.ClassName
				if locationNode == nil {
					locationNode = result.CallExpression
				}
				matches = append(matches, PreferSchemaTaggedErrorMatch{
					SourceFile: sf,
					Location:   scanner.GetErrorRangeForNode(sf, locationNode),
				})
			}
		}
		node.ForEachChild(walk)
		return false
	}
	walk(sf.AsNode())
	return matches
}
