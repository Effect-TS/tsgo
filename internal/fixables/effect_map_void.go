package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var EffectMapVoidFix = fixable.Fixable{
	Name:        "effectMapVoid",
	Description: "Replace with Effect.asVoid",
	ErrorCodes:  []int32{tsdiag.This_expression_discards_the_success_value_through_mapping_Effect_asVoid_represents_that_form_directly_effect_effectMapVoid.Code()},
	FixIDs:      []string{"effectMapVoid_fix"},
	Run:         runEffectMapVoidFix,
}

func runEffectMapVoidFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile

	matches := rules.AnalyzeEffectMapVoid(ctx.TypeParser, ctx.Checker, sf)
	for _, match := range matches {
		diagRange := match.Location
		if !diagRange.Intersects(ctx.Span) && !ctx.Span.ContainedBy(diagRange) {
			continue
		}
		if match.Transformation == nil {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.asVoid",
			Run: func(tracker *rewriter.Tracker) {
				// Build Effect.asVoid, preserving the original module reference (and any alias).
				asVoid := effectModuleMethod(tracker, sf, match.EffectModuleNode, "asVoid")
				tracker.ReplacePipingFlowTransformation(sf, match.Transformation, rewriter.PipingFlowTransformationReplacement{
					Callee: asVoid,
				})
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		continue
	}

	return nil
}
