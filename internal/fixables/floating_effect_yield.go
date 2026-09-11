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

// FloatingEffectYieldFix yields a floating Effect when it occurs in an Effect generator.
var FloatingEffectYieldFix = fixable.Fixable{
	Name:        "floatingEffectYield",
	Description: "Add yield* statement",
	ErrorCodes: []int32{
		tsdiag.This_Effect_value_is_neither_yielded_nor_used_in_an_assignment_effect_floatingEffect.Code(),
		tsdiag.This_Effect_able_0_value_is_neither_yielded_nor_assigned_to_a_variable_effect_floatingEffect.Code(),
	},
	FixIDs: []string{"floatingEffectYield_fix"},
	Run:    runFloatingEffectYieldFix,
}

func runFloatingEffectYieldFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeFloatingEffect(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if ctx.TypeParser.GetEffectContextFlags(match.Expression)&typeparser.EffectContextFlagCanYieldEffect == 0 {
			return nil
		}
		exprType := ctx.TypeParser.GetTypeAtLocation(match.Expression)
		if ctx.TypeParser.EffectYieldableType(exprType) == nil {
			return nil
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Add yield* statement",
			Run: func(tracker *rewriter.Tracker) {
				clonedExpr := tracker.DeepCloneNode(match.Expression)
				yieldExpr := tracker.NewYieldExpression(tracker.NewToken(ast.KindAsteriskToken), clonedExpr)
				ast.SetParentInChildren(yieldExpr)
				tracker.ReplaceNode(ctx.SourceFile, match.Expression, yieldExpr, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}
	return nil
}
