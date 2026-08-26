package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var MapSomeToAsSomeFix = fixable.Fixable{
	Name:        "mapSomeToAsSome",
	Description: "Replace Effect.map(Option.some) with Effect.asSome",
	ErrorCodes: []int32{
		tsdiag.Effect_asSome_expresses_wrapping_the_success_value_in_Option_some_directly_effect_mapSomeToAsSome.Code(),
	},
	FixIDs: []string{"mapSomeToAsSome_fix"},
	Run:    runMapSomeToAsSomeFix,
}

func runMapSomeToAsSomeFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeMapSomeToAsSome(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.asSome",
			Run: func(tracker *rewriter.Tracker) {
				asSome := tracker.NewPropertyAccessExpression(
					tracker.DeepCloneNode(match.EffectModuleNode),
					nil,
					tracker.NewIdentifier("asSome"),
					ast.NodeFlagsNone,
				)
				var replacement = asSome
				if match.SubjectNode != nil {
					replacement = tracker.NewCallExpression(
						asSome,
						nil,
						nil,
						tracker.NewNodeList([]*ast.Node{tracker.DeepCloneNode(match.SubjectNode)}),
						ast.NodeFlagsNone,
					)
				}
				tracker.ReplaceNode(ctx.SourceFile, match.CallNode, replacement, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}
