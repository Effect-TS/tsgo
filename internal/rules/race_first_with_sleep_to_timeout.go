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

// RaceFirstWithSleepToTimeout suggests timeoutOrElse when one side of an
// Effect first-completion race is a hand-rolled timer arm.
var RaceFirstWithSleepToTimeout = rule.Rule{
	Name:            "raceFirstWithSleepToTimeout",
	Group:           "style",
	Description:     "Suggests Effect.timeoutOrElse when Effect.raceFirst has exactly one sleep- or delay-based timer arm",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.This_Effect_first_completion_race_has_exactly_one_sleep_or_delay_based_timer_arm_Effect_timeoutOrElse_expresses_the_timeout_and_fallback_directly_effect_raceFirstWithSleepToTimeout.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeRaceFirstWithSleepToTimeout(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.This_Effect_first_completion_race_has_exactly_one_sleep_or_delay_based_timer_arm_Effect_timeoutOrElse_expresses_the_timeout_and_fallback_directly_effect_raceFirstWithSleepToTimeout,
				nil,
			)
		}
		return diagnostics
	},
}

type RaceFirstWithSleepToTimeoutMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
}

// AnalyzeRaceFirstWithSleepToTimeout finds Effect.raceFirst calls, and
// two-element literal Effect.raceAllFirst calls, with exactly one timer arm.
func AnalyzeRaceFirstWithSleepToTimeout(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []RaceFirstWithSleepToTimeoutMatch {
	if tp == nil || c == nil || sf == nil || tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}

	var matches []RaceFirstWithSleepToTimeoutMatch
	seen := make(map[*ast.Node]struct{})
	for _, flow := range tp.PipingFlows(sf, true) {
		for i := range flow.Transformations {
			transformation := &flow.Transformations[i]
			if transformation.Callee == nil || transformation.Node == nil {
				continue
			}

			var left, right *typeparser.PartialPipingFlow
			switch {
			case tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "raceFirst"):
				// An onWinner option has observable behavior that timeoutOrElse
				// cannot carry over, so only match the two-arm form.
				if len(transformation.Args) != 1 {
					continue
				}
				left = flow.CopyPrefix(i)
				if rightFlow := tp.LongestPipingFlowAt(transformation.Args[0], true); rightFlow != nil {
					right = &rightFlow.PartialPipingFlow
				}

			case tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "raceAllFirst"):
				call := transformation.Node.AsCallExpression()
				if call == nil || call.Arguments == nil || len(call.Arguments.Nodes) != 1 {
					continue
				}
				array := ast.SkipParentheses(call.Arguments.Nodes[0])
				if array == nil || array.Kind != ast.KindArrayLiteralExpression || len(array.AsArrayLiteralExpression().Elements.Nodes) != 2 {
					continue
				}
				if leftFlow := tp.LongestPipingFlowAt(array.AsArrayLiteralExpression().Elements.Nodes[0], true); leftFlow != nil {
					left = &leftFlow.PartialPipingFlow
				}
				if rightFlow := tp.LongestPipingFlowAt(array.AsArrayLiteralExpression().Elements.Nodes[1], true); rightFlow != nil {
					right = &rightFlow.PartialPipingFlow
				}

			default:
				continue
			}

			if isRaceFirstTimerFlow(tp, left) == isRaceFirstTimerFlow(tp, right) {
				continue
			}
			if _, duplicate := seen[transformation.Node]; duplicate {
				continue
			}
			seen[transformation.Node] = struct{}{}
			matches = append(matches, RaceFirstWithSleepToTimeoutMatch{
				SourceFile: sf,
				Location:   scanner.GetErrorRangeForNode(sf, transformation.Callee),
			})
		}
	}
	return matches
}

func isRaceFirstTimerFlow(tp *typeparser.TypeParser, flow *typeparser.PartialPipingFlow) bool {
	if tp == nil || flow == nil || len(flow.Transformations) == 0 {
		return false
	}

	for i := range flow.Transformations {
		transformation := &flow.Transformations[i]
		if transformation.Callee == nil {
			continue
		}
		if tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "sleep") {
			return i == 0 && timerTrailingTransformations(tp, flow.Transformations[i+1:])
		}
		if tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "delay") {
			return timerTrailingTransformations(tp, flow.Transformations[i+1:])
		}
	}
	return false
}

func timerTrailingTransformations(tp *typeparser.TypeParser, transformations []typeparser.PipingFlowTransformation) bool {
	for i := range transformations {
		callee := transformations[i].Callee
		if callee == nil ||
			!tp.IsNodeReferenceToEffectModuleApi(callee, "flatMap") &&
				!tp.IsNodeReferenceToEffectModuleApi(callee, "andThen") &&
				!tp.IsNodeReferenceToEffectModuleApi(callee, "as") &&
				!tp.IsNodeReferenceToEffectModuleApi(callee, "map") {
			return false
		}
	}
	return true
}
