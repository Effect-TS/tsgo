package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// EffectMapFlatten suggests using Effect.flatMap instead of Effect.map followed
// by Effect.flatten in piping flows.
var EffectMapFlatten = rule.Rule{
	Name:            "effectMapFlatten",
	Group:           "style",
	Description:     "Suggests using Effect.flatMap instead of Effect.map followed by Effect.flatten in piping flows",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_map_Effect_flatten_is_the_same_as_Effect_flatMap_that_expresses_the_same_steps_more_directly_effect_effectMapFlatten.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeEffectMapFlatten(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_map_Effect_flatten_is_the_same_as_Effect_flatMap_that_expresses_the_same_steps_more_directly_effect_effectMapFlatten,
				nil,
			)
		}
		return diags
	},
}

type EffectMapFlattenMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	Node       *ast.Node
}

// AnalyzeEffectMapFlatten finds adjacent Effect.map(...), Effect.flatten pairs
// in pipe/pipeable flows.
func AnalyzeEffectMapFlatten(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []EffectMapFlattenMatch {
	var matches []EffectMapFlattenMatch

	flows := tp.PipingFlows(sf, false)
	for _, flow := range flows {
		for i := range len(flow.Transformations) - 1 {
			mapTransformation := flow.Transformations[i]
			flattenTransformation := flow.Transformations[i+1]

			if len(mapTransformation.Args) == 0 || len(flattenTransformation.Args) != 0 {
				continue
			}

			if (mapTransformation.Kind != typeparser.TransformationKindPipe && mapTransformation.Kind != typeparser.TransformationKindPipeable) || flattenTransformation.Kind != mapTransformation.Kind {
				continue
			}

			if !tp.IsNodeReferenceToEffectModuleApi(mapTransformation.Callee, "map") || !tp.IsNodeReferenceToEffectModuleApi(flattenTransformation.Callee, "flatten") {
				continue
			}

			matches = append(matches, EffectMapFlattenMatch{
				SourceFile: sf,
				Location:   scanner.GetErrorRangeForNode(sf, flattenTransformation.Callee),
				Node:       flattenTransformation.Callee,
			})
		}
	}

	return matches
}
