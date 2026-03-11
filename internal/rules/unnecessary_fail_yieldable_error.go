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

// UnnecessaryFailYieldableError suggests yielding yieldable errors directly
// instead of wrapping with Effect.fail.
var UnnecessaryFailYieldableError = rule.Rule{
	Name:        "unnecessaryFailYieldableError",
	Description:     "Suggests yielding yieldable errors directly instead of wrapping with Effect.fail",
	DefaultSeverity: etscore.SeveritySuggestion,
	Codes:       []int32{tsdiag.This_Effect_fail_call_uses_a_yieldable_error_type_as_argument_You_can_yield_Asterisk_the_error_directly_instead_effect_unnecessaryFailYieldableError.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeUnnecessaryFailYieldableError(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.Location, tsdiag.This_Effect_fail_call_uses_a_yieldable_error_type_as_argument_You_can_yield_Asterisk_the_error_directly_instead_effect_unnecessaryFailYieldableError, nil)
		}
		return diags
	},
}

// UnnecessaryFailYieldableErrorMatch holds the AST nodes needed by both the
// diagnostic rule and the quick-fix for the unnecessaryFailYieldableError pattern.
type UnnecessaryFailYieldableErrorMatch struct {
	Location     core.TextRange  // The pre-computed error range for this match
	YieldNode    *ast.Node       // The yield* expression node
	CallNode     *ast.Node       // The Effect.fail(...) call expression (fix replaces this)
	FailArgument *ast.Node       // The first argument to Effect.fail (the replacement text)
}

// AnalyzeUnnecessaryFailYieldableError finds all yield* Effect.fail(...) calls
// where the argument is a yieldable error type that can be yielded directly.
func AnalyzeUnnecessaryFailYieldableError(c *checker.Checker, sf *ast.SourceFile) []UnnecessaryFailYieldableErrorMatch {
	var matches []UnnecessaryFailYieldableErrorMatch

	var walk func(n *ast.Node)
	walk = func(n *ast.Node) {
		if n == nil {
			return
		}

		if n.Kind == ast.KindYieldExpression {
			yield := n.AsYieldExpression()
			// Must be yield* (not plain yield)
			if yield.AsteriskToken != nil && yield.Expression != nil && yield.Expression.Kind == ast.KindCallExpression {
				call := yield.Expression.AsCallExpression()
				if call.Expression != nil && call.Expression.Kind == ast.KindPropertyAccessExpression {
					if typeparser.IsNodeReferenceToEffectModuleApi(c, call.Expression, "fail") {
						if call.Arguments != nil && len(call.Arguments.Nodes) >= 1 {
							arg := call.Arguments.Nodes[0]
							argType := c.GetTypeAtLocation(arg)
							if argType != nil && typeparser.IsYieldableErrorType(c, argType) {
								matches = append(matches, UnnecessaryFailYieldableErrorMatch{
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

		for child := range n.IterChildren() {
			walk(child)
		}
	}

	walk(sf.AsNode())
	return matches
}

