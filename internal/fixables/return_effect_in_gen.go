// Package fixables contains all code fix implementations.
package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/effect-ts/tsgo/internal/rewriter"
)

// ReturnEffectInGenFix adds "yield*" before the return expression in an Effect generator
// when the return value is an Effect-able type (which would cause nested Effect<Effect<...>>).
var ReturnEffectInGenFix = fixable.Fixable{
	Name:        "returnEffectInGen",
	Description: "Add yield* statement",
	ErrorCodes:  []int32{tsdiag.This_generator_returns_an_Effect_able_value_directly_which_produces_a_nested_Effect_Effect_If_the_intended_result_is_the_inner_Effect_value_return_yield_Asterisk_represents_that_form_effect_returnEffectInGen.Code()},
	FixIDs:      []string{"returnEffectInGen_fix"},
	Run:         runReturnEffectInGenFix,
}

func runReturnEffectInGenFix(ctx *fixable.Context) []ls.CodeAction {

	c := ctx.Checker

	sf := ctx.SourceFile

	matches := rules.AnalyzeReturnEffectInGen(ctx.TypeParser, c, sf)

	var match *rules.ReturnEffectInGenMatch
	for i := range matches {
		diagRange := matches[i].Location
		if diagRange.Intersects(ctx.Span) || ctx.Span.ContainedBy(diagRange) {
			match = &matches[i]
			break
		}
	}
	if match == nil {
		return nil
	}

	if action := ctx.NewFixAction(fixable.FixAction{
		Description: "Add yield* statement",
		Run: func(tracker *rewriter.Tracker) {
			clonedExpr := tracker.DeepCloneNode(match.ReturnNode.AsReturnStatement().Expression)
			newYieldExpr := tracker.NewYieldExpression(tracker.NewToken(ast.KindAsteriskToken), clonedExpr)
			ast.SetParentInChildren(newYieldExpr)
			tracker.ReplaceNode(sf, match.ReturnNode.AsReturnStatement().Expression, newYieldExpr, nil)
		},
	}); action != nil {
		return []ls.CodeAction{*action}
	}
	return nil
}
