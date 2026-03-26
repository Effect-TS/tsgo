// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
)

// GlobalErrorInEffectFailure detects when `new Error(...)` expressions appear inside an Effect
// context where the failure channel (E type parameter) contains the global Error type.
var GlobalErrorInEffectFailure = rule.Rule{
	Name:            "globalErrorInEffectFailure",
	Group:           "antipattern",
	Description:     "Warns when the global Error type is used in an Effect failure channel",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Global_Error_loses_type_safety_as_untagged_errors_merge_together_in_the_Effect_failure_channel_Consider_using_a_tagged_error_and_optionally_wrapping_the_original_in_a_cause_property_effect_globalErrorInEffectFailure.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic

		var walk ast.Visitor
		walk = func(n *ast.Node) bool {
			if n == nil {
				return false
			}

			if n.Kind == ast.KindNewExpression {
				if diag := checkGlobalErrorInEffectFailure(ctx, n); diag != nil {
					diags = append(diags, diag)
				}
			}

			n.ForEachChild(walk)
			return false
		}

		walk(ctx.SourceFile.AsNode())

		return diags
	},
}

// checkGlobalErrorInEffectFailure checks a single new expression for the global-error-in-failure pattern.
func checkGlobalErrorInEffectFailure(ctx *rule.Context, node *ast.Node) *ast.Diagnostic {
	// Get the type of the new expression
	newExprType := typeparser.GetTypeAtLocation(ctx.Checker, node)
	if newExprType == nil {
		return nil
	}

	// Skip if not a global Error type
	if !typeparser.IsGlobalErrorType(ctx.Checker, newExprType) {
		return nil
	}

	// Walk up the parent chain to find an enclosing Effect type
	ancestor := ast.FindAncestorOrQuit(node.Parent, func(current *ast.Node) ast.FindAncestorResult {
		currentType := typeparser.GetTypeAtLocation(ctx.Checker, current)
		if currentType == nil {
			return ast.FindAncestorFalse
		}

		effectType := typeparser.EffectType(ctx.Checker, currentType, current)
		if effectType == nil {
			return ast.FindAncestorFalse
		}

		// Found an Effect type — check if the failure channel contains global Error
		for _, member := range typeparser.UnrollUnionMembers(effectType.E) {
			if typeparser.IsGlobalErrorType(ctx.Checker, member) {
				return ast.FindAncestorTrue
			}
		}

		// Effect type found but failure channel doesn't contain global Error — stop searching
		return ast.FindAncestorQuit
	})

	if ancestor != nil {
		return ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(node), tsdiag.Global_Error_loses_type_safety_as_untagged_errors_merge_together_in_the_Effect_failure_channel_Consider_using_a_tagged_error_and_optionally_wrapping_the_original_in_a_cause_property_effect_globalErrorInEffectFailure, nil)
	}

	return nil
}
