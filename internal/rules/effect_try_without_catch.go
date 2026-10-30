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

// EffectTryWithoutCatch reports short-form Effect.try / Effect.tryPromise
// calls, whose failures surface as the generic UnknownError (v4) or
// UnknownException (v3) instead of a domain-specific error.
var EffectTryWithoutCatch = rule.Rule{
	Name:            "effectTryWithoutCatch",
	Group:           "antipattern",
	Description:     "Reports Effect.try and Effect.tryPromise calls without a catch handler, whose failures surface as a generic unknown error",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_0_is_called_without_a_catch_handler_so_failures_surface_as_the_generic_1_The_object_form_try_catch_maps_the_cause_to_a_typed_domain_specific_error_effect_effectTryWithoutCatch.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeEffectTryWithoutCatch(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_0_is_called_without_a_catch_handler_so_failures_surface_as_the_generic_1_The_object_form_try_catch_maps_the_cause_to_a_typed_domain_specific_error_effect_effectTryWithoutCatch,
				nil,
				match.ApiName,
				ctx.Checker.TypeToString(match.ErrorType),
			)
		}
		return diags
	},
}

// EffectTryWithoutCatchMatch holds the nodes needed by the diagnostic and quick fix.
type EffectTryWithoutCatchMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	ApiName    string // try or tryPromise
	ErrorType  *checker.Type
	// Flow applies Effect.try or Effect.tryPromise at step Index to the thunk.
	Flow  *typeparser.PipingFlow
	Index int
	// Thunk is the input of that step, when it is a standalone node.
	Thunk *ast.Node
}

// Step returns the Effect.try or Effect.tryPromise step.
func (m EffectTryWithoutCatchMatch) Step() typeparser.PipingFlowTransformation {
	return m.Flow.Transformations().At(m.Index)
}

// AnalyzeEffectTryWithoutCatch finds piping flow steps that apply Effect.try or
// Effect.tryPromise to a function, i.e. the short form without catch, however
// the step is written: Effect.try(thunk), pipe(thunk, Effect.try) or a later
// step of a longer flow.
func AnalyzeEffectTryWithoutCatch(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []EffectTryWithoutCatchMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}
	apiName := func(transformation typeparser.PipingFlowTransformation) string {
		callee := transformation.Callee()
		if callee == nil || transformation.Args().Len() != 0 {
			return ""
		}
		switch transformation.Kind() {
		case typeparser.TransformationKindCall, typeparser.TransformationKindPipe, typeparser.TransformationKindPipeable:
		default:
			return ""
		}
		for _, name := range []string{"try", "tryPromise"} {
			if tp.IsNodeReferenceToEffectModuleApi(callee, name) {
				return name
			}
		}
		return ""
	}
	isTryStep := func(transformation typeparser.PipingFlowTransformation) bool {
		return apiName(transformation) != ""
	}

	var matches []EffectTryWithoutCatchMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for _, sequence := range flow.FindTransformationSequences(isTryStep).All() {
			index := sequence.Start()
			step := flow.Transformations().At(index)
			// The short form takes a function; the object form { try, catch }
			// has no call signature, and any cannot tell the forms apart.
			input := flow.TransformationInputType(index)
			if input == nil || input.Flags()&checker.TypeFlagsAny != 0 ||
				len(c.GetSignaturesOfType(input, checker.SignatureKindCall)) == 0 {
				continue
			}
			effect := tp.StrictEffectType(step.OutType())
			if effect == nil || effect.E() == nil {
				continue
			}
			matches = append(matches, EffectTryWithoutCatchMatch{
				SourceFile: sf,
				Location:   scanner.GetErrorRangeForNode(sf, step.Callee()),
				ApiName:    apiName(step),
				ErrorType:  effect.E(),
				Flow:       flow,
				Index:      index,
				Thunk:      flow.TransformationInputNode(index),
			})
		}
	}
	return matches
}
