package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var CatchAllTagDispatchToCatchTagFix = fixable.Fixable{
	Name: "catchAllTagDispatchToCatchTag", Description: "Replace hand-rolled tagged error dispatch with Effect.catchTag or Effect.catchTags",
	ErrorCodes: []int32{tsdiag.Branching_on_0_tag_inside_Effect_1_hand_rolls_tagged_error_dispatch_use_Effect_catchTag_or_Effect_catchTags_which_re_fail_unmatched_errors_automatically_effect_catchAllTagDispatchToCatchTag.Code()},
	FixIDs:     []string{"catchAllTagDispatchToCatchTag_fix"}, Run: runCatchAllTagDispatchToCatchTagFix,
}

func runCatchAllTagDispatchToCatchTagFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeCatchAllTagDispatchToCatchTag(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		// The new handlers rebind the parameter per branch, which only holds for
		// an arrow function whose parameter is never reassigned.
		if match.Handler == nil || match.Handler.Kind != ast.KindArrowFunction || checker.Checker_isSymbolAssigned(ctx.Checker, match.ParameterSymbol) {
			continue
		}
		action := ctx.NewFixAction(fixable.FixAction{Description: "Replace with Effect.catchTag or Effect.catchTags", Run: func(tracker *rewriter.Tracker) {
			callee, arguments := buildCatchAllTagDispatchReplacement(tracker, ctx, match)
			if callee != nil && arguments != nil {
				tracker.ReplacePipingFlowTransformation(ctx.SourceFile, match.Transformation, rewriter.PipingFlowTransformationReplacement{Callee: callee, Arguments: arguments})
			}
		}})
		if action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}

func buildCatchAllTagDispatchReplacement(tracker *rewriter.Tracker, ctx *fixable.Context, match rules.CatchAllTagDispatchMatch) (*ast.Node, *ast.NodeList) {
	if tracker == nil || match.Callee == nil || len(match.Branches) == 0 {
		return nil, nil
	}
	methodName := "catchTag"
	if len(match.Branches) > 1 {
		methodName = "catchTags"
	}
	callee := tracker.NewPropertyAccessPath(typeparser.FindEffectModuleReference(ctx.SourceFile).Append(methodName)...)
	if len(match.Branches) == 1 {
		branch := match.Branches[0]
		return callee, tracker.NewNodeList([]*ast.Node{tracker.NewStringLiteral(branch.Tag, 0), newCatchTagHandler(tracker, ctx, match, branch)})
	}
	properties := make([]*ast.Node, 0, len(match.Branches))
	for _, branch := range match.Branches {
		name := tracker.NewStringLiteral(branch.Tag, 0)
		if scanner.IsIdentifierText(branch.Tag, core.LanguageVariantStandard) {
			name = tracker.NewIdentifier(branch.Tag)
		}
		properties = append(properties, tracker.NewPropertyAssignment(nil, name, nil, nil, newCatchTagHandler(tracker, ctx, match, branch)))
	}
	return callee, tracker.NewNodeList([]*ast.Node{tracker.NewObjectLiteralExpression(tracker.NewNodeList(properties), false)})
}

// newCatchTagHandler builds a branch handler that keeps the error parameter
// only when the branch result reads it.
func newCatchTagHandler(tracker *rewriter.Tracker, ctx *fixable.Context, match rules.CatchAllTagDispatchMatch, branch rules.CatchAllTagDispatchBranch) *ast.Node {
	var parameters []*ast.Node
	if rules.ConditionalRefailNodeContainsParameter(ctx.TypeParser, ctx.Checker, branch.Result, match.ParameterSymbol) {
		parameters = []*ast.Node{tracker.NewParameterDeclaration(nil, nil, tracker.NewIdentifier(match.ParameterName), nil, nil, nil)}
	}
	return tracker.NewArrowFunction(nil, nil, tracker.NewNodeList(parameters), nil, nil, tracker.NewToken(ast.KindEqualsGreaterThanToken), tracker.DeepCloneNode(branch.Result))
}
