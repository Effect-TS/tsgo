package refactors

import (
	"github.com/effect-ts/tsgo/internal/refactor"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var RemoveUnnecessaryEffectGen = refactor.Refactor{
	Name:        "removeUnnecessaryEffectGen",
	Description: "Remove unnecessary Effect.gen",
	Kind:        "rewrite.effect.removeUnnecessaryEffectGen",
	Run:         runRemoveUnnecessaryEffectGen,
}

func runRemoveUnnecessaryEffectGen(ctx *refactor.Context) []ls.CodeAction {
	c := ctx.Checker

	matches := rules.AnalyzeUnnecessaryEffectGen(ctx.TypeParser, c, ctx.SourceFile)
	if len(matches) == 0 {
		return nil
	}

	token := astnav.GetTokenAtPosition(ctx.SourceFile, ctx.Span.Pos())
	if token == nil {
		return nil
	}

	// Walk ancestor chain to find a call expression that matches one of the analyzed nodes
	for node := token; node != nil; node = node.Parent {
		if node.Kind != ast.KindCallExpression {
			continue
		}

		for _, match := range matches {
			if match.CallNode != node {
				continue
			}

			// Found a matching unnecessary Effect.gen call at cursor
			sf := ctx.SourceFile

			if match.ExplicitReturn || match.SuccessIsVoid {
				// Simple unwrap: delete the prefix and suffix around the yielded expression
				action := ctx.NewRefactorAction(refactor.RefactorAction{
					Description: "Remove unnecessary Effect.gen",
					Run: func(tracker *change.Tracker) {
						tracker.DeleteRange(sf, core.NewTextRange(match.CallNode.Pos(), match.YieldedExpression.Pos()))
						tracker.DeleteRange(sf, core.NewTextRange(match.YieldedExpression.End(), match.CallNode.End()))
					},
				})
				if action == nil {
					return nil
				}
				action.Kind = "refactor.rewrite.effect.removeUnnecessaryEffectGen"
				return []ls.CodeAction{*action}
			}

			// No return + non-void success: wrap with Effect.asVoid(expr)
			action := ctx.NewRefactorAction(refactor.RefactorAction{
				Description: "Remove unnecessary Effect.gen",
				Run: func(tracker *change.Tracker) {
					var effectModuleId *ast.Node
					if match.EffectModuleNode != nil && match.EffectModuleNode.Kind == ast.KindIdentifier {
						effectModuleId = tracker.DeepCloneNode(match.EffectModuleNode)
					} else {
						effectModuleId = tracker.NewIdentifier("Effect")
					}

					asVoidAccess := tracker.NewPropertyAccessExpression(effectModuleId, nil, tracker.NewIdentifier("asVoid"), ast.NodeFlagsNone)
					clonedExpr := tracker.DeepCloneNode(match.YieldedExpression)
					callExpr := tracker.NewCallExpression(asVoidAccess, nil, nil, tracker.NewNodeList([]*ast.Node{clonedExpr}), ast.NodeFlagsNone)
					ast.SetParentInChildren(callExpr)
					tracker.ReplaceNode(sf, match.CallNode, callExpr, nil)
				},
			})
			if action == nil {
				return nil
			}
			action.Kind = "refactor.rewrite.effect.removeUnnecessaryEffectGen"
			return []ls.CodeAction{*action}
		}
	}

	return nil
}
