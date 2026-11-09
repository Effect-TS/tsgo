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

var OnExitFailureToOnError = rule.Rule{
	Name:            "onExitFailureToOnError",
	Group:           "style",
	Description:     "Suggests Effect.onError when an Effect.onExit callback only acts on failure",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_onError_expresses_this_failure_only_Effect_onExit_callback_more_directly_effect_onExitFailureToOnError.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeOnExitFailureToOnError(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_onError_expresses_this_failure_only_Effect_onExit_callback_more_directly_effect_onExitFailureToOnError,
				nil,
			)
		}
		return diagnostics
	},
}

type OnExitFailureToOnErrorMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
}

func AnalyzeOnExitFailureToOnError(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []OnExitFailureToOnErrorMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}

	var matches []OnExitFailureToOnErrorMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		sequences := flow.FindTransformationSequences(func(t typeparser.PipingFlowTransformation) bool {
			if t.Callee() == nil || t.Args().Len() != 1 {
				return false
			}
			callback := ast.SkipParentheses(t.Args().At(0))
			if callback == nil || (callback.Kind != ast.KindArrowFunction && callback.Kind != ast.KindFunctionExpression) {
				return false
			}
			return tp.IsNodeReferenceToEffectModuleApi(t.Callee(), "onExit")
		})
		for _, sequence := range sequences.All() {
			transformation := sequence.Transformations().At(0)
			if match, ok := analyzeOnExitFailureCallback(tp, c, sf, transformation); ok {
				matches = append(matches, match)
			}
		}
	}
	return matches
}

func analyzeOnExitFailureCallback(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, transformation typeparser.PipingFlowTransformation) (OnExitFailureToOnErrorMatch, bool) {
	callback := ast.SkipParentheses(transformation.Args().At(0))
	if !isSynchronousFunction(callback) {
		return OnExitFailureToOnErrorMatch{}, false
	}
	parsed := tp.ParseReturningDispatchAllowingTypeParameters(callback)
	if parsed == nil || parsed.Params().Len() != 1 {
		return OnExitFailureToOnErrorMatch{}, false
	}
	dispatch := parsed.Dispatch()
	if dispatch.Branches().Len() != 1 || dispatch.Fallback() == nil {
		return OnExitFailureToOnErrorMatch{}, false
	}

	parameter := parsed.Params().At(0)
	if parameter == nil || parameter.Name() == nil || parameter.Name().Kind != ast.KindIdentifier {
		return OnExitFailureToOnErrorMatch{}, false
	}
	declaration := parameter.AsParameterDeclaration()
	if declaration == nil || declaration.DotDotDotToken != nil || declaration.Initializer != nil {
		return OnExitFailureToOnErrorMatch{}, false
	}
	parameterSymbol := tp.GetSymbolAtLocation(parameter.Name())
	if parameterSymbol == nil || checker.Checker_isSymbolAssigned(c, parameterSymbol) {
		return OnExitFailureToOnErrorMatch{}, false
	}

	branch := dispatch.Branches().At(0)
	failureWhenTrue, ok := onExitConditionChecksFailure(tp, c, branch.Condition(), parameterSymbol)
	if !ok {
		return OnExitFailureToOnErrorMatch{}, false
	}

	noop := dispatch.Fallback()
	if !failureWhenTrue {
		noop = branch.Result()
	}
	if !isEffectVoidReference(tp, noop) {
		return OnExitFailureToOnErrorMatch{}, false
	}

	return OnExitFailureToOnErrorMatch{
		SourceFile: sf,
		Location:   scanner.GetErrorRangeForNode(sf, transformation.Callee()),
	}, true
}

func onExitConditionChecksFailure(tp *typeparser.TypeParser, c *checker.Checker, condition typeparser.DispatchCondition, parameterSymbol *ast.Symbol) (failureWhenTrue bool, ok bool) {
	switch condition.Kind() {
	case typeparser.DispatchConditionSwitchCase:
		if !isBareParameterTagReference(tp, c, condition.TagSubject(), parameterSymbol) {
			return false, false
		}
		value, ok := resultDispatchTagValue(condition)
		if !ok {
			return false, false
		}
		return exitTagIsFailure(value)
	case typeparser.DispatchConditionPredicate:
		return decodeOnExitFailurePredicate(tp, c, condition.Subject(), parameterSymbol, false)
	default:
		return false, false
	}
}

func decodeOnExitFailurePredicate(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node, parameterSymbol *ast.Symbol, negated bool) (bool, bool) {
	predicate := unwrapTransparentExpression(node)
	if predicate == nil {
		return false, false
	}

	switch predicate.Kind {
	case ast.KindPrefixUnaryExpression:
		unary := predicate.AsPrefixUnaryExpression()
		if unary == nil || unary.Operator != ast.KindExclamationToken {
			return false, false
		}
		return decodeOnExitFailurePredicate(tp, c, unary.Operand, parameterSymbol, !negated)

	case ast.KindBinaryExpression:
		binary := predicate.AsBinaryExpression()
		if binary == nil || binary.OperatorToken == nil || binary.OperatorToken.Kind != ast.KindEqualsEqualsEqualsToken ||
			binary.Left == nil || binary.Right == nil {
			return false, false
		}
		switch unwrapTransparentExpression(binary.Right).Kind {
		case ast.KindTrueKeyword:
			return decodeOnExitFailurePredicate(tp, c, binary.Left, parameterSymbol, negated)
		case ast.KindFalseKeyword:
			return decodeOnExitFailurePredicate(tp, c, binary.Left, parameterSymbol, !negated)
		}
		subject, value := tp.ParseTagMatch(predicate)
		value = unwrapTransparentExpression(value)
		if subject == nil || value == nil || !ast.IsStringLiteral(value) ||
			!isBareParameterTagReference(tp, c, subject, parameterSymbol) {
			return false, false
		}
		isFailure, ok := exitTagIsFailure(value.AsStringLiteral().Text)
		if !ok {
			return false, false
		}
		return isFailure != negated, true

	case ast.KindCallExpression:
		flow := tp.LongestPipingFlowAt(predicate, false)
		if flow == nil {
			return false, false
		}
		isFailure := false
		matched := flow.MatchesExactly(
			func(subject typeparser.PipingFlowSubject) bool {
				node := unwrapTransparentExpression(subject.Node())
				return node != nil && node.Kind == ast.KindIdentifier && sameCatchReasonSymbol(tp, c, node, parameterSymbol)
			},
			func(t typeparser.PipingFlowTransformation) bool {
				if t.Callee() == nil || t.Args().Len() != 0 {
					return false
				}
				if tp.IsNodeReferenceToEffectExitModuleApi(t.Callee(), "isFailure") {
					isFailure = true
					return true
				}
				return tp.IsNodeReferenceToEffectExitModuleApi(t.Callee(), "isSuccess")
			},
		)
		if !matched {
			return false, false
		}
		return isFailure != negated, true
	}
	return false, false
}

func exitTagIsFailure(tag string) (bool, bool) {
	switch tag {
	case "Failure":
		return true, true
	case "Success":
		return false, true
	default:
		return false, false
	}
}
