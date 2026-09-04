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

// MatchEffectToMapBoth suggests mapBoth when matchEffect only maps the error
// and success channels through Effect.fail and Effect.succeed, respectively.
var MatchEffectToMapBoth = rule.Rule{
	Name:            "matchEffectToMapBoth",
	Group:           "style",
	Description:     "Suggests Effect.mapBoth when Effect.matchEffect only transforms the failure and success channels",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_mapBoth_expresses_these_failure_and_success_transformations_more_directly_than_Effect_matchEffect_effect_matchEffectToMapBoth.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeMatchEffectToMapBoth(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(match.SourceFile, match.Location,
				tsdiag.Effect_mapBoth_expresses_these_failure_and_success_transformations_more_directly_than_Effect_matchEffect_effect_matchEffectToMapBoth,
				nil)
		}
		return diagnostics
	},
}

type MatchEffectToMapBothMatch struct {
	SourceFile      *ast.SourceFile
	Location        core.TextRange
	CalleeNameNode  *ast.Node
	HandlerResults  [2]*ast.Node
	ConstructorArgs [2]*ast.Node
}

func AnalyzeMatchEffectToMapBoth(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []MatchEffectToMapBothMatch {
	if tp == nil || sf == nil {
		return nil
	}
	var matches []MatchEffectToMapBothMatch
	for _, flow := range tp.PipingFlows(sf, true) {
		for index := range flow.Transformations {
			transformation := &flow.Transformations[index]
			if len(transformation.Args) != 1 || transformation.Callee == nil || transformation.Callee.Kind != ast.KindPropertyAccessExpression ||
				!tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "matchEffect") {
				continue
			}

			onFailure := typeparser.ObjectLiteralPropertyInitializer(transformation.Args[0], "onFailure")
			onSuccess := typeparser.ObjectLiteralPropertyInitializer(transformation.Args[0], "onSuccess")
			if onFailure == nil || onSuccess == nil {
				continue
			}
			failureResult, failureArgument, ok := matchEffectConstructorHandler(tp, onFailure, "fail")
			if !ok {
				continue
			}
			successResult, successArgument, ok := matchEffectConstructorHandler(tp, onSuccess, "succeed")
			if !ok {
				continue
			}
			calleeName := transformation.Callee.AsPropertyAccessExpression().Name()
			if calleeName == nil {
				continue
			}
			matches = append(matches, MatchEffectToMapBothMatch{
				SourceFile: sf, Location: scanner.GetErrorRangeForNode(sf, transformation.Callee), CalleeNameNode: calleeName,
				HandlerResults: [2]*ast.Node{failureResult, successResult}, ConstructorArgs: [2]*ast.Node{failureArgument, successArgument},
			})
		}
	}
	return matches
}
