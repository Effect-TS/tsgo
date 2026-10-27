package fixables

import (
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// removePipingFlowTransformation removes one step from its piping flow and
// leaves the step's input in place. A pipe argument is deleted, a pipe call left
// without steps collapses to its subject, and a data-first, data-last or unary
// application is replaced by its subject, parenthesized when the subject binds
// less tightly than the call it replaces. It edits nothing and returns false
// when the step's shape is not recognized, or when the subject would newly start
// an expression statement with a token that continues the previous statement.
func removePipingFlowTransformation(tracker *rewriter.Tracker, sf *ast.SourceFile, transformation *typeparser.PipingFlowTransformation) bool {
	if tracker == nil || sf == nil || transformation == nil || transformation.Callee() == nil {
		return false
	}

	switch transformation.Kind() {
	case typeparser.TransformationKindDataFirst, typeparser.TransformationKindDataLast, typeparser.TransformationKindCall:
		callNode, call := directCallForCallee(transformation.Callee())
		if transformation.Kind() == typeparser.TransformationKindCall && transformation.Args().Len() > 0 {
			// A curried step `factory(args)(subject)` is applied by the outer call.
			callNode, call = directCallForCallee(callNode)
		}
		if call == nil || call.Arguments == nil {
			return false
		}
		arguments := call.Arguments.Nodes
		var subject *ast.Node
		switch transformation.Kind() {
		case typeparser.TransformationKindDataFirst:
			if len(arguments) == transformation.Args().Len()+1 {
				subject = arguments[0]
			}
		case typeparser.TransformationKindDataLast:
			if len(arguments) == transformation.Args().Len()+1 {
				subject = arguments[len(arguments)-1]
			}
		default:
			if len(arguments) == 1 {
				subject = arguments[0]
			}
		}
		if subject == nil {
			return false
		}
		replacement := subjectInCallPosition(tracker, sf, callNode, subject)
		if replacement == nil {
			return false
		}
		tracker.ReplaceNode(sf, callNode, replacement, nil)
		return true

	case typeparser.TransformationKindPipe, typeparser.TransformationKindPipeable,
		typeparser.TransformationKindEffectFn, typeparser.TransformationKindEffectFnUntraced:
		callNode, call, argumentIndex := containingCallArgument(transformation.Callee())
		if call == nil || call.Arguments == nil || argumentIndex < 0 {
			return false
		}
		arguments := call.Arguments.Nodes
		// Effect.fn pipelines always keep their body argument, so they never collapse.
		minimumArguments := 0
		switch transformation.Kind() {
		case typeparser.TransformationKindPipeable:
			minimumArguments = 1
		case typeparser.TransformationKindPipe:
			minimumArguments = 2 // Function.pipe also contains its subject.
		}
		if len(arguments) == minimumArguments {
			var subject *ast.Node
			if transformation.Kind() == typeparser.TransformationKindPipe {
				subject = arguments[0]
			} else if call.Expression != nil && call.Expression.Kind == ast.KindPropertyAccessExpression {
				subject = call.Expression.AsPropertyAccessExpression().Expression
			}
			if subject == nil {
				return false
			}
			replacement := subjectInCallPosition(tracker, sf, callNode, subject)
			if replacement == nil {
				return false
			}
			tracker.ReplaceNode(sf, callNode, replacement, nil)
			return true
		}

		if argumentIndex < len(arguments)-1 {
			start := scanner.GetTokenPosOfNode(arguments[argumentIndex], sf, false)
			end := scanner.GetTokenPosOfNode(arguments[argumentIndex+1], sf, false)
			tracker.DeleteRange(sf, core.NewTextRange(start, end))
			return true
		}
		if argumentIndex > 0 {
			tracker.DeleteRange(sf, core.NewTextRange(arguments[argumentIndex-1].End(), arguments[argumentIndex].End()))
			return true
		}
	}

	return false
}

// subjectInCallPosition clones a removed step's subject for the position of the
// call it replaces. A call binds as tightly as any expression, so a subject that
// binds less tightly (a conditional, binary, assignment, `as` or arrow
// expression, an argument-less `new X`, or an optional chain that would extend
// over a following member access) is parenthesized unless the position accepts
// any expression. Object, function and class expressions are always
// parenthesized because they could start an expression statement.
//
// It returns nil when the replacement would newly begin an expression statement
// with a token that continues the previous statement when that one has no
// semicolon, for example `(cond ? a : b).pipe(...)` on the line after
// `const x = y`. The fix is then withheld instead of adding a semicolon,
// whether or not the previous statement already ends with one.
func subjectInCallPosition(tracker *rewriter.Tracker, sf *ast.SourceFile, position *ast.Node, subject *ast.Node) *ast.Node {
	parenthesize := subjectNeedsParentheses(position, subject)
	leading := ast.KindOpenParenToken
	if !parenthesize {
		leading = leadingTokenKind(subject)
	}
	if continuesPreviousStatement(leading) && leading != leadingTokenKind(position) &&
		startsExpressionStatement(sf, position) {
		return nil
	}
	clone := tracker.DeepCloneNode(subject)
	if parenthesize {
		return tracker.NewParenthesizedExpression(clone)
	}
	return clone
}

// startsExpressionStatement reports whether node begins the expression
// statement that contains it.
func startsExpressionStatement(sf *ast.SourceFile, node *ast.Node) bool {
	start := scanner.GetTokenPosOfNode(node, sf, false)
	for parent := node.Parent; parent != nil; parent = parent.Parent {
		if scanner.GetTokenPosOfNode(parent, sf, false) != start {
			return false
		}
		if parent.Kind == ast.KindExpressionStatement {
			return true
		}
	}
	return false
}

// leadingTokenKind returns the kind of an expression's first token, as far as
// continuesPreviousStatement needs to tell tokens apart.
func leadingTokenKind(node *ast.Node) ast.Kind {
	for node != nil {
		switch node.Kind {
		case ast.KindParenthesizedExpression:
			return ast.KindOpenParenToken
		case ast.KindArrayLiteralExpression:
			return ast.KindOpenBracketToken
		case ast.KindNoSubstitutionTemplateLiteral, ast.KindTemplateExpression:
			return ast.KindNoSubstitutionTemplateLiteral
		case ast.KindRegularExpressionLiteral:
			return ast.KindSlashToken
		case ast.KindTypeAssertionExpression:
			return ast.KindLessThanToken
		case ast.KindPrefixUnaryExpression:
			return node.AsPrefixUnaryExpression().Operator
		case ast.KindPropertyAccessExpression:
			node = node.AsPropertyAccessExpression().Expression
		case ast.KindElementAccessExpression:
			node = node.AsElementAccessExpression().Expression
		case ast.KindCallExpression:
			node = node.AsCallExpression().Expression
		case ast.KindTaggedTemplateExpression:
			node = node.AsTaggedTemplateExpression().Tag
		case ast.KindNonNullExpression:
			node = node.AsNonNullExpression().Expression
		case ast.KindBinaryExpression:
			node = node.AsBinaryExpression().Left
		case ast.KindConditionalExpression:
			node = node.AsConditionalExpression().Condition
		case ast.KindAsExpression:
			node = node.AsAsExpression().Expression
		case ast.KindSatisfiesExpression:
			node = node.AsSatisfiesExpression().Expression
		case ast.KindPostfixUnaryExpression:
			node = node.AsPostfixUnaryExpression().Operand
		default:
			return node.Kind
		}
	}
	return ast.KindUnknown
}

// continuesPreviousStatement reports whether a statement beginning with token
// is parsed as a continuation of a previous statement that has no semicolon.
func continuesPreviousStatement(token ast.Kind) bool {
	switch token {
	case ast.KindOpenParenToken, ast.KindOpenBracketToken, ast.KindNoSubstitutionTemplateLiteral,
		ast.KindPlusToken, ast.KindMinusToken, ast.KindSlashToken, ast.KindLessThanToken:
		return true
	}
	return false
}

func subjectNeedsParentheses(position *ast.Node, subject *ast.Node) bool {
	switch subject.Kind {
	case ast.KindObjectLiteralExpression, ast.KindFunctionExpression, ast.KindClassExpression:
		return true
	}
	if positionAcceptsAnyExpression(position) {
		return false
	}
	switch {
	case subject.Kind == ast.KindNewExpression:
		return subject.AsNewExpression().Arguments == nil
	case subject.Flags&ast.NodeFlagsOptionalChain != 0:
		return true
	}
	return !ast.IsLeftHandSideExpression(subject)
}

// positionAcceptsAnyExpression reports whether an expression of assignment
// precedence can stand at position without parentheses.
func positionAcceptsAnyExpression(position *ast.Node) bool {
	parent := position.Parent
	if parent == nil {
		return false
	}
	switch parent.Kind {
	case ast.KindCallExpression:
		return parent.AsCallExpression().Expression != position
	case ast.KindNewExpression:
		return parent.AsNewExpression().Expression != position
	case ast.KindParenthesizedExpression, ast.KindArrayLiteralExpression, ast.KindSpreadElement,
		ast.KindTemplateSpan, ast.KindReturnStatement:
		return true
	case ast.KindVariableDeclaration:
		return parent.AsVariableDeclaration().Initializer == position
	case ast.KindPropertyAssignment:
		return parent.AsPropertyAssignment().Initializer == position
	}
	return false
}

func directCallForCallee(callee *ast.Node) (*ast.Node, *ast.CallExpression) {
	if callee == nil || callee.Parent == nil || callee.Parent.Kind != ast.KindCallExpression {
		return nil, nil
	}
	call := callee.Parent.AsCallExpression()
	if call == nil || call.Expression != callee {
		return nil, nil
	}
	return callee.Parent, call
}

func containingCallArgument(node *ast.Node) (*ast.Node, *ast.CallExpression, int) {
	for child := node; child != nil && child.Parent != nil; child = child.Parent {
		parent := child.Parent
		if parent.Kind != ast.KindCallExpression {
			continue
		}
		call := parent.AsCallExpression()
		if call == nil || call.Arguments == nil {
			continue
		}
		for index, argument := range call.Arguments.Nodes {
			if argument == child {
				return parent, call, index
			}
		}
	}
	return nil, nil, -1
}
