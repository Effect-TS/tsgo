// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"sort"
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
)

// MissingLayerContext detects when a Layer has context requirements that are not
// handled by the expected type. This happens when assigning a Layer with requirements
// to a variable/parameter expecting a Layer with fewer or no requirements.
var MissingLayerContext = rule.Rule{
	Name:            "missingLayerContext",
	Group:           "correctness",
	Description:     "Detects Layer values with unhandled context requirements",
	DefaultSeverity: etscore.SeverityError,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Missing_0_in_the_expected_Layer_context_effect_missingLayerContext.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic

		for _, re := range ctx.Checker.GetRelationErrors(ctx.SourceFile) {
			// Parse both types as Layers
			srcLayer := typeparser.LayerType(ctx.Checker, re.Source, re.ErrorNode)
			tgtLayer := typeparser.LayerType(ctx.Checker, re.Target, re.ErrorNode)

			// Both must be Layer types
			if srcLayer == nil || tgtLayer == nil {
				continue
			}

			// Find unhandled context types by checking each source RIn member
			// against the target RIn type
			unhandled := findUnhandledLayerContexts(ctx.Checker, srcLayer.RIn, tgtLayer.RIn)
			if len(unhandled) > 0 {
				// Sort deterministically by type name (alphabetical)
				sort.Slice(unhandled, func(i, j int) bool {
					return ctx.Checker.TypeToString(unhandled[i]) < ctx.Checker.TypeToString(unhandled[j])
				})
				contextTypeStr := formatLayerContextTypes(ctx.Checker, unhandled)
				diag := ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(re.ErrorNode), tsdiag.Missing_0_in_the_expected_Layer_context_effect_missingLayerContext, nil, contextTypeStr)
				diags = append(diags, diag)
			}
		}

		return diags
	},
}

// findUnhandledLayerContexts returns the source Layer RIn types that are not assignable to the target RIn type.
func findUnhandledLayerContexts(c *checker.Checker, srcRIn, tgtRIn *checker.Type) []*checker.Type {
	// Unroll source RIn union into individual members
	srcMembers := typeparser.UnrollUnionMembers(srcRIn)

	var unhandled []*checker.Type
	for _, member := range srcMembers {
		// Check if this specific member is assignable to target
		if !checker.Checker_isTypeAssignableTo(c, member, tgtRIn) {
			unhandled = append(unhandled, member)
		}
	}
	return unhandled
}

// formatLayerContextTypes formats a slice of Layer context types as a union string (e.g., "ServiceA | ServiceB").
func formatLayerContextTypes(c *checker.Checker, types []*checker.Type) string {
	if len(types) == 0 {
		return ""
	}
	if len(types) == 1 {
		return c.TypeToString(types[0])
	}
	var result strings.Builder
	result.WriteString(c.TypeToString(types[0]))
	for i := 1; i < len(types); i++ {
		result.WriteString(" | ")
		result.WriteString(c.TypeToString(types[i]))
	}
	return result.String()
}
