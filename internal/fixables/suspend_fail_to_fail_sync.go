package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
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
		lastIndex := match.Inner.Transformations().Len() - 1
		last := match.Inner.Transformations().At(lastIndex)
		valueTypeArguments := last.TypeArguments()
		hasValueTypeArguments := valueTypeArguments != nil && len(valueTypeArguments.Nodes) > 0
		// Explicit type arguments pin the error type in both forms.
		if match.InnerName == "fail" && !hasValueTypeArguments &&
			!failSyncKeepsErrorType(ctx.TypeParser, ctx.Checker, last, match.Inner.TransformationInputType(lastIndex)) {
			continue
		}
		value := match.Inner.TransformationInputNode(lastIndex)
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
				if !replaceSuspendThunkBody(tracker, sf, match, value) {
					return
				}

				var callee *ast.Node
				if qualifier != nil {
					callee = tracker.NewPropertyAccessExpression(tracker.DeepCloneNode(qualifier), nil, tracker.NewIdentifier(match.ReplacementName), ast.NodeFlagsNone)
				} else {
					callee = target.expression(tracker, member)
				}
				if hasValueTypeArguments {
					arguments := make([]*ast.Node, len(valueTypeArguments.Nodes))
					for i, argument := range valueTypeArguments.Nodes {
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
// failing with it. value is the input of the inner flow's last step, when it is
// a standalone node.
func replaceSuspendThunkBody(tracker *rewriter.Tracker, sf *ast.SourceFile, match rules.SuspendFailToFailSyncMatch, value *ast.Node) bool {
	returned := match.Thunk.Expression()
	conciseBody := !ast.IsBlock(match.Thunk.Body())
	// Removing the step keeps its input as written, which cannot drop the
	// parentheses around the returned flow or wrap an object literal that a
	// concise arrow body would parse as a block, so those replace the returned
	// expression with the value.
	if value != nil && (returned != match.Inner.Node() ||
		conciseBody && ast.IsObjectLiteralExpression(ast.SkipParentheses(value))) {
		replacement := tracker.DeepCloneNode(value)
		if conciseBody && ast.IsObjectLiteralExpression(ast.SkipParentheses(value)) {
			replacement = tracker.NewParenthesizedExpression(replacement)
		}
		tracker.ReplaceNode(sf, returned, replacement, nil)
		return true
	}
	return tracker.RemoveTrailingPipingFlowTransformation(sf, match.Inner)
}

// failSyncKeepsErrorType reports whether Effect.failSync(() => value) infers the
// same error type as Effect.fail(value). Effect.fail infers its error from a
// call argument, which widens literal unions, while a failSync thunk returns the
// value against LazyArg<E>, which keeps them: for `cond ? "A" : "B"` the first
// infers string and the second "A" | "B". A single literal widens in both forms.
// The input type of a flow step is a regular literal type, which only the base
// type of a literal widens.
func failSyncKeepsErrorType(tp *typeparser.TypeParser, c *checker.Checker, failStep typeparser.PipingFlowTransformation, valueType *checker.Type) bool {
	effect := tp.EffectType(failStep.OutType())
	if effect == nil || effect.E() == nil || valueType == nil {
		return false
	}
	sameType := func(t *checker.Type) bool {
		return checker.Checker_isTypeAssignableTo(c, t, effect.E()) &&
			checker.Checker_isTypeAssignableTo(c, effect.E(), t)
	}
	return sameType(valueType) ||
		valueType.Flags()&checker.TypeFlagsUnit != 0 && sameType(c.GetBaseTypeOfLiteralType(valueType))
}
