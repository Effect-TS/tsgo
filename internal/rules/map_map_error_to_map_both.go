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
	DefaultSeverity: etscore.SeverityOff,
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
				// The fix removes one callee and replaces the other, so both must be
				// entity names whose evaluation runs no code.
				Fixable: ast.IsEntityNameExpression(first.Callee()) && ast.IsEntityNameExpression(second.Callee()) &&
					!hasExplicitTypeArguments(&second) && !mapMapErrorPathHasTypeArguments(first, second) &&
					mapMapErrorKeepsEvaluationOrder(first, second, a.eager),
			})
		}
	}
	return matches
}

// mapMapErrorPathHasTypeArguments reports explicit type arguments on any call
// from the first step up to the call that applies the second step: the first
// step itself, its curried application or containing pipe, empty pipe calls in
// between, and the second step's application or containing pipe. Removing the
// first step changes the input type of every one of them, so their type
// arguments would no longer match. It also reports true when the second step's
// application is not an ancestor of the first step.
func mapMapErrorPathHasTypeArguments(first, second typeparser.PipingFlowTransformation) bool {
	application := mapMapErrorStepApplication(second)
	for node := first.Callee(); node != nil; node = node.Parent {
		if node.Kind == ast.KindCallExpression {
			typeArguments := node.AsCallExpression().TypeArguments
			if typeArguments != nil && len(typeArguments.Nodes) > 0 {
				return true
			}
		}
		if node == application {
			return false
		}
	}
	return true
}

// mapMapErrorStepApplication returns the call that applies a step to its input.
func mapMapErrorStepApplication(step typeparser.PipingFlowTransformation) *ast.Node {
	call := step.Callee().Parent
	if call == nil || (step.Kind() != typeparser.TransformationKindCall && !isPipingFlowArgumentKind(step.Kind())) {
		return call
	}
	// A curried step is applied by the call around its factory, and a pipe
	// argument by the pipe or Effect.fn call that holds it.
	application := call.Parent
	for application != nil && application.Kind == ast.KindParenthesizedExpression {
		application = application.Parent
	}
	return application
}

// mapMapErrorKeepsEvaluationOrder reports whether the fix keeps the observable
// order of evaluation. Steps in one argument list keep it: their callbacks are
// evaluated before any of them runs. Otherwise the fix moves the first callback
// into the second step and the first step's work into the merged step. Only a
// function literal can move across other code, because evaluating it runs none.
func mapMapErrorKeepsEvaluationOrder(first, second typeparser.PipingFlowTransformation, eager bool) bool {
	if mapMapErrorShareArgumentList(first, second) {
		return true
	}
	if !isFunctionLiteral(first.Args().At(0)) && !mapMapErrorFirstCallbackMovesFreely(first, second) {
		return false
	}
	// A non-eager step only builds an effect, so moving its work is unobservable.
	return !eager || mapMapErrorFirstWorkMovesFreely(second)
}

// mapMapErrorFirstCallbackMovesFreely reports whether nothing observable runs
// between the first callback's evaluation and the second step's callback
// position, apart from applying the first step. That holds when the first step
// evaluates its input before its callback and is the first step of its pipe,
// and the second step evaluates its input before its callback. A curried step
// evaluates its callback before its input, and earlier steps of the first pipe
// run before the second pipe's arguments are evaluated.
func mapMapErrorFirstCallbackMovesFreely(first, second typeparser.PipingFlowTransformation) bool {
	if second.Kind() != typeparser.TransformationKindDataFirst && !isPipingFlowArgumentKind(second.Kind()) {
		return false
	}
	if first.Kind() == typeparser.TransformationKindDataFirst {
		return true
	}
	if !isPipingFlowArgumentKind(first.Kind()) {
		return false
	}
	index, _ := mapMapErrorArgumentIndex(first)
	if first.Kind() == typeparser.TransformationKindPipeable {
		return index == 0
	}
	// Function.pipe and Effect.fn hold their subject or body before the steps.
	return index == 1
}

// mapMapErrorFirstWorkMovesFreely reports whether an eager first step can run
// in the merged step without crossing observable code. When the first step is
// applied before the second callback is evaluated, the merged step runs that
// work after the second callback and, for a pipe argument, after the arguments
// that follow it.
func mapMapErrorFirstWorkMovesFreely(second typeparser.PipingFlowTransformation) bool {
	switch {
	case second.Kind() == typeparser.TransformationKindCall || second.Kind() == typeparser.TransformationKindDataLast:
		return true
	case second.Kind() == typeparser.TransformationKindDataFirst:
		return isFunctionLiteral(second.Args().At(0))
	case isPipingFlowArgumentKind(second.Kind()):
		index, count := mapMapErrorArgumentIndex(second)
		return isFunctionLiteral(second.Args().At(0)) && index == count-1
	}
	return false
}

// mapMapErrorArgumentIndex returns the position of a pipe argument step in its
// call's argument list and the list's length, or -1 when it is not found.
func mapMapErrorArgumentIndex(step typeparser.PipingFlowTransformation) (int, int) {
	stepCall := step.Callee().Parent
	if stepCall == nil || stepCall.Parent == nil || stepCall.Parent.Kind != ast.KindCallExpression {
		return -1, 0
	}
	arguments := stepCall.Parent.AsCallExpression().Arguments
	if arguments == nil {
		return -1, 0
	}
	return slices.Index(arguments.Nodes, stepCall), len(arguments.Nodes)
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
