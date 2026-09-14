package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var UnnecessaryEffectGenFix = fixable.Fixable{
	Name:        "unnecessaryEffectGen",
	Description: "Remove the Effect.gen, and keep the body",
	ErrorCodes:  []int32{tsdiag.This_Effect_gen_contains_a_single_return_statement_effect_unnecessaryEffectGen.Code()},
	FixIDs:      []string{"unnecessaryEffectGen_fix"},
	Run:         runUnnecessaryEffectGenFix,
}

func runUnnecessaryEffectGenFix(ctx *fixable.Context) []ls.CodeAction {

	c := ctx.Checker

	sf := ctx.SourceFile

	matches := rules.AnalyzeUnnecessaryEffectGen(ctx.TypeParser, c, sf)
	for _, match := range matches {
		diagRange := match.Location
		if !diagRange.Intersects(ctx.Span) && !ctx.Span.ContainedBy(diagRange) {
			continue
		}

		if match.ExplicitReturn || match.SuccessIsVoid {
			// Simple unwrap: delete the prefix and suffix around the yielded expression
			if action := ctx.NewFixAction(fixable.FixAction{
				Description: "Remove the Effect.gen, and keep the body",
				Run: func(tracker *rewriter.Tracker) {
					tracker.DeleteRange(sf, core.NewTextRange(match.CallNode.Pos(), match.YieldedExpression.Pos()))
					tracker.DeleteRange(sf, core.NewTextRange(match.YieldedExpression.End(), match.CallNode.End()))
				},
			}); action != nil {
				return []ls.CodeAction{*action}
			}
			continue
		}

		// No return + non-void success: wrap with Effect.asVoid(...)
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Remove the Effect.gen, and keep the body",
			Run: func(tracker *rewriter.Tracker) {
				asVoidAccess := effectModuleMethod(tracker, sf, match.EffectModuleNode, "asVoid")

				// Clone the yielded expression
				clonedExpr := tracker.DeepCloneNode(match.YieldedExpression)

				// Build Effect.asVoid(expr) call expression
				callExpr := tracker.NewCallExpression(asVoidAccess, nil, nil, tracker.NewNodeList([]*ast.Node{clonedExpr}), ast.NodeFlagsNone)

				ast.SetParentInChildren(callExpr)
				tracker.ReplaceNode(sf, match.CallNode, callExpr, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		continue
	}

	return nil
}
