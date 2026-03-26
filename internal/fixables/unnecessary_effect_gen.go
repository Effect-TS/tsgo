package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var UnnecessaryEffectGenFix = fixable.Fixable{
	Name:        "unnecessaryEffectGen",
	Description: "Remove the Effect.gen, and keep the body",
	ErrorCodes:  []int32{tsdiag.This_Effect_gen_contains_a_single_return_statement_effect_unnecessaryEffectGen.Code()},
	FixIDs:      []string{"unnecessaryEffectGen_fix"},
	Run:         runUnnecessaryEffectGenFix,
}

func runUnnecessaryEffectGenFix(ctx *fixable.Context) []ls.CodeAction {

	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	sf := ctx.SourceFile

	matches := rules.AnalyzeUnnecessaryEffectGen(c, sf)
	for _, match := range matches {
		diagRange := match.Location
		if !diagRange.Intersects(ctx.Span) && !ctx.Span.ContainedBy(diagRange) {
			continue
		}

		if match.ExplicitReturn || match.SuccessIsVoid {
			// Simple unwrap: delete the prefix and suffix around the yielded expression
			if action := ctx.NewFixAction(fixable.FixAction{
				Description: "Remove the Effect.gen, and keep the body",
				Run: func(tracker *change.Tracker) {
					tracker.DeleteRange(sf, core.NewTextRange(match.CallNode.Pos(), match.YieldedExpression.Pos()))
					tracker.DeleteRange(sf, core.NewTextRange(match.YieldedExpression.End(), match.CallNode.End()))
				},
			}); action != nil {
				return []ls.CodeAction{*action}
			}
			return nil
		}

		// No return + non-void success: wrap with Effect.asVoid(...)
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Remove the Effect.gen, and keep the body",
			Run: func(tracker *change.Tracker) {
				// Clone the Effect module identifier (or create a fallback)
				var effectModuleId *ast.Node
				if match.EffectModuleNode != nil && match.EffectModuleNode.Kind == ast.KindIdentifier {
					effectModuleId = tracker.DeepCloneNode(match.EffectModuleNode)
				} else {
					effectModuleId = tracker.NewIdentifier("Effect")
				}

				// Build Effect.asVoid property access
				asVoidAccess := tracker.NewPropertyAccessExpression(effectModuleId, nil, tracker.NewIdentifier("asVoid"), ast.NodeFlagsNone)

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
		return nil
	}

	return nil
}
