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

// MapMapErrorToMapBoth suggests Effect.mapBoth when Effect.map and
// Effect.mapError are adjacent steps of the same piping flow.
var MapMapErrorToMapBoth = rule.Rule{
	Name:            "mapMapErrorToMapBoth",
	Group:           "style",
	Description:     "Suggests Effect.mapBoth when Effect.map and Effect.mapError are adjacent steps of a piping flow",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_0_Effect_1_transform_the_success_and_failure_channels_one_step_at_a_time_Effect_2_expresses_both_transformations_in_a_single_step_effect_mapMapErrorToMapBoth.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeMapMapErrorToMapBoth(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(match.SourceFile, match.Location,
				tsdiag.Effect_0_Effect_1_transform_the_success_and_failure_channels_one_step_at_a_time_Effect_2_expresses_both_transformations_in_a_single_step_effect_mapMapErrorToMapBoth,
				nil, match.FirstName, match.SecondName, match.ReplacementName)
		}
		return diagnostics
	},
}

type MapMapErrorToMapBothMatch struct {
	SourceFile            *ast.SourceFile
	Flow                  *typeparser.PipingFlow
	Start                 int
	Location              core.TextRange
	FirstName, SecondName string
	ReplacementName       string
	// FirstFailure is true when the first step transforms the failure channel.
	FirstFailure                  bool
	EffectModule                  *ast.Node
	FirstCallback, SecondCallback *ast.Node
	Fixable                       bool
}

type mapMapErrorStep struct {
	name    string
	failure bool
	eager   bool
}

var mapMapErrorSteps = [...]mapMapErrorStep{
	{name: "map"},
	{name: "mapError", failure: true},
	{name: "mapEager", eager: true},
	{name: "mapErrorEager", failure: true, eager: true},
}

// AnalyzeMapMapErrorToMapBoth finds adjacent success and failure mappings in
// piping flows. Pairs are taken left to right without overlap.
func AnalyzeMapMapErrorToMapBoth(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []MapMapErrorToMapBothMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}
	isCandidate := func(step typeparser.PipingFlowTransformation) bool {
		_, ok := classifyMapMapErrorStep(tp, step)
		return ok
	}

	var matches []MapMapErrorToMapBothMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		next := 0
		for _, sequence := range flow.FindTransformationSequences(isCandidate, isCandidate).All() {
			i := sequence.Start()
			if i < next {
				continue
			}
			first := flow.Transformations().At(i)
			second := flow.Transformations().At(i + 1)
			a, _ := classifyMapMapErrorStep(tp, first)
			b, _ := classifyMapMapErrorStep(tp, second)
			if a.failure == b.failure || a.eager != b.eager || !mapMapErrorSameLocation(first, second) {
				continue
			}
			next = i + 2

			replacement := "mapBoth"
			if a.eager {
				replacement = "mapBothEager"
			}
			var module *ast.Node
			if second.Callee().Kind == ast.KindPropertyAccessExpression {
				module = second.Callee().AsPropertyAccessExpression().Expression
			} else if first.Callee().Kind == ast.KindPropertyAccessExpression {
				module = first.Callee().AsPropertyAccessExpression().Expression
			}
			matches = append(matches, MapMapErrorToMapBothMatch{
				SourceFile:      sf,
				Flow:            flow,
				Start:           i,
				Location:        scanner.GetErrorRangeForNode(sf, second.Callee()),
				FirstName:       a.name,
				SecondName:      b.name,
				ReplacementName: replacement,
				FirstFailure:    a.failure,
				EffectModule:    module,
				FirstCallback:   first.Args().At(0),
				SecondCallback:  second.Args().At(0),
				Fixable:         !catchDieHasTypeArguments(&first) && !catchDieHasTypeArguments(&second),
			})
		}
	}
	return matches
}

func classifyMapMapErrorStep(tp *typeparser.TypeParser, step typeparser.PipingFlowTransformation) (mapMapErrorStep, bool) {
	if step.Callee() == nil || step.Args().Len() != 1 || step.Args().At(0).Kind == ast.KindSpreadElement {
		return mapMapErrorStep{}, false
	}
	for _, candidate := range mapMapErrorSteps {
		if tp.IsNodeReferenceToEffectModuleApi(step.Callee(), candidate.name) {
			return candidate, true
		}
	}
	return mapMapErrorStep{}, false
}

// mapMapErrorSameLocation requires both steps to share one argument list, or
// the first data-first call to be the subject of the second one. Piping flows
// merge chained pipe calls, so equal kinds alone do not guarantee this.
func mapMapErrorSameLocation(first, second typeparser.PipingFlowTransformation) bool {
	kind := first.Kind()
	if kind != second.Kind() {
		return false
	}
	firstCall := mapMapErrorStepCall(first)
	secondCall := mapMapErrorStepCall(second)
	if firstCall == nil || secondCall == nil {
		return false
	}
	switch kind {
	case typeparser.TransformationKindPipe, typeparser.TransformationKindPipeable, typeparser.TransformationKindEffectFn:
		return firstCall.Parent != nil && firstCall.Parent == secondCall.Parent && firstCall.Parent.Kind == ast.KindCallExpression
	case typeparser.TransformationKindDataFirst:
		arguments := secondCall.AsCallExpression().Arguments
		return arguments != nil && len(arguments.Nodes) > 0 && ast.SkipParentheses(arguments.Nodes[0]) == firstCall
	}
	return false
}

func mapMapErrorStepCall(step typeparser.PipingFlowTransformation) *ast.Node {
	call := step.Callee().Parent
	if call == nil || call.Kind != ast.KindCallExpression || call.AsCallExpression().Expression != step.Callee() {
		return nil
	}
	return call
}
