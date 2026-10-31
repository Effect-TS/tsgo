package fixables

import (
	"strings"

	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var ForLoopPushToForEachFix = fixable.Fixable{
	Name:        "forLoopPushToForEach",
	Description: "Replace for-of push loop with Effect.forEach",
	ErrorCodes: []int32{
		tsdiag.Effect_forEach_expresses_this_for_of_loop_that_pushes_yielded_results_into_an_accumulator_more_directly_and_makes_ordering_and_concurrency_explicit_effect_forLoopPushToForEach.Code(),
	},
	FixIDs: []string{"forLoopPushToForEach_fix"},
	Run:    runForLoopPushToForEachFix,
}

func runForLoopPushToForEachFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile
	for _, match := range rules.AnalyzeForLoopPushToForEach(ctx.TypeParser, ctx.Checker, sf) {
		if !match.CanFix {
			continue
		}
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		forEach := effectModuleMember(ctx, "forEach")
		reference := resolveModuleMember(ctx, match.Loop, typeparser.FindEffectModuleReference(sf), forEach)
		if reference == nil {
			continue
		}

		// The fix deletes one statement and replaces another, the loop; the
		// piping flow edits only rewrite steps inside a flow, so none applies.
		// The replacement is written as text: the printer drops the parentheses
		// of a synthesized single-identifier arrow parameter.
		callee := reference.path.String()
		if reference.viaModule {
			callee += "." + forEach.name
		}
		callback := forLoopPushToForEachNodeText(sf, match.PointFreeCallee)
		if match.PointFreeCallee == nil {
			body := forLoopPushToForEachNodeText(sf, match.Yielded)
			// A concise arrow body starting with `{` would parse as a block.
			if strings.HasPrefix(body, "{") {
				body = "(" + body + ")"
			}
			callback = "(" + forLoopPushToForEachNodeText(sf, match.Binding) + ") => " + body
		}
		declaration := "const " + forLoopPushToForEachNodeText(sf, match.AccumulatorName)
		if match.AccumulatorType != nil {
			declaration += ": " + forLoopPushToForEachNodeText(sf, match.AccumulatorType)
		}
		replacement := declaration + " = yield* " + callee + "(" + forLoopPushToForEachNodeText(sf, match.Iterable) + ", " + callback + ")"
		if forLoopPushToForEachNeedsSemicolon(sf, match.Loop) {
			replacement += ";"
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.forEach",
			Run: func(tracker *rewriter.Tracker) {
				tracker.Delete(sf, match.Declaration)
				tracker.ReplaceRangeWithText(sf, lsproto.Range{
					Start: ctx.BytePosToLSPPosition(scanner.GetTokenPosOfNode(match.Loop, sf, false)),
					End:   ctx.BytePosToLSPPosition(match.Loop.End()),
				}, replacement)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}

// forLoopPushToForEachNeedsSemicolon reports whether the declaration replacing
// loop must end with a semicolon. A for statement delimits itself, while the
// declaration relies on automatic semicolon insertion: it keeps the semicolon
// that ended a statement body, and adds one when the next statement starts on
// the same line or with a token that would continue the expression.
func forLoopPushToForEachNeedsSemicolon(sf *ast.SourceFile, loop *ast.Node) bool {
	text := sf.Text()
	end := loop.End()
	if strings.HasSuffix(text[:end], ";") {
		return true
	}
	next := scanner.SkipTrivia(text, end)
	if next >= len(text) || text[next] == '}' || text[next] == ';' {
		return false
	}
	if !strings.ContainsAny(text[end:next], "\n\r\u2028\u2029") {
		return true
	}
	return strings.IndexByte("([`+-/<", text[next]) >= 0
}

func forLoopPushToForEachNodeText(sf *ast.SourceFile, node *ast.Node) string {
	if node == nil {
		return ""
	}
	return scanner.GetSourceTextOfNodeFromSourceFile(sf, node, false)
}
