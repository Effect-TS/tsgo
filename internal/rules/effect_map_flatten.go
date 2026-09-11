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
		sequences := flow.FindTransformationSequences(
			func(transformation *typeparser.PipingFlowTransformation) bool {
				return len(transformation.Args) > 0 && tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "map")
			},
			func(transformation *typeparser.PipingFlowTransformation) bool {
				return len(transformation.Args) == 0 && tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "flatten")
			},
		)
		for _, sequence := range sequences {
			mapTransformation := &flow.Transformations[sequence.Start]
			flattenTransformation := &flow.Transformations[sequence.Start+1]
			if (mapTransformation.Kind != typeparser.TransformationKindPipe && mapTransformation.Kind != typeparser.TransformationKindPipeable) || flattenTransformation.Kind != mapTransformation.Kind {
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
