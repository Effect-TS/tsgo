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

// MatchEffectToMatch suggests the non-effectful match variant when both
// handlers only lift their result with Effect.succeed.
var MatchEffectToMatch = rule.Rule{
	Name:            "matchEffectToMatch",
	Group:           "style",
	Description:     "Suggests Effect.match or Effect.matchCause when both Effect.matchEffect handlers only return Effect.succeed",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_0_expresses_this_non_effectful_fold_more_directly_than_Effect_1_with_Effect_succeed_handlers_effect_matchEffectToMatch.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeMatchEffectToMatch(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(match.SourceFile, match.Location,
				tsdiag.Effect_0_expresses_this_non_effectful_fold_more_directly_than_Effect_1_with_Effect_succeed_handlers_effect_matchEffectToMatch,
				nil, match.ReplacementName, match.MatchEffectName)
		}
		return diagnostics
	},
}

type MatchEffectToMatchMatch struct {
	SourceFile       *ast.SourceFile
	Location         core.TextRange
	CalleeNode       *ast.Node
	MatchEffectName  string
	ReplacementName  string
	HandlerResults   [2]*ast.Node
	SucceedArguments [2]*ast.Node
}

func AnalyzeMatchEffectToMatch(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []MatchEffectToMatchMatch {
	if tp == nil || sf == nil {
		return nil
	}
	var matches []MatchEffectToMatchMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for index := range flow.Transformations().All() {
			transformation := flow.Transformations().At(index)
			if transformation.Args().Len() != 1 || transformation.Callee() == nil {
				continue
			}
			onFailure := typeparser.ObjectLiteralPropertyInitializer(transformation.Args().At(0), "onFailure")
			onSuccess := typeparser.ObjectLiteralPropertyInitializer(transformation.Args().At(0), "onSuccess")
			if onFailure == nil || onSuccess == nil {
				continue
			}
			matchEffectName, replacementName := "", ""
			switch {
			case tp.IsNodeReferenceToEffectModuleApi(transformation.Callee(), "matchEffect"):
				matchEffectName, replacementName = "matchEffect", "match"
			case tp.IsNodeReferenceToEffectModuleApi(transformation.Callee(), "matchCauseEffect"):
				matchEffectName, replacementName = "matchCauseEffect", "matchCause"
			default:
				continue
			}

			failureResult, failureArgument, ok := matchEffectConstructorHandler(tp, onFailure, "succeed")
			if !ok {
				continue
			}
			successResult, successArgument, ok := matchEffectConstructorHandler(tp, onSuccess, "succeed")
			if !ok {
				continue
			}
			matches = append(matches, MatchEffectToMatchMatch{
				SourceFile: sf, Location: scanner.GetErrorRangeForNode(sf, transformation.Callee()),
				CalleeNode: transformation.Callee(), MatchEffectName: matchEffectName, ReplacementName: replacementName,
				HandlerResults: [2]*ast.Node{failureResult, successResult}, SucceedArguments: [2]*ast.Node{failureArgument, successArgument},
			})
		}
	}
	return matches
}

func matchEffectConstructorHandler(tp *typeparser.TypeParser, node *ast.Node, constructorName string) (result *ast.Node, argument *ast.Node, ok bool) {
	lazy := typeparser.ParseLazyExpression(node, typeparser.LazyExpressionNone)
	if lazy == nil || lazy.Expression() == nil {
		return nil, nil, false
	}
	flow := tp.LongestPipingFlowAt(lazy.Expression(), false)
	if flow == nil || flow.Transformations().Len() == 0 {
		return nil, nil, false
	}
	last := flow.Transformations().Len() - 1
	transformation := flow.Transformations().At(last)
	if !tp.IsNodeReferenceToEffectModuleApi(transformation.Callee(), constructorName) {
		return nil, nil, false
	}
	argument = flow.TransformationInputNode(last)
	if argument == nil {
		return nil, nil, false
	}
	return lazy.Expression(), argument, true
}
