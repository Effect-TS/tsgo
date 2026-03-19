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

// MissingReturnYieldStar suggests "return yield*" for Effects that never succeed.
var MissingReturnYieldStar = rule.Rule{
	Name:            "missingReturnYieldStar",
	Description:     "Suggests using return yield* for Effects that never succeed",
	DefaultSeverity: etscore.SeverityError,
	Codes:           []int32{tsdiag.It_is_recommended_to_use_return_yield_Asterisk_for_Effects_that_never_succeed_to_signal_a_definitive_exit_point_for_type_narrowing_and_tooling_support_effect_missingReturnYieldStar.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeMissingReturnYieldStar(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.It_is_recommended_to_use_return_yield_Asterisk_for_Effects_that_never_succeed_to_signal_a_definitive_exit_point_for_type_narrowing_and_tooling_support_effect_missingReturnYieldStar, nil)
		}
		return diags
	},
}

// MissingReturnYieldStarMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the missingReturnYieldStar pattern.
type MissingReturnYieldStarMatch struct {
	SourceFile   *ast.SourceFile // The source file where the diagnostic should be reported
	Location     core.TextRange  // The pre-computed error range for this match
	YieldNode    *ast.Node       // The yield* expression node (for diagnostic location)
	ExprStmtNode *ast.Node       // The expression statement node (for quickfix replacement)
}

// AnalyzeMissingReturnYieldStar finds all yield* expressions inside Effect generators
// where the yielded Effect never succeeds, suggesting "return yield*" instead.
func AnalyzeMissingReturnYieldStar(c *checker.Checker, sf *ast.SourceFile) []MissingReturnYieldStarMatch {
	var matches []MissingReturnYieldStarMatch

	var walk ast.Visitor
	walk = func(n *ast.Node) bool {
		if n == nil {
			return false
		}

		if n.Kind == ast.KindExpressionStatement {
			expr := n.Expression()
			unwrapped := ast.SkipOuterExpressions(expr, ast.OEKAll)
			if unwrapped != nil && unwrapped.Kind == ast.KindYieldExpression {
				yield := unwrapped.AsYieldExpression()
				if yield != nil && yield.AsteriskToken != nil && yield.Expression != nil {
					if shouldReportMissingReturnYieldStar(c, n, unwrapped, yield.Expression) {
						matches = append(matches, MissingReturnYieldStarMatch{
							SourceFile:   sf,
							Location:     scanner.GetErrorRangeForNode(sf, unwrapped),
							YieldNode:    unwrapped,
							ExprStmtNode: n,
						})
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

func shouldReportMissingReturnYieldStar(c *checker.Checker, exprStmtNode *ast.Node, yieldNode *ast.Node, expr *ast.Expression) bool {
	if c == nil || exprStmtNode == nil || yieldNode == nil || expr == nil {
		return false
	}

	scopes := typeparser.FindEnclosingScopes(c, exprStmtNode)
	if scopes.ScopeNode == nil {
		return false
	}
	genFn := scopes.EffectGeneratorFunction()
	if genFn == nil {
		return false
	}
	if scopes.ScopeNode != genFn.AsNode() {
		return false
	}

	t := typeparser.GetTypeAtLocation(c, expr)
	if t == nil {
		return false
	}
	effect := typeparser.EffectYieldableType(c, t, expr.AsNode())
	if effect == nil || effect.A == nil {
		return false
	}
	return effect.A.Flags()&checker.TypeFlagsNever != 0
}
