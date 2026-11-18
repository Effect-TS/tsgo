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

var FlatMapMapToZipWith = rule.Rule{
	Name:            "flatMapMapToZipWith",
	Group:           "style",
	Description:     "Suggests Effect.zipWith when Effect.flatMap only maps an independent effect with its parameter",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_zipWith_combines_these_two_independent_effects_directly_instead_of_Effect_flatMap_nesting_an_Effect_map_over_an_effect_that_does_not_use_the_outer_result_effect_flatMapMapToZipWith.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeFlatMapMapToZipWith(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(match.SourceFile, match.Location,
				tsdiag.Effect_zipWith_combines_these_two_independent_effects_directly_instead_of_Effect_flatMap_nesting_an_Effect_map_over_an_effect_that_does_not_use_the_outer_result_effect_flatMapMapToZipWith, nil)
		}
		return diagnostics
	},
}

type FlatMapMapToZipWithMatch struct {
	SourceFile      *ast.SourceFile
	Location        core.TextRange
	Step            typeparser.PipingFlowTransformation
	FlatMapCallback *ast.Node
	ReturnedFlow    *typeparser.PipingFlow
	MapCallback     *ast.Node
	// MapResult is what MapCallback returns when its body is an expression or a
	// single return, and nil otherwise.
	MapResult *ast.Node
}

func AnalyzeFlatMapMapToZipWith(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []FlatMapMapToZipWithMatch {
	var matches []FlatMapMapToZipWithMatch
	isEffectApiStep := func(step typeparser.PipingFlowTransformation, name string) bool {
		callee := step.Callee()
		return callee != nil && callee.Flags&ast.NodeFlagsOptionalChain == 0 && step.Args().Len() == 1 &&
			tp.IsNodeReferenceToEffectModuleApi(callee, name)
	}
	isFlatMap := func(step typeparser.PipingFlowTransformation) bool { return isEffectApiStep(step, "flatMap") }

	for _, flow := range tp.PipingFlows(sf, true).All() {
		for _, sequence := range flow.FindTransformationSequences(isFlatMap).All() {
			i := sequence.Start()
			step := sequence.Transformations().At(0)

			parsedFlatMapCallback := typeparser.ParseLazyExpression(step.Args().At(0), typeparser.LazyExpressionAllowTypeParameters)
			if parsedFlatMapCallback == nil || parsedFlatMapCallback.Params().Len() != 1 ||
				!flatMapMapToZipWithPlainParameter(parsedFlatMapCallback.Params().At(0)) {
				continue
			}
			flatMapCallback := parsedFlatMapCallback.Node()
			returned := ast.SkipParentheses(parsedFlatMapCallback.Expression())
			returnedFlow := tp.LongestPipingFlowAt(returned, false)
			if returnedFlow == nil || returnedFlow.Node() != returned || returnedFlow.Transformations().Len() == 0 {
				continue
			}
			last := returnedFlow.Transformations().Len() - 1
			mapStep := returnedFlow.Transformations().At(last)
			if !isEffectApiStep(mapStep, "map") {
				continue
			}

			mapCallback := flatMapMapToZipWithMapCallback(mapStep.Args().At(0))
			if mapCallback == nil || !flatMapMapToZipWithUnaryMapper(mapCallback) {
				continue
			}
			var mapResult *ast.Node
			if parsedMapCallback := typeparser.ParseLazyExpression(mapCallback, typeparser.LazyExpressionAllowTypeParameters); parsedMapCallback != nil {
				mapResult = ast.SkipParentheses(parsedMapCallback.Expression())
			}

			var outerSymbols []*ast.Symbol
			for _, symbol := range ast.TSGetSymbolTableEntries(flatMapCallback.Locals()) {
				outerSymbols = append(outerSymbols, symbol)
			}
			if len(outerSymbols) == 0 {
				continue
			}
			if !flatMapMapToZipWithOnlyMapperUsesOuter(tp, returned, mapCallback, outerSymbols) ||
				tp.UsesFunctionOwnBindings(flatMapCallback, returned) {
				continue
			}
			if flatMapMapToZipWithReyieldsOuter(tp, flatMapCallback, mapResult) {
				continue
			}

			if tp.StrictEffectType(flow.TransformationInputType(i)) == nil ||
				tp.StrictEffectType(returnedFlow.TransformationInputType(last)) == nil {
				continue
			}

			matches = append(matches, FlatMapMapToZipWithMatch{
				SourceFile:      sf,
				Location:        scanner.GetErrorRangeForNode(sf, step.Callee()),
				Step:            step,
				FlatMapCallback: flatMapCallback,
				ReturnedFlow:    returnedFlow,
				MapCallback:     mapCallback,
				MapResult:       mapResult,
			})
		}
	}
	return matches
}

func flatMapMapToZipWithPlainParameter(parameter *ast.Node) bool {
	if parameter == nil || parameter.Kind != ast.KindParameter {
		return false
	}
	declaration := parameter.AsParameterDeclaration()
	if declaration.DotDotDotToken != nil || declaration.Initializer != nil {
		return false
	}
	name := declaration.Name()
	if name == nil {
		return false
	}
	return name.Kind == ast.KindIdentifier || name.Kind == ast.KindObjectBindingPattern || name.Kind == ast.KindArrayBindingPattern
}

// flatMapMapToZipWithOnlyMapperUsesOuter reports whether the outer parameter is
// used in returned, and only inside mapper: everything else in returned becomes
// the independent effect that zipWith runs on its own.
func flatMapMapToZipWithOnlyMapperUsesOuter(tp *typeparser.TypeParser, returned, mapper *ast.Node, outerSymbols []*ast.Symbol) bool {
	used := false
	for _, symbol := range outerSymbols {
		for _, reference := range tp.ReferencesToSymbol(returned, symbol).All() {
			if reference.Pos() < mapper.Pos() || reference.End() > mapper.End() {
				return false
			}
			used = true
		}
	}
	return used
}

func flatMapMapToZipWithReyieldsOuter(tp *typeparser.TypeParser, flatMapCallback, mapResult *ast.Node) bool {
	name := flatMapCallback.Parameters()[0].Name()
	if name.Kind != ast.KindIdentifier {
		return false
	}
	return mapResult != nil && mapResult.Kind == ast.KindIdentifier && tp.GetSymbolAtLocation(mapResult) == tp.GetSymbolAtLocation(name)
}

func flatMapMapToZipWithUnaryMapper(mapper *ast.Node) bool {
	values := 0
	for _, parameter := range mapper.Parameters() {
		if ast.IsThisParameter(parameter) {
			continue
		}
		values++
		if values > 1 || !flatMapMapToZipWithPlainParameter(parameter) {
			return false
		}
	}
	return true
}

// flatMapMapToZipWithMapCallback accepts any body shape, unlike
// ParseLazyExpression: the whole body moves into the combiner, so a mapper that
// branches between several returns is still fine.
func flatMapMapToZipWithMapCallback(node *ast.Node) *ast.Node {
	node = ast.SkipParentheses(node)
	if node == nil {
		return nil
	}
	isNonGeneratorFunction := ast.IsArrowFunction(node) || ast.IsFunctionExpression(node) && node.AsFunctionExpression().AsteriskToken == nil
	if !isNonGeneratorFunction || node.Body() == nil || ast.GetCombinedModifierFlags(node)&ast.ModifierFlagsAsync != 0 {
		return nil
	}
	return node
}
