package fixables

import (
	"strings"
	"unicode"

	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
	"github.com/microsoft/TypeScript/tsc/shim/printer"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var YieldInCallArgumentFix = fixable.Fixable{
	Name:        "yieldInCallArgument",
	Description: "Hoist yield* into a const",
	ErrorCodes: []int32{
		tsdiag.This_yield_Asterisk_is_used_as_a_call_argument_yield_it_into_a_const_first_or_map_the_effect_so_running_the_effect_and_building_the_call_stay_separate_steps_effect_yieldInCallArgument.Code(),
	},
	FixIDs: []string{"yieldInCallArgument_hoist"},
	Run:    runYieldInCallArgumentFix,
}

func runYieldInCallArgumentFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile
	match := yieldInCallArgumentMatchAt(rules.AnalyzeYieldInCallArgument(ctx.TypeParser, sf), ctx.Span)
	if match == nil {
		return nil
	}
	// Hoisting could change what runs before the effect; the diagnostic stays
	// without a fix.
	anchor := yieldInCallArgumentHoistAnchor(match.Argument)
	if anchor == nil {
		return nil
	}

	if action := ctx.NewFixAction(fixable.FixAction{
		Description: "Hoist yield* into a const",
		Run: func(tracker *rewriter.Tracker) {
			// The printer turns the name into the first of name, name_1, ... that
			// no identifier in the file uses, as TypeScript's extract refactors do.
			// Each use gets its own node: every edit is printed by a fresh printer,
			// which resolves the same name, and a node shared between the edits
			// would carry the positions of the first print into the second, where
			// the printer emits the source comments found at those positions.
			base := yieldInCallArgumentName(match.Yield.Expression())
			newName := func() *ast.Node {
				return tracker.Factory.NewUniqueNameEx(base, printer.AutoGenerateOptions{Flags: printer.GeneratedIdentifierFlagsOptimistic})
			}
			declaration := tracker.NewVariableDeclaration(
				newName(),
				nil,
				nil,
				tracker.NewYieldExpression(tracker.NewToken(ast.KindAsteriskToken), tracker.DeepCloneNode(match.Yield.Expression())),
			)
			statement := tracker.NewVariableStatement(
				nil,
				tracker.NewVariableDeclarationList(tracker.NewNodeList([]*ast.Node{declaration}), ast.NodeFlagsConst),
			)
			ast.SetParentInChildren(statement)

			// No flow edit inserts a statement or replaces a single argument, and
			// new or multi-argument calls have no flow, so the fix edits nodes directly.

			tracker.InsertNodeBefore(sf, anchor, statement, false, rewriter.LeadingTriviaOptionNone)
			tracker.ReplaceNode(sf, match.Argument, newName(), nil)
		},
	}); action != nil {
		return []ls.CodeAction{*action}
	}
	return nil
}

// yieldInCallArgumentMatchAt picks the match whose location is exactly span,
// falling back to the first overlapping one. The exact match comes first because
// a nested yield* such as f(yield* g(yield* x)) lies inside the outer range.
func yieldInCallArgumentMatchAt(matches []rules.YieldInCallArgumentMatch, span core.TextRange) *rules.YieldInCallArgumentMatch {
	for i := range matches {
		if matches[i].Location == span {
			return &matches[i]
		}
	}
	for i := range matches {
		if matches[i].Location.Intersects(span) || span.ContainedBy(matches[i].Location) {
			return &matches[i]
		}
	}
	return nil
}

// yieldInCallArgumentVerbs maps a leading verb of the yielded name to its past
// participle, so `load` becomes `loaded` and `fetchUser` becomes `fetchedUser`.
var yieldInCallArgumentVerbs = map[string]string{
	"load":  "loaded",
	"fetch": "fetched",
	"build": "built",
}

// yieldInCallArgumentName picks the base of the hoisted const name from the
// yielded expression: its identifier or last property name, with a leading verb
// turned into a participle, falling back to `value`.
func yieldInCallArgumentName(operand *ast.Node) string {
	operand = ast.SkipOuterExpressions(operand, ast.OEKParentheses|ast.OEKAssertions)
	segment := ""
	switch operand.Kind {
	case ast.KindIdentifier:
		segment = operand.Text()
	case ast.KindPropertyAccessExpression:
		if name := operand.Name(); name.Kind == ast.KindIdentifier {
			segment = name.Text()
		}
	}
	if segment == "" {
		return "value"
	}
	verbEnd := strings.IndexFunc(segment, func(r rune) bool { return !unicode.IsLower(r) })
	if verbEnd < 0 {
		verbEnd = len(segment)
	}
	if participle, ok := yieldInCallArgumentVerbs[segment[:verbEnd]]; ok {
		segment = participle + segment[verbEnd:]
	}
	if token := scanner.StringToToken(segment); !scanner.IsValidIdentifier(segment) ||
		(token >= ast.KindFirstReservedWord && token <= ast.KindLastFutureReservedWord) {
		return "value"
	}
	return segment
}

// yieldInCallArgumentHoistAnchor returns the statement before which the yield*
// argument can be hoisted without changing what runs before the effect, or nil.
// It climbs from the argument through an allowlist of parent kinds; everything
// evaluated before the argument along the way must be side-effect free.
func yieldInCallArgumentHoistAnchor(arg *ast.Node) *ast.Node {
	child := arg
	anchor := ast.FindAncestorOrQuit(arg.Parent, func(p *ast.Node) ast.FindAncestorResult {
		result := yieldInCallArgumentHoistStep(p, child)
		child = p
		return result
	})
	if anchor == nil {
		return nil
	}
	if anchor.Kind == ast.KindVariableDeclaration {
		anchor = anchor.Parent.Parent
	}
	return yieldInCallArgumentFinalizeAnchor(anchor)
}

// yieldInCallArgumentHoistStep decides one step of the climb from child to its
// parent p: climb on, stop at p as the statement (or the single const
// declaration) to hoist before, or quit because hoisting could change what runs
// before the effect.
func yieldInCallArgumentHoistStep(p *ast.Node, child *ast.Node) ast.FindAncestorResult {
	switch p.Kind {
	case ast.KindParenthesizedExpression, ast.KindAsExpression, ast.KindNonNullExpression, ast.KindYieldExpression:
		return ast.FindAncestorFalse
	case ast.KindCallExpression, ast.KindNewExpression:
		if child == p.Expression() {
			return ast.FindAncestorFalse
		}
		if p.Flags&ast.NodeFlagsOptionalChain != 0 || !yieldInCallArgumentIsSideEffectFree(p.Expression()) {
			return ast.FindAncestorQuit
		}
		for _, argument := range p.Arguments() {
			if argument == child {
				break
			}
			if !yieldInCallArgumentIsSideEffectFree(argument) {
				return ast.FindAncestorQuit
			}
		}
		return ast.FindAncestorFalse
	case ast.KindPropertyAccessExpression:
		if child != p.Expression() {
			return ast.FindAncestorQuit
		}
		return ast.FindAncestorFalse
	case ast.KindElementAccessExpression:
		if child != p.Expression() && (p.Flags&ast.NodeFlagsOptionalChain != 0 || !yieldInCallArgumentIsSideEffectFree(p.Expression())) {
			return ast.FindAncestorQuit
		}
		return ast.FindAncestorFalse
	case ast.KindBinaryExpression:
		binary := p.AsBinaryExpression()
		if child == binary.Left {
			return ast.FindAncestorFalse
		}
		operator := binary.OperatorToken.Kind
		if operator == ast.KindAmpersandAmpersandToken || operator == ast.KindBarBarToken || operator == ast.KindQuestionQuestionToken ||
			ast.IsLogicalOrCoalescingAssignmentOperator(operator) || !yieldInCallArgumentIsSideEffectFree(binary.Left) {
			return ast.FindAncestorQuit
		}
		return ast.FindAncestorFalse
	case ast.KindConditionalExpression:
		if child != p.AsConditionalExpression().Condition {
			return ast.FindAncestorQuit
		}
		return ast.FindAncestorFalse
	case ast.KindExpressionStatement, ast.KindReturnStatement:
		return ast.FindAncestorTrue
	case ast.KindIfStatement:
		if child != p.AsIfStatement().Expression {
			return ast.FindAncestorQuit
		}
		return ast.FindAncestorTrue
	case ast.KindVariableDeclaration:
		list := p.Parent
		if child != p.Initializer() || list == nil || list.Kind != ast.KindVariableDeclarationList ||
			list.Flags&ast.NodeFlagsConst == 0 || len(list.AsVariableDeclarationList().Declarations.Nodes) != 1 ||
			list.Parent == nil || list.Parent.Kind != ast.KindVariableStatement {
			return ast.FindAncestorQuit
		}
		return ast.FindAncestorTrue
	}
	return ast.FindAncestorQuit
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
	node = ast.SkipParentheses(node)
	switch node.Kind {
	case ast.KindIdentifier,
		ast.KindStringLiteral, ast.KindNumericLiteral, ast.KindBigIntLiteral,
		ast.KindNoSubstitutionTemplateLiteral, ast.KindRegularExpressionLiteral,
		ast.KindTrueKeyword, ast.KindFalseKeyword, ast.KindNullKeyword:
		return true
	case ast.KindPrefixUnaryExpression:
		prefix := node.AsPrefixUnaryExpression()
		return prefix.Operator == ast.KindMinusToken && prefix.Operand.Kind == ast.KindNumericLiteral
	case ast.KindPropertyAccessExpression:
		return node.Flags&ast.NodeFlagsOptionalChain == 0 && yieldInCallArgumentIsSideEffectFree(node.Expression())
	}
	return false
}
