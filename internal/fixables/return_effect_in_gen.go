// Package fixables contains all code fix implementations.
package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

// ReturnEffectInGenFix adds "yield*" before the return expression in an Effect generator
// when the return value is an Effect-able type (which would cause nested Effect<Effect<...>>).
var ReturnEffectInGenFix = fixable.Fixable{
	Name:        "returnEffectInGen",
	Description: "Add yield* statement",
	ErrorCodes:  []int32{tsdiag.You_are_returning_an_Effect_able_type_inside_a_generator_function_and_will_result_in_nested_Effect_Effect_Maybe_you_wanted_to_return_yield_Asterisk_instead_Nested_Effect_able_types_may_be_intended_if_you_plan_to_later_manually_flatten_or_unwrap_this_Effect_if_so_you_can_safely_disable_this_diagnostic_for_this_line_through_quickfixes_effect_returnEffectInGen.Code()},
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
		Run: func(tracker *change.Tracker) {
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
