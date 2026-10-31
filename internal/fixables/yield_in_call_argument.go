package fixables

import (
	"strconv"
	"strings"
	"unicode"

	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
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
	if match == nil || match.Statement == nil {
		return nil
	}

	name := yieldInCallArgumentName(ctx.Checker, match.Yield.Expression(), match.Statement)
	if action := ctx.NewFixAction(fixable.FixAction{
		Description: "Hoist yield* into a const",
		Run: func(tracker *rewriter.Tracker) {
			declaration := tracker.NewVariableDeclaration(
				tracker.NewIdentifier(name),
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

			pos := scanner.GetTokenPosOfNode(match.Statement, sf, true)
			tracker.InsertNodeAt(sf, core.TextPos(pos), statement, rewriter.NodeOptions{Suffix: typeUnionToSchemaUnionInsertionSuffix(sf, pos)})
			tracker.ReplaceNode(sf, match.Argument, tracker.NewIdentifier(name), nil)
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

// yieldInCallArgumentName picks the hoisted const name from the yielded
// expression (its identifier or last property name, with a leading verb turned
// into a participle), falling back to `value`, then appends 2, 3, ... until no
// value with that name is visible at the statement.
func yieldInCallArgumentName(c *checker.Checker, operand *ast.Node, statement *ast.Node) string {
	base := "value"
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
	if segment != "" {
		verbEnd := strings.IndexFunc(segment, func(r rune) bool { return !unicode.IsLower(r) })
		if verbEnd < 0 {
			verbEnd = len(segment)
		}
		if participle, ok := yieldInCallArgumentVerbs[segment[:verbEnd]]; ok {
			segment = participle + segment[verbEnd:]
		}
		if token := scanner.StringToToken(segment); scanner.IsValidIdentifier(segment) &&
			(token < ast.KindFirstReservedWord || token > ast.KindLastFutureReservedWord) {
			base = segment
		}
	}

	isFree := func(candidate string) bool {
		return c.ResolveName(candidate, statement, ast.SymbolFlagsValue|ast.SymbolFlagsAlias, false) == nil
	}
	if isFree(base) {
		return base
	}
	for i := 2; ; i++ {
		if candidate := base + strconv.Itoa(i); isFree(candidate) {
			return candidate
		}
	}
}
