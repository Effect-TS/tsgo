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

var IsTaggedOnTaggedUnionFix = fixable.Fixable{
	Name:        "isTaggedOnTaggedUnion",
	Description: "Replace Predicate.isTagged with a _tag comparison or the data type's guard",
	ErrorCodes: []int32{
		tsdiag.Predicate_isTagged_accepts_any_value_so_on_this_tagged_union_it_neither_checks_the_tag_against_the_members_nor_narrows_as_a_tag_comparison_does_effect_isTaggedOnTaggedUnion.Code(),
	},
	FixIDs: []string{"isTaggedOnTaggedUnion_fix"},
	Run:    runIsTaggedOnTaggedUnionFix,
}

func runIsTaggedOnTaggedUnionFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeIsTaggedOnTaggedUnion(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		// A tag outside the union has no replacement that keeps the meaning.
		if !match.KnownTag {
			continue
		}
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		input := match.Flow.TransformationInputNode(match.Index)

		var guard *moduleMemberReference
		var member moduleMember
		if match.Module != "" {
			member = taggedDataTypeGuard(ctx, match.Module, "is"+match.Tag)
			callee := match.Flow.Transformations().At(match.Index).Callee()
			guard = resolveModuleMember(ctx, callee, typeparser.FindModuleReference(match.SourceFile, match.Module), member)
		}
		description := "Replace with a _tag comparison"
		if guard != nil {
			description = "Replace with " + match.Module + "." + member.name
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: description,
			Run: func(tracker *rewriter.Tracker) {
				switch {
				case match.Applied && input != nil && guard != nil:
					call := tracker.NewCallExpression(guard.expression(tracker, member), nil, nil,
						tracker.NewNodeList([]*ast.Node{tracker.DeepCloneNode(input)}), ast.NodeFlagsNone)
					tracker.ReplacePipingFlowPrefix(match.SourceFile, match.Flow, match.Index+1, call)
				case match.Applied && input != nil:
					replaceIsTaggedWithTagComparison(tracker, match, input)
				case match.Applied:
					// A pipe step after the first has no input node, so the predicate becomes the step.
					transformation := match.Flow.Transformations().At(match.Index)
					tracker.ReplacePipingFlowTransformation(match.SourceFile, &transformation,
						rewriter.PipingFlowTransformationReplacement{Callee: isTaggedPredicate(tracker, guard, member, match.Tag)})
				default:
					predicate := isTaggedPredicate(tracker, guard, member, match.Tag)
					// A step followed by others becomes the argument of the next one.
					if guard == nil && match.Index == match.Flow.Transformations().Len()-1 && !positionAcceptsAnyExpression(match.Flow.Node()) {
						predicate = tracker.NewParenthesizedExpression(predicate)
					}
					tracker.ReplacePipingFlowPrefix(match.SourceFile, match.Flow, match.Index+1, predicate)
				}
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}

// isTaggedPredicate is the refinement that stands for isTagged(tag): the
// data type's guard, or value => value._tag === tag.
func isTaggedPredicate(tracker *rewriter.Tracker, guard *moduleMemberReference, member moduleMember, tag string) *ast.Node {
	if guard != nil {
		return guard.expression(tracker, member)
	}
	parameter := tracker.NewParameterDeclaration(nil, nil, tracker.NewIdentifier("value"), nil, nil, nil)
	return tracker.NewArrowFunction(nil, nil, tracker.NewNodeList([]*ast.Node{parameter}),
		nil, nil, tracker.NewToken(ast.KindEqualsGreaterThanToken),
		tagComparison(tracker, tracker.NewIdentifier("value"), ast.KindEqualsEqualsEqualsToken, tag))
}

// replaceIsTaggedWithTagComparison writes input._tag === tag in place of an
// applied isTagged, or input._tag !== tag in place of its negation. The `!`
// is not part of the flow, so a negated test is replaced as a node.
func replaceIsTaggedWithTagComparison(tracker *rewriter.Tracker, match rules.IsTaggedOnTaggedUnionMatch, input *ast.Node) {
	object := tracker.DeepCloneNode(input)
	if !ast.IsLeftHandSideExpression(input) {
		object = tracker.NewParenthesizedExpression(object)
	}
	last := match.Index == match.Flow.Transformations().Len()-1
	position := match.Flow.Node()
	if parent := position.Parent; last && parent != nil && parent.Kind == ast.KindPrefixUnaryExpression &&
		parent.AsPrefixUnaryExpression().Operator == ast.KindExclamationToken {
		comparison := tagComparison(tracker, object, ast.KindExclamationEqualsEqualsToken, match.Tag)
		if tagComparisonNeedsParentheses(parent) {
			comparison = tracker.NewParenthesizedExpression(comparison)
		}
		tracker.ReplaceNode(match.SourceFile, parent, comparison, nil)
		return
	}
	comparison := tagComparison(tracker, object, ast.KindEqualsEqualsEqualsToken, match.Tag)
	// A step followed by others becomes the argument of the next one.
	if last && tagComparisonNeedsParentheses(position) {
		comparison = tracker.NewParenthesizedExpression(comparison)
	}
	tracker.ReplacePipingFlowPrefix(match.SourceFile, match.Flow, match.Index+1, comparison)
}

func tagComparison(tracker *rewriter.Tracker, object *ast.Node, operator ast.Kind, tag string) *ast.Node {
	access := tracker.NewPropertyAccessExpression(object, nil, tracker.NewIdentifier("_tag"), ast.NodeFlagsNone)
	return tracker.NewBinaryExpression(nil, access, nil, tracker.NewToken(operator), tracker.NewStringLiteral(tag, 0))
}

// tagComparisonNeedsParentheses reports whether an equality expression at
// position binds less tightly than the expression it is an operand of.
func tagComparisonNeedsParentheses(position *ast.Node) bool {
	parent := position.Parent
	switch {
	case parent == nil || positionAcceptsAnyExpression(position):
		return false
	case parent.Kind == ast.KindBinaryExpression:
		return ast.GetBinaryOperatorPrecedence(parent.AsBinaryExpression().OperatorToken.Kind) >= ast.OperatorPrecedenceEquality
	case ast.IsExpression(parent):
		return ast.GetExpressionPrecedence(parent) > ast.OperatorPrecedenceEquality
	}
	return false
}

// taggedDataTypeGuard is the is<Tag> guard exported by one of the modules
// TypeParser.TaggedDataTypeModule returns.
func taggedDataTypeGuard(ctx *fixable.Context, module string, name string) moduleMember {
	tp := ctx.TypeParser
	switch module {
	case "Option":
		return moduleMember{name: name, isModule: tp.IsExpressionEffectOptionModule,
			isMember: func(node *ast.Node) bool { return tp.IsNodeReferenceToEffectOptionModuleApi(node, name) }}
	case "Exit":
		return moduleMember{name: name, isModule: tp.IsExpressionEffectExitModule,
			isMember: func(node *ast.Node) bool { return tp.IsNodeReferenceToEffectExitModuleApi(node, name) }}
	case "Result":
		return moduleMember{name: name, isModule: tp.IsExpressionEffectResultModule,
			isMember: func(node *ast.Node) bool { return tp.IsNodeReferenceToEffectResultModuleApi(node, name) }}
	default:
		return moduleMember{name: name, isModule: tp.IsExpressionEffectEitherModule,
			isMember: func(node *ast.Node) bool { return tp.IsNodeReferenceToEffectEitherModuleApi(node, name) }}
	}
}
