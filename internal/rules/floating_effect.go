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

// FloatingEffect detects Effect values that are created as standalone
// expression statements and are neither yielded nor assigned.
var FloatingEffect = rule.Rule{
	Name:            "floatingEffect",
	Description:     "Detects Effect values that are neither yielded nor assigned",
	DefaultSeverity: etscore.SeverityError,
	Codes:       []int32{tsdiag.Effect_must_be_yielded_or_assigned_to_a_variable_effect_floatingEffect.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic

		// Walk the entire AST using IterChildren (per 002-diagnostics-framework.md)
		var walk func(n *ast.Node)
		walk = func(n *ast.Node) {
			if n == nil {
				return
			}

			// Check if this node is a floating Effect expression statement
			if isFloatingEffectExpression(ctx.Checker, n) {
				// Use the expression's position if this is an expression statement
				// to avoid including leading trivia in the span
				expr := n
				if n.Kind == ast.KindExpressionStatement {
					exprStmt := n.AsExpressionStatement()
					if exprStmt != nil && exprStmt.Expression != nil {
						expr = exprStmt.Expression
					}
				}
				diag := ctx.NewDiagnostic(ctx.GetErrorRange(expr), tsdiag.Effect_must_be_yielded_or_assigned_to_a_variable_effect_floatingEffect, nil)
				diags = append(diags, diag)
			}

			// Recurse into all children
			for child := range n.IterChildren() {
				walk(child)
			}
		}

		walk(ctx.SourceFile.AsNode())

		return diags
	},
}

// isFloatingEffectExpression checks if a node is an expression statement
// containing an Effect type that is neither yielded nor assigned.
func isFloatingEffectExpression(c *checker.Checker, node *ast.Node) bool {
	// Must be an ExpressionStatement
	if node == nil || node.Kind != ast.KindExpressionStatement {
		return false
	}

	exprStmt := node.AsExpressionStatement()
	if exprStmt == nil || exprStmt.Expression == nil {
		return false
	}

	expr := exprStmt.Expression

	// Exclude assignment expressions
	if isAssignmentExpression(expr) {
		return false
	}

	// Get the type of the expression
	t := c.GetTypeAtLocation(expr)
	if t == nil {
		return false
	}

	// Check if it's an Effect type using the quick check first
	if !typeparser.HasEffectTypeId(c, t, expr) {
		return false
	}

	// Full validation
	return typeparser.IsEffectType(c, t, expr)
}

// isAssignmentExpression checks if an expression is an assignment (=, ??=, &&=, ||=).
func isAssignmentExpression(node *ast.Node) bool {
	if node == nil || node.Kind != ast.KindBinaryExpression {
		return false
	}

	binExpr := node.AsBinaryExpression()
	if binExpr == nil || binExpr.OperatorToken == nil {
		return false
	}

	switch binExpr.OperatorToken.Kind {
	case ast.KindEqualsToken,
		ast.KindQuestionQuestionEqualsToken,
		ast.KindAmpersandAmpersandEqualsToken,
		ast.KindBarBarEqualsToken:
		return true
	default:
		return false
	}
}
