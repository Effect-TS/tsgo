package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// ResultMatchToFromResult suggests Effect.fromResult when Result.match or a
// Result guard conditional only lifts Success with Effect.succeed and Failure
// with Effect.fail.
var ResultMatchToFromResult = rule.Rule{
	Name:            "resultMatchToFromResult",
	Group:           "style",
	Description:     "Suggests Effect.fromResult when Result.match or a Result guard conditional only converts Success to Effect.succeed and Failure to Effect.fail",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.Effect_fromResult_expresses_this_Result_to_Effect_conversion_more_directly_than_Result_match_or_a_Result_guard_conditional_effect_resultMatchToFromResult.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeResultMatchToFromResult(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_fromResult_expresses_this_Result_to_Effect_conversion_more_directly_than_Result_match_or_a_Result_guard_conditional_effect_resultMatchToFromResult,
				nil,
			)
		}
		return diagnostics
	},
}

// ResultMatchToFromResultMatch holds the nodes needed by the diagnostic and
// quick fix. Result.match steps set Transformation; guard conditionals set
// ReplacementNode and ResultNode.
type ResultMatchToFromResultMatch struct {
	SourceFile      *ast.SourceFile
	Location        core.TextRange
	Transformation  *typeparser.PipingFlowTransformation
	ReplacementNode *ast.Node
	ResultNode      *ast.Node
}

// AnalyzeResultMatchToFromResult finds Result.match steps and Result guard
// conditionals that are equivalent to Effect.fromResult.
func AnalyzeResultMatchToFromResult(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []ResultMatchToFromResultMatch {
	if tp == nil || c == nil || sf == nil || tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}

	var matches []ResultMatchToFromResultMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for index := range flow.Transformations().All() {
			if match, ok := analyzeResultMatchStep(tp, sf, flow.Transformations().At(index)); ok {
				matches = append(matches, match)
			}
		}
		if match, ok := analyzeResultGuardConditional(tp, c, sf, flow); ok {
			matches = append(matches, match)
		}
	}
	return matches
}

// analyzeResultMatchStep matches a Result.match step whose handlers only lift
// the failure with Effect.fail and the success with Effect.succeed.
func analyzeResultMatchStep(tp *typeparser.TypeParser, sf *ast.SourceFile, transformation typeparser.PipingFlowTransformation) (ResultMatchToFromResultMatch, bool) {
	callee := transformation.Callee()
	if callee == nil || transformation.Args().Len() != 1 || !hasNoTypeArguments(transformation) {
		return ResultMatchToFromResultMatch{}, false
	}
	handlers := ast.SkipParentheses(transformation.Args().At(0))
	onFailure := typeparser.ObjectLiteralPropertyInitializer(handlers, "onFailure")
	onSuccess := typeparser.ObjectLiteralPropertyInitializer(handlers, "onSuccess")
	if onFailure == nil || onSuccess == nil || len(handlers.AsObjectLiteralExpression().Properties.Nodes) != 2 ||
		!tp.IsNodeReferenceToEffectResultModuleApi(callee, "match") ||
		!isEffectLiftHandler(tp, onSuccess, "succeed") ||
		!isEffectLiftHandler(tp, onFailure, "fail") {
		return ResultMatchToFromResultMatch{}, false
	}

	return ResultMatchToFromResultMatch{
		SourceFile:     sf,
		Location:       scanner.GetErrorRangeForNode(sf, callee),
		Transformation: &transformation,
	}, true
}

func hasNoTypeArguments(transformation typeparser.PipingFlowTransformation) bool {
	return transformation.TypeArguments() == nil || len(transformation.TypeArguments().Nodes) == 0
}

// isEffectLiftStep reports whether a step applies Effect.<api> to its input
// without extra or type arguments.
func isEffectLiftStep(tp *typeparser.TypeParser, api string) func(typeparser.PipingFlowTransformation) bool {
	return func(transformation typeparser.PipingFlowTransformation) bool {
		return transformation.Callee() != nil && transformation.Args().Len() == 0 && hasNoTypeArguments(transformation) &&
			tp.IsNodeReferenceToEffectModuleApi(transformation.Callee(), api)
	}
}

// isEffectLiftHandler reports whether a Result.match handler is Effect.<api>,
// directly or through an identity forwarder, without type arguments.
func isEffectLiftHandler(tp *typeparser.TypeParser, node *ast.Node, api string) bool {
	target, typeArguments, _ := tp.UnwrapIdentityForwarder(node)
	return (typeArguments == nil || len(typeArguments.Nodes) == 0) && tp.IsNodeReferenceToEffectModuleApi(target, api)
}

// analyzeResultGuardConditional matches a flow that applies Result.isSuccess or
// Result.isFailure to an identifier and is the condition of a conditional
// whose branches lift that identifier's .success with Effect.succeed and its
// .failure with Effect.fail.
func analyzeResultGuardConditional(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, flow *typeparser.PipingFlow) (ResultMatchToFromResultMatch, bool) {
	isFailure := false
	isGuard := func(transformation typeparser.PipingFlowTransformation) bool {
		if transformation.Callee() == nil || transformation.Args().Len() != 0 {
			return false
		}
		if tp.IsNodeReferenceToEffectResultModuleApi(transformation.Callee(), "isSuccess") {
			return true
		}
		isFailure = tp.IsNodeReferenceToEffectResultModuleApi(transformation.Callee(), "isFailure")
		return isFailure
	}
	isIdentifier := func(subject typeparser.PipingFlowSubject) bool {
		return subject.Node() != nil && subject.Node().Kind == ast.KindIdentifier
	}
	if !flow.MatchesExactly(isIdentifier, isGuard) {
		return ResultMatchToFromResultMatch{}, false
	}

	// The conditional is not a call chain, so it is read from the syntax tree.
	condition := flow.Node()
	if condition.Parent == nil || condition.Parent.Kind != ast.KindConditionalExpression ||
		condition.Parent.AsConditionalExpression().Condition != condition {
		return ResultMatchToFromResultMatch{}, false
	}
	conditional := condition.Parent.AsConditionalExpression()

	resultNode := flow.Subject().Node()
	succeedNode, failNode := conditional.WhenTrue, conditional.WhenFalse
	if isFailure {
		succeedNode, failNode = failNode, succeedNode
	}
	if !isResultMemberLift(tp, c, resultNode, succeedNode, "succeed", "success") ||
		!isResultMemberLift(tp, c, resultNode, failNode, "fail", "failure") {
		return ResultMatchToFromResultMatch{}, false
	}

	return ResultMatchToFromResultMatch{
		SourceFile:      sf,
		Location:        scanner.GetErrorRangeForNode(sf, flow.Transformations().At(0).Callee()),
		ReplacementNode: conditional.AsNode(),
		ResultNode:      resultNode,
	}, true
}

// isResultMemberLift reports whether node is a flow that only applies
// Effect.<api> to resultNode.<property>.
func isResultMemberLift(tp *typeparser.TypeParser, c *checker.Checker, resultNode *ast.Node, node *ast.Node, api string, property string) bool {
	flow := tp.LongestPipingFlowAt(node, false)
	return flow != nil && flow.MatchesExactly(func(subject typeparser.PipingFlowSubject) bool {
		value := ast.SkipParentheses(subject.Node())
		if value == nil || value.Kind != ast.KindPropertyAccessExpression {
			return false
		}
		access := value.AsPropertyAccessExpression()
		return access.QuestionDotToken == nil && access.Name() != nil && scanner.GetTextOfNode(access.Name()) == property &&
			sameIdentifierReference(tp, c, resultNode, ast.SkipParentheses(access.Expression))
	}, isEffectLiftStep(tp, api))
}
