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

// floatingEffectResult holds information about a detected floating Effect expression.
type floatingEffectResult struct {
	// isStrict is true when the type's symbol name is exactly "Effect"
	isStrict bool
	// exprType is the checker type of the floating expression
	exprType *checker.Type
}

// FloatingEffect detects Effect values that are created as standalone
// expression statements and are neither yielded nor assigned.
var FloatingEffect = rule.Rule{
	Name:            "floatingEffect",
	Description:     "Detects Effect values that are neither yielded nor assigned",
	DefaultSeverity: etscore.SeverityError,
	Codes: []int32{
		tsdiag.Effect_must_be_yielded_or_assigned_to_a_variable_effect_floatingEffect.Code(),
		tsdiag.Effect_able_0_must_be_yielded_or_assigned_to_a_variable_effect_floatingEffect.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic

		// Walk the entire AST using ForEachChild
		var walk ast.Visitor
		walk = func(n *ast.Node) bool {
			if n == nil {
				return false
			}

			// Check if this node is a floating Effect expression statement
			if result := detectFloatingEffect(ctx.Checker, n); result != nil {
				// Use the expression's position if this is an expression statement
				// to avoid including leading trivia in the span
				expr := n
				if n.Kind == ast.KindExpressionStatement {
					exprStmt := n.AsExpressionStatement()
					if exprStmt != nil && exprStmt.Expression != nil {
						expr = exprStmt.Expression
					}
				}

				var diag *ast.Diagnostic
				if result.isStrict {
					diag = ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(expr), tsdiag.Effect_must_be_yielded_or_assigned_to_a_variable_effect_floatingEffect, nil)
				} else {
					typeName := ctx.Checker.TypeToString(result.exprType)
					diag = ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(expr), tsdiag.Effect_able_0_must_be_yielded_or_assigned_to_a_variable_effect_floatingEffect, nil, typeName)
				}
				diags = append(diags, diag)
			}

			// Recurse into all children
			n.ForEachChild(walk)
			return false
		}

		walk(ctx.SourceFile.AsNode())

		return diags
	},
}

// detectFloatingEffect checks if a node is an expression statement containing an Effect type
// that is neither yielded nor assigned. Returns nil if the node should not be reported,
// or a result with type info for selecting the appropriate diagnostic message.
func detectFloatingEffect(c *checker.Checker, node *ast.Node) *floatingEffectResult {
	// Must be an ExpressionStatement
	if node == nil || node.Kind != ast.KindExpressionStatement {
		return nil
	}

	exprStmt := node.AsExpressionStatement()
	if exprStmt == nil || exprStmt.Expression == nil {
		return nil
	}

	expr := exprStmt.Expression

	// Exclude assignment expressions
	if isAssignmentExpression(expr) {
		return nil
	}

	// Get the type of the expression
	t := typeparser.GetTypeAtLocation(c, expr)
	if t == nil {
		return nil
	}

	// Check if it's an Effect type using the quick check first
	if !typeparser.HasEffectTypeId(c, t, expr) {
		return nil
	}

	// Full validation
	if !typeparser.IsEffectType(c, t, expr) {
		return nil
	}

	// Exclude Fiber types (considered valid floating operations)
	if typeparser.IsFiberType(c, t, expr) {
		return nil
	}

	// Exclude Effect subtypes (Exit, Option, Either, Pool, etc.)
	if typeparser.IsEffectSubtype(c, t, expr) {
		return nil
	}

	// Determine if this is strictly an Effect or an Effect-able type
	isStrict := typeparser.StrictIsEffectType(c, t, expr)
	return &floatingEffectResult{
		isStrict: isStrict,
		exprType: t,
	}
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
