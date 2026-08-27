package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
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

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.asVoid",
			Run: func(tracker *rewriter.Tracker) {
				// Build Effect.asVoid, preserving the original module reference (and any alias).
				asVoid := tracker.NewPropertyAccessExpression(
					tracker.DeepCloneNode(match.EffectModuleNode),
					nil,
					tracker.NewIdentifier("asVoid"),
					ast.NodeFlagsNone,
				)
				// Data-last/pipeable forms drop to a bare Effect.asVoid reference; the
				// data-first form must keep its subject as Effect.asVoid(self).
				var replacement = asVoid
				if match.SubjectNode != nil {
					replacement = tracker.NewCallExpression(
						asVoid,
						nil,
						nil,
						tracker.NewNodeList([]*ast.Node{tracker.DeepCloneNode(match.SubjectNode)}),
						ast.NodeFlagsNone,
					)
				}
				tracker.ReplaceNode(sf, match.CallNode, replacement, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
