package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var ProvideMergeAllToArrayFix = fixable.Fixable{
	Name:        "provideMergeAllToArray",
	Description: "Pass the layers as an array to Effect.provide",
	ErrorCodes: []int32{
		tsdiag.Effect_provide_accepts_an_array_of_layers_and_merges_them_itself_pass_the_layers_as_an_array_instead_of_wrapping_them_in_Layer_mergeAll_effect_provideMergeAllToArray.Code(),
	},
	FixIDs: []string{"provideMergeAllToArray_fix"},
	Run:    runProvideMergeAllToArrayFix,
}

func runProvideMergeAllToArrayFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile
	text := sf.Text()
	for _, match := range rules.AnalyzeProvideMergeAllToArray(ctx.TypeParser, ctx.Checker, sf) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		// Only the call syntax around the layers is replaced, so the layer
		// expressions, spreads, comments and trailing comma stay as written.
		arguments := match.MergeAll.AsCallExpression().Arguments
		openStart := scanner.GetTokenPosOfNode(match.Argument, sf, false)
		closeStart := scanner.SkipTrivia(text, arguments.End())
		if closeStart >= len(text) || text[closeStart] != ')' {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Pass the layers as an array to Effect.provide",
			Run: func(tracker *rewriter.Tracker) {
				tracker.ReplaceRangeWithText(sf, lsproto.Range{
					Start: ctx.BytePosToLSPPosition(openStart),
					End:   ctx.BytePosToLSPPosition(arguments.Pos()),
				}, "[")
				tracker.ReplaceRangeWithText(sf, lsproto.Range{
					Start: ctx.BytePosToLSPPosition(closeStart),
					End:   ctx.BytePosToLSPPosition(match.Argument.End()),
				}, "]")
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}
