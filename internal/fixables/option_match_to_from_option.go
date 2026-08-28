package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var OptionMatchToFromOptionFix = fixable.Fixable{
	Name:        "optionMatchToFromOption",
	Description: "Replace Option.match or an Option tag conditional with Effect.fromOption",
	ErrorCodes: []int32{
		tsdiag.Effect_fromOption_expresses_this_Option_to_Effect_conversion_more_directly_than_Option_match_or_an_Option_tag_conditional_effect_optionMatchToFromOption.Code(),
	},
	FixIDs: []string{"optionMatchToFromOption_fix"},
	Run:    runOptionMatchToFromOptionFix,
}

func runOptionMatchToFromOptionFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeOptionMatchToFromOption(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if !match.CanFix || match.EffectModuleNode == nil || match.ReplacementNode == nil || !match.Pipeable && match.OptionNode == nil {
			return nil
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.fromOption",
			Run: func(tracker *rewriter.Tracker) {
				replacement := buildFromOptionReplacement(tracker, match)
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

func buildFromOptionReplacement(tracker *rewriter.Tracker, match rules.OptionMatchToFromOptionMatch) *ast.Node {
	fromOption := tracker.NewPropertyAccessExpression(
		tracker.DeepCloneNode(match.EffectModuleNode),
		nil,
		tracker.NewIdentifier("fromOption"),
		ast.NodeFlagsNone,
	)

	if match.Pipeable && match.DefaultFailure {
		return fromOption
	}

	var arguments []*ast.Node
	if !match.Pipeable {
		arguments = append(arguments, tracker.DeepCloneNode(match.OptionNode))
	}
	if !match.DefaultFailure {
		arguments = append(arguments, tracker.NewArrowFunction(
			nil,
			nil,
			tracker.NewNodeList(nil),
			nil,
			nil,
			tracker.NewToken(ast.KindEqualsGreaterThanToken),
			tracker.DeepCloneNode(match.FailureNode),
		))
	}

	return tracker.NewCallExpression(fromOption, nil, nil, tracker.NewNodeList(arguments), ast.NodeFlagsNone)
}
