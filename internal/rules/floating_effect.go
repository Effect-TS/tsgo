// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
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
	Group:           "correctness",
	Description:     "Detects Effect values that are neither yielded nor assigned",
	DefaultSeverity: etscore.SeverityError,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.This_Effect_value_is_neither_yielded_nor_used_in_an_assignment_effect_floatingEffect.Code(),
		tsdiag.This_Effect_able_0_value_is_neither_yielded_nor_assigned_to_a_variable_effect_floatingEffect.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeFloatingEffect(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			if match.isStrict {
				diags[i] = ctx.NewDiagnostic(match.SourceFile, match.Location, tsdiag.This_Effect_value_is_neither_yielded_nor_used_in_an_assignment_effect_floatingEffect, nil)
			} else {
				typeName := ctx.Checker.TypeToString(match.exprType)
				diags[i] = ctx.NewDiagnostic(match.SourceFile, match.Location, tsdiag.This_Effect_able_0_value_is_neither_yielded_nor_assigned_to_a_variable_effect_floatingEffect, nil, typeName)
			}
		}
		return diags
	},
}

// FloatingEffectMatch holds the diagnostic location and expression needed by
// both the diagnostic rule and its quick-fix.
type FloatingEffectMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	Expression *ast.Node
	isStrict   bool
	exprType   *checker.Type
}

// AnalyzeFloatingEffect finds Effect values used as standalone expression statements.
func AnalyzeFloatingEffect(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []FloatingEffectMatch {
	var matches []FloatingEffectMatch
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}

		if result := detectFloatingEffect(tp, c, node); result != nil {
			expression := node.AsExpressionStatement().Expression
			matches = append(matches, FloatingEffectMatch{
				SourceFile: sf,
				Location:   scanner.GetErrorRangeForNode(sf, expression),
				Expression: expression,
				isStrict:   result.isStrict,
				exprType:   result.exprType,
			})
		}

		node.ForEachChild(walk)
		return false
	}
	walk(sf.AsNode())
	return matches
}

// detectFloatingEffect checks if a node is an expression statement containing an Effect type
// that is neither yielded nor assigned. Returns nil if the node should not be reported,
// or a result with type info for selecting the appropriate diagnostic message.
func detectFloatingEffect(tp *typeparser.TypeParser, _ *checker.Checker, node *ast.Node) *floatingEffectResult {
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
	t := tp.GetTypeAtLocation(expr)
	if t == nil {
		return nil
	}

	if tp.HasEffectTypeId(t) {
		// Full Effect validation.
		if !tp.IsEffectType(t) {
			return nil
		}

		// Exclude Fiber types (considered valid floating operations)
		if tp.IsFiberType(t) {
			return nil
		}

		// Exclude Effect subtypes (Exit, Option, Either, Pool, etc.)
		if tp.IsEffectSubtype(t) {
			return nil
		}
	} else if tp.StreamType(t) == nil {
		return nil
	}

	// Determine if this is strictly an Effect or an Effect-able type
	isStrict := tp.StrictIsEffectType(t)
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
