package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/effect-ts/tsgo/internal/rewriter"
)

var MissingStarInYieldEffectGenFix = fixable.Fixable{
	Name:        "missingStarInYieldEffectGen",
	Description: "Replace yield with yield* inside Effect generator scopes",
	ErrorCodes:  []int32{tsdiag.This_uses_yield_for_an_Effect_value_yield_Asterisk_is_the_Effect_aware_form_in_this_context_effect_missingStarInYieldEffectGen.Code()},
	FixIDs:      []string{"missingStarInYieldEffectGen_fix"},
	Run:         runMissingStarInYieldEffectGenFix,
}

func runMissingStarInYieldEffectGenFix(ctx *fixable.Context) []ls.CodeAction {

	c := ctx.Checker

	sf := ctx.SourceFile

	matches := rules.AnalyzeMissingStarInYieldEffectGen(ctx.TypeParser, c, sf)

	var yieldNode *ast.Node
	for _, match := range matches {
		diagRange := match.Location
		if diagRange.Intersects(ctx.Span) || ctx.Span.ContainedBy(diagRange) {
			yieldNode = match.YieldNode
			break
		}
	}
	if yieldNode == nil {
		return nil
	}

	if action := ctx.NewFixAction(fixable.FixAction{
		Description: "Replace yield with yield*",
		Run: func(tracker *rewriter.Tracker) {
			clonedExpr := tracker.DeepCloneNode(yieldNode.AsYieldExpression().Expression)
			newYieldExpr := tracker.NewYieldExpression(tracker.NewToken(ast.KindAsteriskToken), clonedExpr)
			ast.SetParentInChildren(newYieldExpr)
			tracker.ReplaceNode(sf, yieldNode, newYieldExpr, nil)
		},
	}); action != nil {
		return []ls.CodeAction{*action}
	}
	return nil
}
