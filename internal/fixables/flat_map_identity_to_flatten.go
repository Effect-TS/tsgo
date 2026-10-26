package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var FlatMapIdentityToFlattenFix = fixable.Fixable{
	Name:        "flatMapIdentityToFlatten",
	Description: "Replace Effect.flatMap with an identity callback by Effect.flatten",
	ErrorCodes: []int32{
		tsdiag.Effect_flatten_expresses_this_unwrapping_more_directly_than_Effect_flatMap_with_an_identity_callback_effect_flatMapIdentityToFlatten.Code(),
	},
	FixIDs: []string{"flatMapIdentityToFlatten_fix"},
	Run:    runFlatMapIdentityToFlattenFix,
}

func runFlatMapIdentityToFlattenFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeFlatMapIdentityToFlatten(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		// Without an Effect module receiver there is no bound name to reach flatten through.
		if match.EffectModuleNode == nil {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.flatten",
			Run: func(tracker *rewriter.Tracker) {
				flatten := effectModuleMethod(tracker, match.SourceFile, match.EffectModuleNode, "flatten")
				ast.SetParentInChildren(flatten)
				tracker.ReplacePipingFlowTransformation(match.SourceFile, match.Transformation, rewriter.PipingFlowTransformationReplacement{
					Callee: flatten,
				})
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}
