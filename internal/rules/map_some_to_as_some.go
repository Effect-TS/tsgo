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

// MapSomeToAsSome suggests Effect.asSome when Effect.map only wraps the
// success value with Option.some.
var MapSomeToAsSome = rule.Rule{
	Name:            "mapSomeToAsSome",
	Group:           "style",
	Description:     "Suggests using Effect.asSome instead of Effect.map when the mapper only wraps the success value with Option.some",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_asSome_expresses_wrapping_the_success_value_in_Option_some_directly_effect_mapSomeToAsSome.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeMapSomeToAsSome(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_asSome_expresses_wrapping_the_success_value_in_Option_some_directly_effect_mapSomeToAsSome,
				nil,
			)
		}
		return diags
	},
}

// MapSomeToAsSomeMatch holds the nodes needed by the diagnostic and quick fix.
type MapSomeToAsSomeMatch struct {
	SourceFile       *ast.SourceFile
	Location         core.TextRange
	Transformation   *typeparser.PipingFlowTransformation
	EffectModuleNode *ast.Node
}

// AnalyzeMapSomeToAsSome finds data-last and data-first Effect.map calls whose
// mapper is Option.some or an identity forwarder such as value => Option.some(value).
func AnalyzeMapSomeToAsSome(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []MapSomeToAsSomeMatch {
	var matches []MapSomeToAsSomeMatch
	for _, flow := range tp.PipingFlows(sf, true) {
		for index := range flow.Transformations {
			transformation := &flow.Transformations[index]
			if len(transformation.Args) != 1 ||
				transformation.Callee == nil || transformation.Callee.Kind != ast.KindPropertyAccessExpression ||
				!tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "map") {
				continue
			}

			if transformation.TypeArguments != nil && len(transformation.TypeArguments.Nodes) > 0 ||
				!isOptionSomeMapper(tp, transformation.Args[0]) {
				continue
			}
			propertyAccess := transformation.Callee.AsPropertyAccessExpression()
			matches = append(matches, MapSomeToAsSomeMatch{
				SourceFile:       sf,
				Location:         scanner.GetErrorRangeForNode(sf, transformation.Callee),
				Transformation:   transformation,
				EffectModuleNode: propertyAccess.Expression,
			})
		}
	}
	return matches
}

func isOptionSomeMapper(tp *typeparser.TypeParser, node *ast.Node) bool {
	mapper, typeArguments, parameter := tp.UnwrapIdentityForwarder(node)
	if typeArguments != nil && len(typeArguments.Nodes) > 0 {
		return false
	}
	if parameter != nil {
		parameterDeclaration := parameter.AsParameterDeclaration()
		if parameterDeclaration == nil || parameterDeclaration.QuestionToken != nil || parameterDeclaration.Type != nil {
			return false
		}
	}
	return tp.IsNodeReferenceToEffectOptionModuleApi(mapper, "some")
}
