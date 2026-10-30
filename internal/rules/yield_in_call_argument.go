package rules

import (
	"slices"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// YieldInCallArgument suggests yielding into a const before a call instead of
// passing a yield* directly as a call or new argument in Effect generators, so
// running the effect and building the call stay separate steps.
var YieldInCallArgument = rule.Rule{
	Name:            "yieldInCallArgument",
	Group:           "style",
	Description:     "Suggests yielding into a const before a call instead of passing a yield* directly as a call or new argument in Effect generators",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.This_yield_Asterisk_is_used_as_a_call_argument_yield_it_into_a_const_first_or_map_the_effect_so_running_the_effect_and_building_the_call_stay_separate_steps_effect_yieldInCallArgument.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeYieldInCallArgument(ctx.TypeParser, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				ctx.SourceFile,
				match.Location,
				tsdiag.This_yield_Asterisk_is_used_as_a_call_argument_yield_it_into_a_const_first_or_map_the_effect_so_running_the_effect_and_building_the_call_stay_separate_steps_effect_yieldInCallArgument,
				nil,
			)
		}
		return diags
	},
}

type YieldInCallArgumentMatch struct {
	Location core.TextRange
	// Yield is the yield* expression passed as an argument.
	Yield *ast.Node
	// Argument is Yield with its enclosing parentheses, the node the fix replaces.
	Argument *ast.Node
	// Statement is where the fix inserts the hoisted const; nil when hoisting
	// could change evaluation order, and the match is reported without a fix.
	Statement *ast.Node
}

func AnalyzeYieldInCallArgument(tp *typeparser.TypeParser, sf *ast.SourceFile) []YieldInCallArgumentMatch {
	var matches []YieldInCallArgumentMatch

	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}

		if node.Kind == ast.KindYieldExpression {
			yieldExpr := node.AsYieldExpression()
			if yieldExpr.AsteriskToken != nil && yieldExpr.Expression != nil {
				arg := node
				for arg.Parent != nil && arg.Parent.Kind == ast.KindParenthesizedExpression {
					arg = arg.Parent
				}
				if yieldInCallArgumentIsArgument(arg) && tp.GetEffectContextFlags(node)&typeparser.EffectContextFlagCanYieldEffect != 0 {
					matches = append(matches, YieldInCallArgumentMatch{
						Location:  scanner.GetErrorRangeForNode(sf, node),
						Yield:     node,
						Argument:  arg,
						Statement: yieldInCallArgumentHoistAnchor(arg),
					})
				}
			}
		}

		node.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())
	return matches
}

// yieldInCallArgumentIsArgument reports whether node is one of the arguments of
// its parent call or new expression.
func yieldInCallArgumentIsArgument(node *ast.Node) bool {
	parent := node.Parent
	if parent == nil || (parent.Kind != ast.KindCallExpression && parent.Kind != ast.KindNewExpression) {
		return false
	}
	return slices.Contains(parent.Arguments(), node)
}

// yieldInCallArgumentHoistAnchor returns the statement before which the yield*
// argument can be hoisted without changing what runs before the effect, or nil.
// It climbs from the argument through an allowlist of parent kinds; everything
// evaluated before the argument along the way must be side-effect free.
func yieldInCallArgumentHoistAnchor(arg *ast.Node) *ast.Node {
	child := arg
	for p := child.Parent; p != nil; child, p = p, p.Parent {
		switch p.Kind {
		case ast.KindParenthesizedExpression, ast.KindAsExpression, ast.KindSatisfiesExpression,
			ast.KindNonNullExpression, ast.KindTypeAssertionExpression,
			ast.KindTypeOfExpression, ast.KindVoidExpression, ast.KindAwaitExpression, ast.KindYieldExpression:
			continue
		case ast.KindPrefixUnaryExpression:
			operator := p.AsPrefixUnaryExpression().Operator
			if operator == ast.KindPlusPlusToken || operator == ast.KindMinusMinusToken {
				return nil
			}
		case ast.KindCallExpression, ast.KindNewExpression:
			if child == p.Expression() {
				continue
			}
			if p.Flags&ast.NodeFlagsOptionalChain != 0 || !yieldInCallArgumentIsSideEffectFree(p.Expression()) {
				return nil
			}
			for _, argument := range p.Arguments() {
				if argument == child {
					break
				}
				if !yieldInCallArgumentIsSideEffectFree(argument) {
					return nil
				}
			}
		case ast.KindPropertyAccessExpression:
			if child != p.Expression() {
				return nil
			}
		case ast.KindElementAccessExpression:
			if child == p.Expression() {
				continue
			}
			if p.Flags&ast.NodeFlagsOptionalChain != 0 || !yieldInCallArgumentIsSideEffectFree(p.Expression()) {
				return nil
			}
		case ast.KindBinaryExpression:
			binary := p.AsBinaryExpression()
			if child == binary.Left {
				continue
			}
			operator := binary.OperatorToken.Kind
			if operator == ast.KindAmpersandAmpersandToken || operator == ast.KindBarBarToken || operator == ast.KindQuestionQuestionToken ||
				ast.IsLogicalOrCoalescingAssignmentOperator(operator) || !yieldInCallArgumentIsSideEffectFree(binary.Left) {
				return nil
			}
		case ast.KindConditionalExpression:
			if child != p.AsConditionalExpression().Condition {
				return nil
			}
		case ast.KindExpressionStatement, ast.KindReturnStatement:
			return yieldInCallArgumentFinalizeAnchor(p)
		case ast.KindIfStatement:
			if child != p.AsIfStatement().Expression {
				return nil
			}
			return yieldInCallArgumentFinalizeAnchor(p)
		case ast.KindVariableDeclaration:
			list := p.Parent
			if child != p.Initializer() || list == nil || list.Kind != ast.KindVariableDeclarationList ||
				list.Flags&ast.NodeFlagsConst == 0 || len(list.AsVariableDeclarationList().Declarations.Nodes) != 1 ||
				list.Parent == nil || list.Parent.Kind != ast.KindVariableStatement {
				return nil
			}
			return yieldInCallArgumentFinalizeAnchor(list.Parent)
		default:
			return nil
		}
	}
	return nil
}

// yieldInCallArgumentFinalizeAnchor accepts a statement only when it sits in a
// statement list, so a const can be inserted right before it.
func yieldInCallArgumentFinalizeAnchor(statement *ast.Node) *ast.Node {
	if statement.Parent == nil {
		return nil
	}
	switch statement.Parent.Kind {
	case ast.KindBlock, ast.KindCaseClause, ast.KindDefaultClause:
		return statement
	}
	return nil
}

// yieldInCallArgumentIsSideEffectFree reports whether evaluating node cannot run
// code: identifiers, literals and non-optional property accesses on them.
func yieldInCallArgumentIsSideEffectFree(node *ast.Node) bool {
	node = ast.SkipOuterExpressions(node, ast.OEKParentheses|ast.OEKAssertions|ast.OEKSatisfies)
	switch node.Kind {
	case ast.KindIdentifier,
		ast.KindStringLiteral, ast.KindNumericLiteral, ast.KindBigIntLiteral,
		ast.KindNoSubstitutionTemplateLiteral, ast.KindRegularExpressionLiteral,
		ast.KindTrueKeyword, ast.KindFalseKeyword, ast.KindNullKeyword:
		return true
	case ast.KindPrefixUnaryExpression:
		prefix := node.AsPrefixUnaryExpression()
		return prefix.Operator == ast.KindMinusToken &&
			(prefix.Operand.Kind == ast.KindNumericLiteral || prefix.Operand.Kind == ast.KindBigIntLiteral)
	case ast.KindPropertyAccessExpression:
		return node.Flags&ast.NodeFlagsOptionalChain == 0 && yieldInCallArgumentIsSideEffectFree(node.Expression())
	}
	return false
}
