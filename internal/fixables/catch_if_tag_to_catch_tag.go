package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var CatchIfTagToCatchTagFix = fixable.Fixable{
	Name: "catchIfTagToCatchTag", Description: "Replace Effect.catchIf with Effect.catchTag",
	ErrorCodes: []int32{tsdiag.Effect_catchTag_expresses_tagged_error_recovery_more_directly_than_Effect_catchIf_with_a_tag_equality_predicate_effect_catchIfTagToCatchTag.Code()},
	FixIDs:     []string{"catchIfTagToCatchTag_fix"}, Run: runCatchIfTagToCatchTagFix,
}

func runCatchIfTagToCatchTagFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeCatchIfTagToCatchTag(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		action := ctx.NewFixAction(fixable.FixAction{Description: "Replace with Effect.catchTag", Run: func(tracker *rewriter.Tracker) {
			access := match.Transformation.Callee.AsPropertyAccessExpression()
			if access == nil || access.Expression == nil {
				return
			}
			callee := tracker.NewPropertyAccessExpression(tracker.DeepCloneNode(access.Expression), nil, tracker.NewIdentifier("catchTag"), ast.NodeFlagsNone)
			arguments := tracker.NewNodeList([]*ast.Node{tracker.NewStringLiteral(match.Tag, 0), tracker.DeepCloneNode(match.Handler)})
			tracker.ReplacePipingFlowTransformation(ctx.SourceFile, match.Transformation, rewriter.PipingFlowTransformationReplacement{Callee: callee, Arguments: arguments})
		}})
		if action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}
