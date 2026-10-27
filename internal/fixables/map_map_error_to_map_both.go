package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var MapMapErrorToMapBothFix = fixable.Fixable{
	Name:        "mapMapErrorToMapBoth",
	Description: "Replace with Effect.mapBoth",
	ErrorCodes: []int32{
		tsdiag.Effect_0_Effect_1_transform_the_success_and_failure_channels_one_step_at_a_time_Effect_2_expresses_both_transformations_in_a_single_step_effect_mapMapErrorToMapBoth.Code(),
	},
	FixIDs: []string{"mapMapErrorToMapBoth_fix"},
	Run:    runMapMapErrorToMapBothFix,
}

func runMapMapErrorToMapBothFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeMapMapErrorToMapBoth(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Fixable || (!match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location)) {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect." + match.ReplacementName,
			Run: func(t *rewriter.Tracker) {
				callee := effectModuleMethod(t, match.SourceFile, match.EffectModule, match.ReplacementName)
				// Keep the callbacks in flow order so they are evaluated as before.
				firstName, secondName := "onSuccess", "onFailure"
				if match.FirstFailure {
					firstName, secondName = secondName, firstName
				}
				options := t.NewObjectLiteralExpression(t.NewNodeList([]*ast.Node{
					t.NewPropertyAssignment(nil, t.NewIdentifier(firstName), nil, nil, t.DeepCloneNode(match.FirstCallback)),
					t.NewPropertyAssignment(nil, t.NewIdentifier(secondName), nil, nil, t.DeepCloneNode(match.SecondCallback)),
				}), true)
				t.CollapsePipingFlowTransformations(match.SourceFile, match.Flow, match.Start, match.Start+1,
					rewriter.PipingFlowTransformationReplacement{Callee: callee, Arguments: t.NewNodeList([]*ast.Node{options})})
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}
