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

// EffectSucceedWithVoid suggests using Effect.void instead of Effect.succeed(undefined) or Effect.succeed(void 0).
var EffectSucceedWithVoid = rule.Rule{
	Name:            "effectSucceedWithVoid",
	Group:           "style",
	Description:     "Suggests using Effect.void instead of Effect.succeed(undefined) or Effect.succeed(void 0)",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Effect_void_represents_the_same_outcome_as_Effect_succeed_undefined_or_Effect_succeed_void_0_effect_effectSucceedWithVoid.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeEffectSucceedWithVoid(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.Effect_void_represents_the_same_outcome_as_Effect_succeed_undefined_or_Effect_succeed_void_0_effect_effectSucceedWithVoid, nil)
		}
		return diags
	},
}

// EffectSucceedWithVoidMatch holds the AST nodes needed by both the diagnostic rule
// and the quick-fix for the effectSucceedWithVoid pattern.
type EffectSucceedWithVoidMatch struct {
	SourceFile       *ast.SourceFile // The source file where the diagnostic should be reported
	Location         core.TextRange  // The pre-computed error range for this match
	CallNode         *ast.Node       // The Effect.succeed(...) call expression (replacement target)
	EffectModuleNode *ast.Node       // The Effect module identifier (e.g., "Effect" in Effect.succeed)
}

// isVoidExpression checks if a node is `undefined`, `void 0`, or a parenthesized version of either.
func isVoidExpression(node *ast.Node) bool {
	// Unwrap parenthesized expressions
	node = ast.SkipParentheses(node)

	// Check for `undefined`
	if node.Kind == ast.KindIdentifier && scanner.GetTextOfNode(node) == "undefined" {
		return true
	}

	// Check for `void 0`
	if node.Kind == ast.KindVoidExpression {
		operand := node.AsVoidExpression().Expression
		if operand.Kind == ast.KindNumericLiteral && scanner.GetTextOfNode(operand) == "0" {
			return true
		}
	}

	return false
}

// AnalyzeEffectSucceedWithVoid finds all Effect.succeed(undefined) and Effect.succeed(void 0) calls
// that can be replaced with Effect.void.
func AnalyzeEffectSucceedWithVoid(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []EffectSucceedWithVoidMatch {
	var matches []EffectSucceedWithVoidMatch

	var walk ast.Visitor
	walk = func(n *ast.Node) bool {
		if n == nil {
			return false
		}

		if n.Kind == ast.KindCallExpression {
			call := n.AsCallExpression()
			if call.Expression != nil && call.Expression.Kind == ast.KindPropertyAccessExpression {
				if tp.IsNodeReferenceToEffectModuleApi(call.Expression, "succeed") {
					if call.Arguments != nil && len(call.Arguments.Nodes) >= 1 {
						arg := call.Arguments.Nodes[0]
						if isVoidExpression(arg) {
							propAccess := call.Expression.AsPropertyAccessExpression()
							matches = append(matches, EffectSucceedWithVoidMatch{
								SourceFile:       sf,
								Location:         scanner.GetErrorRangeForNode(sf, n),
								CallNode:         n,
								EffectModuleNode: propAccess.Expression,
							})
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
