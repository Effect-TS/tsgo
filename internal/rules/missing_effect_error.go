// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// MissingEffectError detects when an Effect has error types that are not
// handled by the expected type. This happens when assigning an Effect with errors
// to a variable/parameter expecting an Effect with fewer or no errors.
var MissingEffectError = rule.Rule{
	Name:            "missingEffectError",
	Group:           "correctness",
	Description:     "Detects Effect values with unhandled error types",
	DefaultSeverity: etscore.SeverityError,
	SupportedEffect: []string{"v3", "v4"},
	Codes:       []int32{tsdiag.Missing_errors_0_in_the_expected_Effect_type_effect_missingEffectError.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeMissingEffectError(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.Missing_errors_0_in_the_expected_Effect_type_effect_missingEffectError, nil, m.ErrorTypeStr)
		}
		return diags
	},
}

// MissingEffectErrorMatch holds the analysis results needed by both the
// diagnostic rule and the quick-fixes for the missingEffectError pattern.
type MissingEffectErrorMatch struct {
	SourceFile        *ast.SourceFile // The source file where the diagnostic should be reported
	Location          core.TextRange   // The diagnostic error range
	ErrorNode         *ast.Node        // The AST node where the type error occurs
	UnhandledErrors   []*checker.Type  // The individual error types not handled by the target
	ExpectedErrorType *checker.Type    // The target Effect's error type (.E)
	ErrorTypeStr      string           // The formatted union string of unhandled errors
}

// AnalyzeMissingEffectError finds all relation errors where an Effect has error
// types that are not handled by the expected type.
func AnalyzeMissingEffectError(c *checker.Checker, sf *ast.SourceFile) []MissingEffectErrorMatch {
	var matches []MissingEffectErrorMatch

	for _, re := range c.GetRelationErrors(sf) {
		// Parse both types as Effects
		srcEffect := typeparser.EffectType(c, re.Source, re.ErrorNode)
		tgtEffect := typeparser.EffectType(c, re.Target, re.ErrorNode)

		// Both must be Effect types
		if srcEffect == nil || tgtEffect == nil {
			continue
		}

		// If source has no errors, nothing to report
		if srcEffect.E.Flags()&checker.TypeFlagsNever != 0 {
			continue
		}

		// Find unhandled error types
		unhandledErrors := findUnhandledErrors(c, srcEffect.E, tgtEffect.E)
		if len(unhandledErrors) > 0 {
			matches = append(matches, MissingEffectErrorMatch{
				SourceFile:        sf,
				Location:          scanner.GetErrorRangeForNode(sf, re.ErrorNode),
				ErrorNode:         re.ErrorNode,
				UnhandledErrors:   unhandledErrors,
				ExpectedErrorType: tgtEffect.E,
				ErrorTypeStr:      formatErrorTypes(c, unhandledErrors),
			})
		}
	}

	return matches
}

// findUnhandledErrors returns the source error types that are not assignable to the target error type.
func findUnhandledErrors(c *checker.Checker, srcE, tgtE *checker.Type) []*checker.Type {
	// Unroll source error union into individual members
	srcMembers := typeparser.UnrollUnionMembers(srcE)

	var unhandled []*checker.Type
	for _, member := range srcMembers {
		// Check if this specific member is assignable to target
		if !checker.Checker_isTypeAssignableTo(c, member, tgtE) {
			unhandled = append(unhandled, member)
		}
	}
	return unhandled
}

// formatErrorTypes formats a slice of error types as a union string (e.g., "ErrorA | ErrorB").
func formatErrorTypes(c *checker.Checker, types []*checker.Type) string {
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
