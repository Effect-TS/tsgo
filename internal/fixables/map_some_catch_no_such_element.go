package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var MapSomeCatchNoSuchElementFix = fixable.Fixable{
	Name:        "mapSomeCatchNoSuchElement",
	Description: "Replace with Effect.catchNoSuchElement",
	ErrorCodes:  []int32{tsdiag.Use_Effect_catchNoSuchElement_instead_of_wrapping_successes_in_Option_some_and_catching_NoSuchElementError_into_Option_none_effect_mapSomeCatchNoSuchElement.Code()},
	FixIDs:      []string{"mapSomeCatchNoSuchElement_fix"},
	Run: func(ctx *fixable.Context) []ls.CodeAction {
		for _, match := range rules.AnalyzeMapSomeCatchNoSuchElement(ctx.TypeParser, ctx.SourceFile) {
			if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
				continue
			}
			if action := ctx.NewFixAction(fixable.FixAction{
				Description: "Replace with Effect.catchNoSuchElement",
				Run: func(t *rewriter.Tracker) {
					callee := t.NewPropertyAccessPath(typeparser.FindEffectModuleReference(ctx.SourceFile).Append("catchNoSuchElement")...)
					t.CollapsePipingFlowTransformations(ctx.SourceFile, match.Flow, match.Start, match.End, rewriter.PipingFlowTransformationReplacement{Callee: callee})
				},
			}); action != nil {
				return []ls.CodeAction{*action}
			}
		}
		return nil
	},
}
