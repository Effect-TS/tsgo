package fixables

import (
	"strings"

	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var UseSpanUnusedSpanToWithSpanFix = fixable.Fixable{
	Name:        "useSpanUnusedSpanToWithSpan",
	Description: "Use Effect.withSpan",
	ErrorCodes: []int32{
		tsdiag.This_Effect_useSpan_callback_never_uses_the_span_it_receives_Effect_useSpan_does_not_make_that_span_the_parent_of_spans_created_inside_the_callback_so_they_will_not_nest_under_it_Effect_withSpan_does_and_removes_ParentSpan_from_the_requirements_effect_useSpanUnusedSpanToWithSpan.Code(),
	},
	FixIDs: []string{"useSpanUnusedSpanToWithSpan_fix"},
	Run:    runUseSpanUnusedSpanToWithSpanFix,
}

// runUseSpanUnusedSpanToWithSpanFix rewrites `R.useSpan(name, options?, () => body)`
// to `body.pipe(R.withSpan(name, options?))`. It edits the text around the body
// so the body, name and options keep their original formatting and comments.
func runUseSpanUnusedSpanToWithSpanFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeUseSpanUnusedSpanToWithSpan(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if match.Receiver == nil || match.Body == nil || (!match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location)) {
			continue
		}
		parenthesize, ok := pipeReceiverParentheses(match.SourceFile, match.Call, match.Body)
		if !ok {
			continue
		}
		sf := match.SourceFile
		nodeText := func(node *ast.Node) string {
			return sf.Text()[scanner.GetTokenPosOfNode(node, sf, false):node.End()]
		}
		var prefix, suffix strings.Builder
		if parenthesize {
			prefix.WriteString("(")
			suffix.WriteString(")")
		}
		suffix.WriteString(".pipe(")
		suffix.WriteString(nodeText(match.Receiver))
		suffix.WriteString(".withSpan(")
		suffix.WriteString(nodeText(match.Name))
		if match.Options != nil {
			suffix.WriteString(", ")
			suffix.WriteString(nodeText(match.Options))
		}
		suffix.WriteString("))")

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Use Effect.withSpan",
			Run: func(tracker *rewriter.Tracker) {
				tracker.ReplaceRangeWithText(sf, lsproto.Range{
					Start: ctx.BytePosToLSPPosition(scanner.GetTokenPosOfNode(match.Call, sf, false)),
					End:   ctx.BytePosToLSPPosition(scanner.GetTokenPosOfNode(match.Body, sf, false)),
				}, prefix.String())
				tracker.ReplaceRangeWithText(sf, lsproto.Range{
					Start: ctx.BytePosToLSPPosition(match.Body.End()),
					End:   ctx.BytePosToLSPPosition(match.Call.End()),
				}, suffix.String())
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}
