package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var EffectSleepZeroToYieldNowFix = fixable.Fixable{
	Name:        "effectSleepZeroToYieldNow",
	Description: "Replace zero-duration sleep with cooperative yielding",
	ErrorCodes: []int32{
		tsdiag.Consider_Effect_yieldNow_for_cooperative_yielding_instead_of_sleeping_for_zero_duration_effect_effectSleepZeroToYieldNow.Code(),
	},
	FixIDs: []string{"effectSleepZeroToYieldNow_fix"},
	Run:    runEffectSleepZeroToYieldNowFix,
}

func runEffectSleepZeroToYieldNowFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeEffectSleepZeroToYieldNow(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if match.EffectModule == nil || (!match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location)) {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.yieldNow",
			Run: func(tracker *rewriter.Tracker) {
				replacement := effectModuleMethod(tracker, match.SourceFile, match.EffectModule, "yieldNow")
				if match.Form == rules.YieldNowCall {
					replacement = tracker.NewCallExpression(replacement, nil, nil, tracker.NewNodeList(nil), ast.NodeFlagsNone)
				}
				tracker.ReplaceNode(match.SourceFile, match.Call, replacement, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}
