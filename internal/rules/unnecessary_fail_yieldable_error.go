package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// UnnecessaryFailYieldableError suggests yielding yieldable errors directly
// instead of wrapping with Effect.fail.
var UnnecessaryFailYieldableError = rule.Rule{
	Name:            "unnecessaryFailYieldableError",
	Group:           "style",
	Description:     "Suggests yielding yieldable errors directly instead of wrapping with Effect.fail",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_yield_Asterisk_Effect_fail_passes_a_yieldable_error_value_yield_Asterisk_represents_that_value_directly_without_wrapping_it_in_Effect_fail_effect_unnecessaryFailYieldableError.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeUnnecessaryFailYieldableError(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.This_yield_Asterisk_Effect_fail_passes_a_yieldable_error_value_yield_Asterisk_represents_that_value_directly_without_wrapping_it_in_Effect_fail_effect_unnecessaryFailYieldableError, nil)
		}
		return diags
	},
}

// UnnecessaryFailYieldableErrorMatch holds the AST nodes needed by both the
// diagnostic rule and the quick-fix for the unnecessaryFailYieldableError pattern.
type UnnecessaryFailYieldableErrorMatch struct {
	SourceFile   *ast.SourceFile // The source file where this match was found
	Location     core.TextRange  // The pre-computed error range for this match
	YieldNode    *ast.Node       // The yield* expression node
	CallNode     *ast.Node       // The Effect.fail(...) call expression (fix replaces this)
	FailArgument *ast.Node       // The first argument to Effect.fail (the replacement text)
}

// AnalyzeUnnecessaryFailYieldableError finds all yield* Effect.fail(...) calls
// where the argument is a yieldable error type that can be yielded directly.
func AnalyzeUnnecessaryFailYieldableError(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []UnnecessaryFailYieldableErrorMatch {
	var matches []UnnecessaryFailYieldableErrorMatch

	var walk ast.Visitor
	walk = func(n *ast.Node) bool {
		if n == nil {
			return false
		}

		if n.Kind == ast.KindYieldExpression {
			yield := n.AsYieldExpression()
			// Must be yield* (not plain yield)
			if yield.AsteriskToken != nil && yield.Expression != nil && yield.Expression.Kind == ast.KindCallExpression {
				call := yield.Expression.AsCallExpression()
				if call.Expression != nil && call.Expression.Kind == ast.KindPropertyAccessExpression {
					if tp.IsNodeReferenceToEffectModuleApi(call.Expression, "fail") {
						if call.Arguments != nil && len(call.Arguments.Nodes) >= 1 {
							arg := call.Arguments.Nodes[0]
							argType := tp.GetTypeAtLocation(arg)
							if argType != nil && tp.IsYieldableErrorType(argType) {
								matches = append(matches, UnnecessaryFailYieldableErrorMatch{
									SourceFile:   sf,
									Location:     scanner.GetErrorRangeForNode(sf, n),
									YieldNode:    n,
									CallNode:     yield.Expression,
									FailArgument: arg,
								})
							}
						}
					}
				}
			}
		}

		n.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())
	return matches
}
