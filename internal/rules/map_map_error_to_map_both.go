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
	FirstFailure bool
	// First and Second are the two steps in flow order.
	First, Second typeparser.PipingFlowTransformation
	Fixable       bool
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
			if a.failure == b.failure || a.eager != b.eager {
				continue
			}
			next = i + 2

			replacement := "mapBoth"
			if a.eager {
				replacement = "mapBothEager"
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
				First:           first,
				Second:          second,
				Fixable: !mapMapErrorHasTypeArguments(first) && !mapMapErrorHasTypeArguments(second) &&
					mapMapErrorKeepsEvaluationOrder(first, second, a.eager),
			})
		}
	}
	return matches
}

// mapMapErrorHasTypeArguments reports explicit type arguments on any call the
// fix edits: the step itself, the application of a curried step, or the pipe or
// Effect.fn call whose argument list holds the step. Those type arguments
// describe signatures that the merged step or the shortened argument list no
// longer has.
func mapMapErrorHasTypeArguments(step typeparser.PipingFlowTransformation) bool {
	if hasExplicitTypeArguments(&step) {
		return true
	}
	if step.Kind() != typeparser.TransformationKindCall && !isPipingFlowArgumentKind(step.Kind()) {
		return false
	}
	outer := step.Callee().Parent
	if outer != nil {
		outer = outer.Parent
	}
	for outer != nil && outer.Kind == ast.KindParenthesizedExpression {
		outer = outer.Parent
	}
	if outer == nil || outer.Kind != ast.KindCallExpression {
		return false
	}
	typeArguments := outer.AsCallExpression().TypeArguments
	return typeArguments != nil && len(typeArguments.Nodes) > 0
}

// Positions in the evaluation order modeled by mapMapErrorKeepsEvaluationOrder.
const (
	mapMapErrorInput = iota // the first step's input
	mapMapErrorFirstCallback
	mapMapErrorSecondCallback
)

// mapMapErrorKeepsEvaluationOrder reports whether the merged step evaluates the
// first step's input and both callbacks, and runs the first step, in the same
// observable order as the original code. A curried or data-last step evaluates
// its callback before its input; every other step evaluates its input first.
// The merged step keeps the second step's form with the callbacks in flow
// order. Only a function literal can move without running user code.
func mapMapErrorKeepsEvaluationOrder(first, second typeparser.PipingFlowTransformation, eager bool) bool {
	original := []int{mapMapErrorInput, mapMapErrorFirstCallback}
	if mapMapErrorCallbackBeforeInput(first) {
		original = []int{mapMapErrorFirstCallback, mapMapErrorInput}
	}
	merged := []int{mapMapErrorInput, mapMapErrorFirstCallback, mapMapErrorSecondCallback}
	if mapMapErrorCallbackBeforeInput(second) {
		original = append([]int{mapMapErrorSecondCallback}, original...)
		merged = []int{mapMapErrorFirstCallback, mapMapErrorSecondCallback, mapMapErrorInput}
	} else {
		original = append(original, mapMapErrorSecondCallback)
	}
	literal := map[int]bool{
		mapMapErrorFirstCallback:  isFunctionLiteral(first.Args().At(0)),
		mapMapErrorSecondCallback: isFunctionLiteral(second.Args().At(0)),
	}
	if !slices.Equal(slices.DeleteFunc(original, func(position int) bool { return literal[position] }),
		slices.DeleteFunc(merged, func(position int) bool { return literal[position] })) {
		return false
	}
	// An eager first step runs its callback on an already resolved effect when
	// it is applied. That happens before the second callback is evaluated unless
	// the second step evaluates its callback first or both steps share one
	// argument list, while the merged step evaluates both callbacks first.
	if eager && !mapMapErrorCallbackBeforeInput(second) && !mapMapErrorShareArgumentList(first, second) {
		return literal[mapMapErrorSecondCallback]
	}
	return true
}

func mapMapErrorCallbackBeforeInput(step typeparser.PipingFlowTransformation) bool {
	return step.Kind() == typeparser.TransformationKindCall || step.Kind() == typeparser.TransformationKindDataLast
}

func isFunctionLiteral(node *ast.Node) bool {
	node = ast.SkipParentheses(node)
	return node.Kind == ast.KindArrowFunction || node.Kind == ast.KindFunctionExpression
}

func mapMapErrorShareArgumentList(first, second typeparser.PipingFlowTransformation) bool {
	if !isPipingFlowArgumentKind(first.Kind()) || !isPipingFlowArgumentKind(second.Kind()) {
		return false
	}
	firstStep, secondStep := first.Callee().Parent, second.Callee().Parent
	return firstStep != nil && secondStep != nil && firstStep.Parent != nil && firstStep.Parent == secondStep.Parent
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
