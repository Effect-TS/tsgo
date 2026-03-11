// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
)

// MissingEffectContext detects when an Effect has context requirements that are not
// handled by the expected type. This happens when assigning an Effect with requirements
// to a variable/parameter expecting an Effect with fewer or no requirements.
var MissingEffectContext = rule.Rule{
	Name:            "missingEffectContext",
	Description:     "Detects Effect values with unhandled context requirements",
	DefaultSeverity: etscore.SeverityError,
	Codes:       []int32{tsdiag.Missing_context_0_in_the_expected_Effect_type_effect_missingEffectContext.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic

		for _, re := range ctx.Checker.GetRelationErrors(ctx.SourceFile) {
			// Parse both types as Effects
			srcEffect := typeparser.EffectType(ctx.Checker, re.Source, re.ErrorNode)
			tgtEffect := typeparser.EffectType(ctx.Checker, re.Target, re.ErrorNode)

			// Both must be Effect types
			if srcEffect == nil || tgtEffect == nil {
				continue
			}

			// Find unhandled context types by checking each source requirement member
			// against the target requirement type
			unhandledContexts := findUnhandledContexts(ctx.Checker, srcEffect.R, tgtEffect.R)
			if len(unhandledContexts) > 0 {
				contextTypeStr := formatContextTypes(ctx.Checker, unhandledContexts)
				diag := ctx.NewDiagnostic(ctx.GetErrorRange(re.ErrorNode), tsdiag.Missing_context_0_in_the_expected_Effect_type_effect_missingEffectContext, nil, contextTypeStr)
				diags = append(diags, diag)
			}
		}

		return diags
	},
}

// findUnhandledContexts returns the source context types that are not assignable to the target context type.
func findUnhandledContexts(c *checker.Checker, srcR, tgtR *checker.Type) []*checker.Type {
	// Unroll source context union into individual members
	srcMembers := typeparser.UnrollUnionMembers(srcR)

	var unhandled []*checker.Type
	for _, member := range srcMembers {
		// Check if this specific member is assignable to target
		if !checker.Checker_isTypeAssignableTo(c, member, tgtR) {
			unhandled = append(unhandled, member)
		}
	}
	return unhandled
}

// formatContextTypes formats a slice of context types as a union string (e.g., "EnvA | EnvB").
func formatContextTypes(c *checker.Checker, types []*checker.Type) string {
	if len(types) == 0 {
		return ""
	}
	if len(types) == 1 {
		return c.TypeToString(types[0])
	}
	result := c.TypeToString(types[0])
	for i := 1; i < len(types); i++ {
		result += " | " + c.TypeToString(types[i])
	}
	return result
}
