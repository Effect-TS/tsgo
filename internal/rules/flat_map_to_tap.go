package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/readonly"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var FlatMapToTap = rule.Rule{
	Name:            "flatMapToTap",
	Group:           "style",
	Description:     "Suggests Effect.tap when a flatMap or andThen callback runs an effect and returns its unchanged parameter",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_tap_expresses_this_observation_more_directly_than_Effect_0_with_a_manual_re_yield_effect_flatMapToTap.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeFlatMapToTap(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			method := "flatMap"
			if ctx.TypeParser.IsNodeReferenceToEffectModuleApi(match.Callee, "andThen") {
				method = "andThen"
			}
			diagnostics[i] = ctx.NewDiagnostic(match.SourceFile, match.Location,
				tsdiag.Effect_tap_expresses_this_observation_more_directly_than_Effect_0_with_a_manual_re_yield_effect_flatMapToTap, nil, method)
		}
		return diagnostics
	},
}

type FlatMapToTapMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	Callee     *ast.Node
	ReturnFlow *typeparser.PipingFlow
}

// AnalyzeFlatMapToTap finds callbacks that restore their unchanged input after an effect.
func AnalyzeFlatMapToTap(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []FlatMapToTapMatch {
	var matches []FlatMapToTapMatch
	flows := tp.PipingFlows(sf, true)
	for _, flow := range flows.All() {
		for i, step := range flow.Transformations().All() {
			callee := step.Callee()
			args := step.Args()
			if step.Kind() == typeparser.TransformationKindCall && callee != nil && callee.Kind == ast.KindCallExpression {
				factory := callee.AsCallExpression()
				if factory.Arguments != nil && len(factory.Arguments.Nodes) == 1 && factory.TypeArguments == nil &&
					tp.IsNodeReferenceToEffectModuleApi(factory.Expression, "andThen") {
					callee = factory.Expression
					args = readonly.New(factory.Arguments.Nodes)
				}
			}
			if callee == nil || args.Len() != 1 || step.TypeArguments() != nil ||
				callee.Flags&ast.NodeFlagsOptionalChain != 0 ||
				!tp.IsNodeReferenceToEffectModuleApi(callee, "flatMap") && !tp.IsNodeReferenceToEffectModuleApi(callee, "andThen") {
				continue
			}
			input := tp.StrictEffectType(flow.TransformationInputType(i))
			output := tp.StrictEffectType(step.OutType())
			if input == nil || output == nil || input.A() != output.A() {
				continue
			}
			callback := typeparser.ParseLazyExpression(args.At(0), typeparser.LazyExpressionAllowMultiStatementBlock)
			if callback == nil || callback.Params().Len() != 1 || callback.Node().Type() != nil {
				continue
			}
			parameter := callback.Params().At(0).AsParameterDeclaration()
			if !plainTapParameter(parameter) || step.Kind() == typeparser.TransformationKindCall && parameter.Type != nil {
				continue
			}
			symbol := tp.GetSymbolAtLocation(parameter.Name())
			if symbol == nil || len(ast.TSGetSymbolDeclarations(symbol)) != 1 || checker.Checker_isSymbolAssigned(c, symbol) {
				continue
			}
			returned := tp.LongestPipingFlowAt(callback.Expression(), false)
			if returned == nil || returned.Transformations().Len() == 0 {
				continue
			}
			last := returned.Transformations().Len() - 1
			terminal := returned.Transformations().At(last)
			if terminal.Callee() == nil || terminal.Callee().Flags&ast.NodeFlagsOptionalChain != 0 || terminal.Args().Len() != 1 ||
				tp.StrictEffectType(returned.TransformationInputType(last)) == nil {
				continue
			}
			ref := terminal.Args().At(0)
			switch {
			case tp.IsNodeReferenceToEffectModuleApi(terminal.Callee(), "as"):
			case tp.IsNodeReferenceToEffectModuleApi(terminal.Callee(), "map"):
				mapper := typeparser.ParseLazyExpression(ref, typeparser.LazyExpressionNone)
				if mapper == nil || mapper.Params().Len() > 1 ||
					mapper.Params().Len() == 1 && !plainTapParameter(mapper.Params().At(0).AsParameterDeclaration()) {
					continue
				}
				ref = mapper.Expression()
			default:
				continue
			}
			ref = ast.SkipParentheses(ref)
			if ref.Kind != ast.KindIdentifier || tp.GetSymbolAtLocation(ref) != symbol {
				continue
			}
			matches = append(matches, FlatMapToTapMatch{
				SourceFile: sf,
				Location:   scanner.GetErrorRangeForNode(sf, callee),
				Callee:     callee,
				ReturnFlow: returned,
			})
		}
	}
	return matches
}

func plainTapParameter(parameter *ast.ParameterDeclaration) bool {
	return parameter.Name() != nil && parameter.Name().Kind == ast.KindIdentifier &&
		parameter.Initializer == nil && parameter.DotDotDotToken == nil
}
