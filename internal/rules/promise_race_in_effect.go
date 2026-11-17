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

var PromiseRaceInEffect = rule.Rule{
	Name:            "promiseRaceInEffect",
	Group:           "effectNative",
	Description:     "Warns when Promise.race or Promise.any races a list of promises inside Effect code, leaving the losing promises running",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.This_Effect_code_races_promises_with_Promise_0_the_losing_promises_keep_running_and_are_not_interrupted_Use_Effect_1_over_Effect_values_instead_effect_promiseRaceInEffect.Code(),
		tsdiag.This_Effect_code_races_promises_with_Promise_race_the_losing_promises_keep_running_and_are_not_interrupted_effect_promiseRaceInEffect.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzePromiseRaceInEffect(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, 0, len(matches))
		for _, match := range matches {
			if match.Replacement == "" {
				diags = append(diags, ctx.NewDiagnostic(
					match.SourceFile,
					match.Location,
					tsdiag.This_Effect_code_races_promises_with_Promise_race_the_losing_promises_keep_running_and_are_not_interrupted_effect_promiseRaceInEffect,
					nil,
				))
				continue
			}
			diags = append(diags, ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.This_Effect_code_races_promises_with_Promise_0_the_losing_promises_keep_running_and_are_not_interrupted_Use_Effect_1_over_Effect_values_instead_effect_promiseRaceInEffect,
				nil,
				match.Method,
				match.Replacement,
			))
		}
		return diags
	},
}

// PromiseRaceInEffectMatch is a global Promise.race or Promise.any step that
// races a list of promises inside Effect code.
type PromiseRaceInEffectMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	// Method is "race" or "any".
	Method string
	// Replacement is the Effect combinator named by the message: "raceAll" for
	// any, "raceAllFirst" for race on Effect v4, and "" for race on Effect v3,
	// which has no list version of raceFirst.
	Replacement string
	Flow        *typeparser.PipingFlow
	Index       int
}

// AnalyzePromiseRaceInEffect returns the Promise.race and Promise.any steps
// that run inside Effect code (an Effect constructor thunk, or an Effect.gen or
// generator Effect.fn body) over an array or a tuple with a rest element.
func AnalyzePromiseRaceInEffect(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []PromiseRaceInEffectMatch {
	var matches []PromiseRaceInEffectMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for index, transformation := range flow.Transformations().All() {
			callee := transformation.Callee()
			if transformation.Args().Len() != 0 || callee == nil ||
				tp.GetEffectContextFlags(callee)&typeparser.EffectContextFlagInEffect == 0 {
				continue
			}
			method := ""
			for _, candidate := range []string{"race", "any"} {
				if tp.IsNodeReferenceToGlobalMember(callee, "Promise", candidate) {
					method = candidate
					break
				}
			}
			if method == "" || !promiseRaceInEffectIsVariableLengthList(tp, c, flow.TransformationInputType(index)) {
				continue
			}
			replacement := "raceAll"
			if method == "race" {
				replacement = ""
				if tp.SupportedEffectVersion() == typeparser.EffectMajorV4 {
					replacement = "raceAllFirst"
				}
			}
			matches = append(matches, PromiseRaceInEffectMatch{
				SourceFile:  sf,
				Location:    scanner.GetErrorRangeForNode(sf, callee),
				Method:      method,
				Replacement: replacement,
				Flow:        flow,
				Index:       index,
			})
		}
	}
	return matches
}

func promiseRaceInEffectIsVariableLengthList(tp *typeparser.TypeParser, c *checker.Checker, t *checker.Type) bool {
	members := promiseRaceInEffectConstraintMembers(tp, c, t)
	if len(members) == 0 {
		return false
	}
	for _, member := range members {
		constrainedMembers := promiseRaceInEffectConstraintMembers(tp, c, member)
		if len(constrainedMembers) == 0 {
			return false
		}
		for _, constrained := range constrainedMembers {
			if constrained.Flags()&checker.TypeFlagsInstantiable != 0 {
				return false
			}
			if tp.ArrayType(constrained) == nil && (!checker.IsTupleType(constrained) || !promiseRaceInEffectHasRest(constrained)) {
				return false
			}
		}
	}
	return true
}

func promiseRaceInEffectConstraintMembers(tp *typeparser.TypeParser, c *checker.Checker, t *checker.Type) []*checker.Type {
	if t != nil && t.Flags()&checker.TypeFlagsInstantiable != 0 {
		t = c.GetBaseConstraintOfType(t)
	}
	return tp.UnrollUnionMembers(t)
}

func promiseRaceInEffectHasRest(tuple *checker.Type) bool {
	for _, flags := range tuple.TargetTupleType().ElementFlags() {
		if flags&checker.ElementFlagsVariable != 0 {
			return true
		}
	}
	return false
}
