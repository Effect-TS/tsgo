package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var MapConstToAsFix = fixable.Fixable{
	Name:        "mapConstToAs",
	Description: "Replace Effect.map with Effect.as",
	ErrorCodes: []int32{
		tsdiag.Effect_as_expresses_replacing_the_success_value_with_a_constant_directly_effect_mapConstToAs.Code(),
	},
	FixIDs: []string{"mapConstToAs_fix"},
	Run:    runMapConstToAsFix,
}

func runMapConstToAsFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeMapConstToAs(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if match.Transformation == nil {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.as",
			Run: func(tracker *rewriter.Tracker) {
				as := tracker.NewPropertyAccessPath(typeparser.FindEffectModuleReference(ctx.SourceFile).Append("as")...)
				tracker.ReplacePipingFlowTransformation(ctx.SourceFile, match.Transformation, rewriter.PipingFlowTransformationReplacement{
					Callee:    as,
					Arguments: tracker.NewNodeList([]*ast.Node{tracker.DeepCloneNode(match.ConstantValue)}),
				})
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}
