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
	Flow            *typeparser.PipingFlow
	Index           int
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
			if callee == nil || transformation.Args().Len() != 1 {
				continue
			}
			options := transformation.Args().At(0)
			successValue, ok := booleanLiteralHandler(typeparser.ObjectLiteralPropertyInitializer(options, "onSuccess"))
			if !ok {
				continue
			}
			failureValue, ok := booleanLiteralHandler(typeparser.ObjectLiteralPropertyInitializer(options, "onFailure"))
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
				MatchName:       matchName,
				ReplacementName: replacementName,
			})
		}
	}
	return matches
}

// booleanLiteralHandler accepts a lazy expression (see ParseLazyExpression)
// that ignores its input and returns a bare (possibly parenthesized) boolean
// literal.
func booleanLiteralHandler(handler *ast.Node) (value bool, ok bool) {
	if handler == nil {
		return false, false
	}
	lazy := typeparser.ParseLazyExpression(handler, typeparser.LazyExpressionNone)
	if lazy == nil || !isIgnorableMapParameterList(lazy.Params()) {
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
