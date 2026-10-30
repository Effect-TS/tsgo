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

// MatchBooleanToIsSuccess suggests Effect.isSuccess or Effect.isFailure when
// Effect.match or Effect.matchEager only folds the result to complementary
// boolean literals. Both keep defects and interruptions propagating, unlike the
// matchCause variants or Effect.exit, which are not reported.
var MatchBooleanToIsSuccess = rule.Rule{
	Name:            "matchBooleanToIsSuccess",
	Group:           "style",
	Description:     "Suggests Effect.isSuccess or Effect.isFailure when Effect.match or Effect.matchEager folds to complementary boolean literals",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_isSuccess_expresses_this_check_more_directly_than_Effect_0_folding_to_complementary_boolean_literals_effect_matchBooleanToIsSuccess.Code(),
		tsdiag.Effect_isFailure_expresses_this_check_more_directly_than_Effect_0_folding_to_complementary_boolean_literals_effect_matchBooleanToIsSuccess.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeMatchBooleanToIsSuccess(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			message := tsdiag.Effect_isSuccess_expresses_this_check_more_directly_than_Effect_0_folding_to_complementary_boolean_literals_effect_matchBooleanToIsSuccess
			if match.ReplacementName == "isFailure" {
				message = tsdiag.Effect_isFailure_expresses_this_check_more_directly_than_Effect_0_folding_to_complementary_boolean_literals_effect_matchBooleanToIsSuccess
			}
			diagnostics[i] = ctx.NewDiagnostic(match.SourceFile, match.Location, message, nil, match.MatchName)
		}
		return diagnostics
	},
}

// MatchBooleanToIsSuccessMatch holds the nodes needed by the diagnostic and quick fix.
type MatchBooleanToIsSuccessMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	Callee     *ast.Node
	// Flow contains the match step at Index.
	Flow  *typeparser.PipingFlow
	Index int
	// PointFree is true for a standalone match({...}) call: the options are the
	// subject of Flow and the call is its first step, applied to no input.
	PointFree       bool
	MatchName       string // "match" or "matchEager"
	ReplacementName string // "isSuccess" or "isFailure"
}

// AnalyzeMatchBooleanToIsSuccess finds Effect.match and Effect.matchEager
// calls whose inline handlers ignore their input and return complementary
// boolean literals.
func AnalyzeMatchBooleanToIsSuccess(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []MatchBooleanToIsSuccessMatch {
	if tp == nil || sf == nil {
		return nil
	}
	var matches []MatchBooleanToIsSuccessMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for index := range flow.Transformations().All() {
			transformation := flow.Transformations().At(index)
			callee := transformation.Callee()
			if callee == nil {
				continue
			}

			var options *ast.Node
			pointFree := false
			switch {
			case transformation.Args().Len() == 1:
				options = transformation.Args().At(0)
			case index == 0 && transformation.Kind() == typeparser.TransformationKindCall && transformation.Args().Len() == 0:
				// A standalone match({...}): the options are the subject of the flow.
				options = flow.Subject().Node()
				pointFree = true
			default:
				continue
			}

			onSuccess, onFailure, ok := matchBooleanHandlers(options)
			if !ok {
				continue
			}
			successValue, ok := booleanLiteralHandler(onSuccess)
			if !ok {
				continue
			}
			failureValue, ok := booleanLiteralHandler(onFailure)
			if !ok || successValue == failureValue {
				continue
			}

			matchName := ""
			switch {
			case tp.IsNodeReferenceToEffectModuleApi(callee, "match"):
				matchName = "match"
			case tp.IsNodeReferenceToEffectModuleApi(callee, "matchEager"):
				matchName = "matchEager"
			default:
				continue
			}
			replacementName := "isFailure"
			if successValue {
				replacementName = "isSuccess"
			}

			matches = append(matches, MatchBooleanToIsSuccessMatch{
				SourceFile:      sf,
				Location:        scanner.GetErrorRangeForNode(sf, callee),
				Callee:          callee,
				Flow:            flow,
				Index:           index,
				PointFree:       pointFree,
				MatchName:       matchName,
				ReplacementName: replacementName,
			})
		}
	}
	return matches
}

// matchBooleanHandlers returns the onSuccess and onFailure initializers of an
// inline object literal made of exactly those two property assignments.
func matchBooleanHandlers(options *ast.Node) (onSuccess *ast.Node, onFailure *ast.Node, ok bool) {
	object := ast.SkipParentheses(options)
	if object == nil || object.Kind != ast.KindObjectLiteralExpression {
		return nil, nil, false
	}
	properties := object.AsObjectLiteralExpression().Properties
	if properties == nil || len(properties.Nodes) != 2 {
		return nil, nil, false
	}
	for _, property := range properties.Nodes {
		if property.Kind != ast.KindPropertyAssignment {
			return nil, nil, false
		}
	}
	onSuccess = typeparser.ObjectLiteralPropertyInitializer(object, "onSuccess")
	onFailure = typeparser.ObjectLiteralPropertyInitializer(object, "onFailure")
	return onSuccess, onFailure, onSuccess != nil && onFailure != nil
}

// booleanLiteralHandler accepts an expression-bodied arrow function that
// ignores its input and returns a bare (possibly parenthesized) boolean literal.
func booleanLiteralHandler(handler *ast.Node) (value bool, ok bool) {
	if handler == nil || ast.SkipParentheses(handler).Kind != ast.KindArrowFunction {
		return false, false
	}
	lazy := typeparser.ParseLazyExpression(handler, typeparser.LazyExpressionNone)
	if lazy == nil || lazy.Body() == nil || lazy.Body().Kind == ast.KindBlock || lazy.Expression() == nil ||
		!isIgnorableMapParameterList(lazy.Params()) {
		return false, false
	}
	switch ast.SkipParentheses(lazy.Expression()).Kind {
	case ast.KindTrueKeyword:
		return true, true
	case ast.KindFalseKeyword:
		return false, true
	}
	return false, false
}
