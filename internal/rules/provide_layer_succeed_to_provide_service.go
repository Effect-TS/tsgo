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

var ProvideLayerSucceedToProvideService = rule.Rule{
	Name:            "provideLayerSucceedToProvideService",
	Group:           "style",
	Description:     "Suggests providing inline Layer.succeed and Layer.effect services directly",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.Effect_0_provides_this_inline_single_service_layer_directly_effect_provideLayerSucceedToProvideService.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeProvideLayerSucceedToProvideService(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_0_provides_this_inline_single_service_layer_directly_effect_provideLayerSucceedToProvideService,
				nil,
				match.ReplacementMethodName,
			)
		}
		return diagnostics
	},
}

type ProvideLayerSucceedToProvideServiceMatch struct {
	SourceFile            *ast.SourceFile
	Location              core.TextRange
	ProvideTransformation *typeparser.PipingFlowTransformation
	EffectModuleNode      *ast.Node
	ServiceNode           *ast.Node
	ImplementationNode    *ast.Node
	ReplacementMethodName string
}

func AnalyzeProvideLayerSucceedToProvideService(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []ProvideLayerSucceedToProvideServiceMatch {
	if tp == nil || sf == nil || tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}

	var matches []ProvideLayerSucceedToProvideServiceMatch
	for _, flow := range tp.PipingFlows(sf, true) {
		for index := range flow.Transformations {
			provide := &flow.Transformations[index]
			if provide.Callee == nil || provide.Callee.Kind != ast.KindPropertyAccessExpression ||
				!tp.IsNodeReferenceToEffectModuleApi(provide.Callee, "provide") || len(provide.Args) != 1 {
				continue
			}

			service, implementation, replacement, ok := inlineSingleServiceLayer(tp, provide.Args[0])
			if !ok {
				continue
			}
			serviceType := tp.GetTypeAtLocation(service)
			if serviceType == nil || (!tp.IsServiceType(serviceType) && !tp.IsContextTag(serviceType)) {
				continue
			}

			matches = append(matches, ProvideLayerSucceedToProvideServiceMatch{
				SourceFile:            sf,
				Location:              scanner.GetErrorRangeForNode(sf, provide.Callee),
				ProvideTransformation: provide,
				EffectModuleNode:      provide.Callee.AsPropertyAccessExpression().Expression,
				ServiceNode:           service,
				ImplementationNode:    implementation,
				ReplacementMethodName: replacement,
			})
		}
	}
	return matches
}

func inlineSingleServiceLayer(tp *typeparser.TypeParser, node *ast.Node) (*ast.Node, *ast.Node, string, bool) {
	node = ast.SkipParentheses(node)
	flow := tp.LongestPipingFlowAt(node, false)
	if flow == nil || flow.Node != node || len(flow.Transformations) != 1 || flow.Subject.Node == nil {
		return nil, nil, "", false
	}
	transformation := &flow.Transformations[0]
	if transformation.Callee == nil || len(transformation.Args) != 1 ||
		transformation.TypeArguments != nil && len(transformation.TypeArguments.Nodes) > 0 {
		return nil, nil, "", false
	}
	replacement := ""
	switch {
	case tp.IsNodeReferenceToEffectLayerModuleApi(transformation.Callee, "succeed"):
		replacement = "provideService"
	case tp.IsNodeReferenceToEffectLayerModuleApi(transformation.Callee, "effect"):
		replacement = "provideServiceEffect"
	default:
		return nil, nil, "", false
	}
	return transformation.Args[0], flow.Subject.Node, replacement, true
}
