// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
)

// catchFunctions lists the Effect module catch functions to check (V3 and V4).
var catchFunctions = []string{"catch", "catchAll", "catchIf", "catchSome", "catchTag", "catchTags"}

// CatchUnfailableEffect detects when error-handling functions are applied
// to an Effect whose error type is never, meaning the handler will never trigger.
var CatchUnfailableEffect = rule.Rule{
	Name:            "catchUnfailableEffect",
	Group:           "antipattern",
	Description:     "Warns when using error handling on Effects that never fail",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Looks_like_the_previous_effect_never_fails_so_probably_this_error_handling_will_never_be_triggered_effect_catchUnfailableEffect.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic

		flows := typeparser.PipingFlows(ctx.Checker, ctx.SourceFile, true)
		for _, flow := range flows {
			for i, transformation := range flow.Transformations {
				if !isCatchCallee(ctx.Checker, transformation.Callee) {
					continue
				}

				// Determine the input type for this transformation
				var inputType *checker.Type
				if i == 0 {
					inputType = flow.Subject.OutType
				} else {
					inputType = flow.Transformations[i-1].OutType
				}
				if inputType == nil {
					continue
				}

				// Parse input type as an Effect
				effect := typeparser.EffectType(ctx.Checker, inputType, transformation.Callee)
				if effect == nil {
					continue
				}

				// Check if E is never
				if effect.E == nil || effect.E.Flags()&checker.TypeFlagsNever == 0 {
					continue
				}

				diags = append(diags, ctx.NewDiagnostic(ast.GetSourceFileOfNode(transformation.Callee), ctx.GetErrorRange(transformation.Callee), tsdiag.Looks_like_the_previous_effect_never_fails_so_probably_this_error_handling_will_never_be_triggered_effect_catchUnfailableEffect, nil))
			}
		}

		return diags
	},
}

// isCatchCallee checks if a node references one of the Effect module catch functions.
func isCatchCallee(c *checker.Checker, node *ast.Node) bool {
	for _, name := range catchFunctions {
		if typeparser.IsNodeReferenceToEffectModuleApi(c, node, name) {
			return true
		}
	}
	return false
}
