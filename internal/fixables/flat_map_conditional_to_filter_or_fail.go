package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var FlatMapConditionalToFilterOrFailFix = fixable.Fixable{
	Name:        "flatMapConditionalToFilterOrFail",
	Description: "Replace a conditional Effect.flatMap identity filter with Effect.filterOrFail or Effect.filterOrElse",
	ErrorCodes: []int32{
		tsdiag.Effect_0_expresses_this_conditional_validation_more_directly_than_Effect_flatMap_with_an_identity_Effect_succeed_branch_effect_flatMapConditionalToFilterOrFail.Code(),
	},
	FixIDs: []string{"flatMapConditionalToFilterOrFail_fix"},
	Run:    runFlatMapConditionalToFilterOrFailFix,
}

func runFlatMapConditionalToFilterOrFailFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeFlatMapConditionalToFilterOrFail(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if !match.CanFix || match.ReplacementNode == nil || match.EffectModuleNode == nil ||
			match.ParameterNode == nil || match.PredicateNode == nil || match.FallbackNode == nil {
			return nil
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect." + match.PreferredMethodName,
			Run: func(tracker *rewriter.Tracker) {
				replacement := buildFlatMapConditionalFilterReplacement(tracker, match)
				if replacement == nil {
					return
				}
				ast.SetParentInChildren(replacement)
				tracker.ReplaceNode(ctx.SourceFile, match.ReplacementNode, replacement, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}

func buildFlatMapConditionalFilterReplacement(tracker *rewriter.Tracker, match rules.FlatMapConditionalToFilterOrFailMatch) *ast.Node {
	method := tracker.NewPropertyAccessExpression(
		tracker.DeepCloneNode(match.EffectModuleNode),
		nil,
		tracker.NewIdentifier(match.PreferredMethodName),
		ast.NodeFlagsNone,
	)
	predicateBody := tracker.DeepCloneNode(match.PredicateNode)
	if match.NegatePredicate {
		predicateBody = tracker.NewPrefixUnaryExpression(ast.KindExclamationToken, predicateBody)
	}
	parameter := tracker.DeepCloneNode(match.ParameterNode)
	predicate := tracker.NewArrowFunction(
		nil,
		nil,
		tracker.NewNodeList([]*ast.Node{parameter}),
		nil,
		nil,
		tracker.NewToken(ast.KindEqualsGreaterThanToken),
		predicateBody,
	)
	fallback := tracker.NewArrowFunction(
		nil,
		nil,
		tracker.NewNodeList([]*ast.Node{tracker.DeepCloneNode(match.ParameterNode)}),
		nil,
		nil,
		tracker.NewToken(ast.KindEqualsGreaterThanToken),
		tracker.DeepCloneNode(match.FallbackNode),
	)
	arguments := []*ast.Node{predicate, fallback}
	if match.SubjectNode != nil {
		arguments = append([]*ast.Node{tracker.DeepCloneNode(match.SubjectNode)}, arguments...)
	}
	return tracker.NewCallExpression(method, nil, nil, tracker.NewNodeList(arguments), ast.NodeFlagsNone)
}
