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

var SuspendFailToFailSyncFix = fixable.Fixable{
	Name:        "suspendFailToFailSync",
	Description: "Replace with Effect.failSync or Effect.dieSync",
	ErrorCodes: []int32{
		tsdiag.Effect_0_builds_this_1_lazily_directly_instead_of_wrapping_Effect_2_in_Effect_suspend_effect_suspendFailToFailSync.Code(),
	},
	FixIDs: []string{"suspendFailToFailSync_fix"},
	Run:    runSuspendFailToFailSyncFix,
}

func runSuspendFailToFailSyncFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile
	for _, match := range rules.AnalyzeSuspendFailToFailSync(ctx.TypeParser, ctx.Checker, sf) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if !match.Fixable {
			continue
		}
		member := effectModuleMember(ctx, match.ReplacementName)
		suspendCallee := match.Suspend.Callee()
		// Keep the qualifier of Effect.suspend when it is the Effect module, which
		// already resolves where the edit lands; otherwise go through the file's
		// Effect import.
		var qualifier *ast.Node
		var target *moduleMemberReference
		if ast.IsPropertyAccessExpression(suspendCallee) {
			expression := suspendCallee.AsPropertyAccessExpression().Expression
			if ctx.TypeParser.IsExpressionEffectModule(expression) && typeHasMember(ctx, ctx.TypeParser.GetTypeAtLocation(expression), member) {
				qualifier = expression
			}
		}
		if qualifier == nil {
			target = importedModuleMember(ctx, suspendCallee, typeparser.FindEffectModuleReference(sf), member)
			if target == nil {
				continue
			}
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect." + match.ReplacementName,
			Run: func(tracker *rewriter.Tracker) {
				if !replaceSuspendThunkBody(tracker, sf, match) {
					return
				}

				var callee *ast.Node
				if qualifier != nil {
					callee = tracker.NewPropertyAccessExpression(tracker.DeepCloneNode(qualifier), nil, tracker.NewIdentifier(match.ReplacementName), ast.NodeFlagsNone)
				} else {
					callee = target.expression(tracker, member)
				}
				if match.ValueTypeArguments != nil && len(match.ValueTypeArguments.Nodes) > 0 {
					arguments := make([]*ast.Node, len(match.ValueTypeArguments.Nodes))
					for i, argument := range match.ValueTypeArguments.Nodes {
						arguments[i] = tracker.DeepCloneNode(argument)
					}
					callee = tracker.NewExpressionWithTypeArguments(callee, tracker.NewNodeList(arguments))
				}

				if match.Suspend.Kind() == typeparser.TransformationKindCall {
					// The step replacement would reprint the whole suspend(thunk) call,
					// overlapping the edit of the thunk body, so only the callee is
					// replaced.
					tracker.ReplaceNode(sf, suspendCallee, callee, nil)
					return
				}
				tracker.ReplacePipingFlowTransformation(sf, &match.Suspend, rewriter.PipingFlowTransformationReplacement{Callee: callee})
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}

// replaceSuspendThunkBody makes the thunk return the error value instead of
// failing with it.
func replaceSuspendThunkBody(tracker *rewriter.Tracker, sf *ast.SourceFile, match rules.SuspendFailToFailSyncMatch) bool {
	returned := match.Thunk.Expression()
	conciseBody := !ast.IsBlock(match.Thunk.Body())
	// Removing the step keeps its input as written, which cannot drop the
	// parentheses around the returned flow or wrap an object literal that a
	// concise arrow body would parse as a block, so those replace the returned
	// expression with the value.
	if match.Value != nil && (returned != match.Inner.Node() ||
		conciseBody && ast.IsObjectLiteralExpression(ast.SkipParentheses(match.Value))) {
		value := tracker.DeepCloneNode(match.Value)
		if conciseBody && ast.IsObjectLiteralExpression(ast.SkipParentheses(match.Value)) {
			value = tracker.NewParenthesizedExpression(value)
		}
		tracker.ReplaceNode(sf, returned, value, nil)
		return true
	}
	return tracker.RemoveTrailingPipingFlowTransformation(sf, match.Inner)
}
