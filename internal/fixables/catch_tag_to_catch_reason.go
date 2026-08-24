package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var CatchTagToCatchReasonFix = fixable.Fixable{
	Name:        "catchTagToCatchReason",
	Description: "Replace hand-rolled reason dispatch with Effect.catchReason or Effect.catchReasons",
	ErrorCodes: []int32{
		tsdiag.Branching_on_0_reason_tag_inside_Effect_1_hand_rolls_reason_dispatch_use_Effect_catchReason_or_Effect_catchReasons_which_re_fail_unmatched_reasons_automatically_effect_catchTagToCatchReason.Code(),
	},
	FixIDs: []string{"catchTagToCatchReason_fix"},
	Run:    runCatchTagToCatchReasonFix,
}

func runCatchTagToCatchReasonFix(ctx *fixable.Context) []ls.CodeAction {
	matches := rules.AnalyzeCatchTagToCatchReason(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
	for _, match := range matches {
		if !match.CanFix || (!match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location)) {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.catchReason or Effect.catchReasons",
			Run: func(tracker *rewriter.Tracker) {
				replacement := buildCatchTagToCatchReasonReplacement(tracker, match)
				if replacement == nil {
					return
				}
				ast.SetParentInChildren(replacement)
				tracker.ReplaceNode(ctx.SourceFile, match.CallNode, replacement, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}

func buildCatchTagToCatchReasonReplacement(tracker *rewriter.Tracker, match rules.CatchTagToCatchReasonMatch) *ast.Node {
	if tracker == nil || match.Callee == nil || match.Callee.Kind != ast.KindPropertyAccessExpression || match.OuterTag == nil || len(match.Branches) == 0 {
		return nil
	}
	callee := match.Callee.AsPropertyAccessExpression()
	if callee == nil || callee.Expression == nil {
		return nil
	}

	methodName := "catchReason"
	if len(match.Branches) > 1 {
		methodName = "catchReasons"
	}
	method := tracker.NewPropertyAccessExpression(
		tracker.DeepCloneNode(callee.Expression),
		nil,
		tracker.NewIdentifier(methodName),
		ast.NodeFlagsNone,
	)
	arguments := []*ast.Node{tracker.DeepCloneNode(match.OuterTag)}
	if len(match.Branches) == 1 {
		branch := match.Branches[0]
		arguments = append(arguments, tracker.NewStringLiteral(branch.ReasonTag, 0), newCatchReasonHandler(tracker, match.ParameterName, branch))
	} else {
		properties := make([]*ast.Node, 0, len(match.Branches))
		for _, branch := range match.Branches {
			properties = append(properties, tracker.NewPropertyAssignment(
				nil,
				tracker.NewStringLiteral(branch.ReasonTag, 0),
				nil,
				nil,
				newCatchReasonHandler(tracker, match.ParameterName, branch),
			))
		}
		arguments = append(arguments, tracker.NewObjectLiteralExpression(tracker.NewNodeList(properties), false))
	}

	return tracker.NewCallExpression(method, nil, nil, tracker.NewNodeList(arguments), ast.NodeFlagsNone)
}

func newCatchReasonHandler(tracker *rewriter.Tracker, parameterName string, branch rules.CatchTagToCatchReasonBranch) *ast.Node {
	var parameters []*ast.Node
	if branch.UsesParameter {
		parameters = []*ast.Node{
			tracker.NewParameterDeclaration(nil, nil, tracker.NewIdentifier(branch.ReasonParameterName), nil, nil, nil),
			tracker.NewParameterDeclaration(nil, nil, tracker.NewIdentifier(parameterName), nil, nil, nil),
		}
	}
	return tracker.NewArrowFunction(
		nil,
		nil,
		tracker.NewNodeList(parameters),
		nil,
		nil,
		tracker.NewToken(ast.KindEqualsGreaterThanToken),
		tracker.DeepCloneNode(branch.Result),
	)
}
