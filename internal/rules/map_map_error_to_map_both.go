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

// MapMapErrorToMapBoth suggests using Effect.mapBoth instead of directly adjacent
// Effect.map and Effect.mapError in piping flows or nested data-first calls.
var MapMapErrorToMapBoth = rule.Rule{
	Name:            "mapMapErrorToMapBoth",
	Group:           "style",
	Description:     "Suggests using Effect.mapBoth instead of directly adjacent Effect.map and Effect.mapError",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_mapBoth_expresses_these_failure_and_success_transformations_more_directly_than_adjacent_Effect_map_and_Effect_mapError_effect_mapMapErrorToMapBoth.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeMapMapErrorToMapBoth(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_mapBoth_expresses_these_failure_and_success_transformations_more_directly_than_adjacent_Effect_map_and_Effect_mapError_effect_mapMapErrorToMapBoth,
				nil,
			)
		}
		return diags
	},
}

// MapMapErrorToMapBothMatch holds match details for diagnostic emission.
type MapMapErrorToMapBothMatch struct {
	SourceFile   *ast.SourceFile
	Location     core.TextRange
	Node         *ast.Node
	FirstCallee  *ast.Node
	SecondCallee *ast.Node
}

// AnalyzeMapMapErrorToMapBoth finds directly adjacent Effect.map and Effect.mapError
// combinators in either order applied to an Effect receiver.
func AnalyzeMapMapErrorToMapBoth(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []MapMapErrorToMapBothMatch {
	if tp == nil || sf == nil {
		return nil
	}

	var matches []MapMapErrorToMapBothMatch

	flows := tp.PipingFlows(sf, false)
	for _, flow := range flows {
		sequences := flow.FindTransformationSequences(
			func(transformation *typeparser.PipingFlowTransformation) bool {
				return len(transformation.Args) > 0 &&
					(tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "map") ||
						tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "mapError"))
			},
			func(transformation *typeparser.PipingFlowTransformation) bool {
				return len(transformation.Args) > 0 &&
					(tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "map") ||
						tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "mapError"))
			},
		)

		nextAllowedIndex := 0
		for _, sequence := range sequences {
			if sequence.Start < nextAllowedIndex {
				continue
			}

			first := &flow.Transformations[sequence.Start]
			second := &flow.Transformations[sequence.Start+1]

			if first.Kind != second.Kind {
				continue
			}
			if first.Kind != typeparser.TransformationKindPipe &&
				first.Kind != typeparser.TransformationKindPipeable &&
				first.Kind != typeparser.TransformationKindDataFirst {
				continue
			}

		isFirstMap := tp.IsNodeReferenceToEffectModuleApi(first.Callee, "map")
		isFirstMapError := tp.IsNodeReferenceToEffectModuleApi(first.Callee, "mapError")
		isSecondMap := tp.IsNodeReferenceToEffectModuleApi(second.Callee, "map")
		isSecondMapError := tp.IsNodeReferenceToEffectModuleApi(second.Callee, "mapError")
		isMapPair := (isFirstMap && isSecondMapError) || (isFirstMapError && isSecondMap)

		if !isMapPair {
			continue
		}

			inputType := flow.TransformationInputType(sequence.Start)
			if inputType == nil {
				if sequence.Start == 0 && flow.Subject.Node != nil {
					inputType = tp.GetTypeAtLocation(flow.Subject.Node)
				} else if inputNode := flow.TransformationInputNode(sequence.Start); inputNode != nil {
					inputType = tp.GetTypeAtLocation(inputNode)
				}
			}
			if inputType == nil || !tp.IsEffectType(inputType) || tp.IsEffectSubtype(inputType) {
				continue
			}

			nextAllowedIndex = sequence.Start + 2
			matches = append(matches, MapMapErrorToMapBothMatch{
				SourceFile:   sf,
				Location:     scanner.GetErrorRangeForNode(sf, second.Callee),
				Node:         second.Callee,
				FirstCallee:  first.Callee,
				SecondCallee: second.Callee,
			})
		}
	}

	return matches
}
