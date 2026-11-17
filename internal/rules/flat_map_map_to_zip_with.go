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

			flatMapCallback := flatMapMapToZipWithFunction(step.Args().At(0))
			if flatMapCallback == nil || len(flatMapCallback.Parameters()) != 1 || !flatMapMapToZipWithPlainParameter(flatMapCallback.Parameters()[0]) {
				continue
			}
			returned := FlatMapMapToZipWithReturnedExpression(flatMapCallback)
			if returned == nil {
				continue
			}
			returnedFlow := tp.LongestPipingFlowAt(returned, false)
			if returnedFlow == nil || returnedFlow.Node() != returned || returnedFlow.Transformations().Len() == 0 {
				continue
			}
			last := returnedFlow.Transformations().Len() - 1
			mapStep := returnedFlow.Transformations().At(last)
			if !isEffectApiStep(mapStep, "map") {
				continue
			}

			mapCallback := flatMapMapToZipWithFunction(mapStep.Args().At(0))
			if mapCallback == nil || !flatMapMapToZipWithUnaryMapper(mapCallback) {
				continue
			}

			var outerSymbols []*ast.Symbol
			for _, symbol := range ast.TSGetSymbolTableEntries(flatMapCallback.Locals()) {
				outerSymbols = append(outerSymbols, symbol)
			}
			if len(outerSymbols) == 0 {
				continue
			}
			if flatMapMapToZipWithThatDependsOnOuter(tp, flatMapCallback, returnedFlow, outerSymbols) {
				continue
			}
			if !flatMapMapToZipWithReferencesAny(tp, mapCallback, outerSymbols) {
				continue
			}
			if flatMapMapToZipWithReyieldsOuter(tp, flatMapCallback, mapCallback) {
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

func flatMapMapToZipWithThatParts(returnedFlow *typeparser.PipingFlow) []*ast.Node {
	parts := []*ast.Node{returnedFlow.Subject().Node()}
	steps := returnedFlow.Transformations()
	for j := range steps.Len() - 1 {
		step := steps.At(j)
		parts = append(parts, step.Callee())
		for _, argument := range step.Args().All() {
			parts = append(parts, argument)
		}
	}
	return parts
}

func flatMapMapToZipWithThatDependsOnOuter(tp *typeparser.TypeParser, flatMapCallback *ast.Node, returnedFlow *typeparser.PipingFlow, outerSymbols []*ast.Symbol) bool {
	for _, part := range flatMapMapToZipWithThatParts(returnedFlow) {
		if part == nil {
			continue
		}
		if flatMapMapToZipWithReferencesAny(tp, part, outerSymbols) || tp.UsesFunctionOwnBindings(flatMapCallback, part) {
			return true
		}
	}
	return false
}

func flatMapMapToZipWithReferencesAny(tp *typeparser.TypeParser, root *ast.Node, symbols []*ast.Symbol) bool {
	for _, symbol := range symbols {
		if tp.ReferencesToSymbol(root, symbol).Len() > 0 {
			return true
		}
	}
	return false
}

func flatMapMapToZipWithReyieldsOuter(tp *typeparser.TypeParser, flatMapCallback, mapCallback *ast.Node) bool {
	name := flatMapCallback.Parameters()[0].Name()
	if name.Kind != ast.KindIdentifier {
		return false
	}
	returned := FlatMapMapToZipWithReturnedExpression(mapCallback)
	return returned != nil && returned.Kind == ast.KindIdentifier && tp.GetSymbolAtLocation(returned) == tp.GetSymbolAtLocation(name)
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

func flatMapMapToZipWithFunction(node *ast.Node) *ast.Node {
	node = ast.SkipParentheses(node)
	if node == nil || ast.GetCombinedModifierFlags(node)&ast.ModifierFlagsAsync != 0 {
		return nil
	}
	switch node.Kind {
	case ast.KindArrowFunction:
	case ast.KindFunctionExpression:
		if node.AsFunctionExpression().AsteriskToken != nil {
			return nil
		}
	default:
		return nil
	}
	if node.Body() == nil {
		return nil
	}
	return node
}

func FlatMapMapToZipWithReturnedExpression(fn *ast.Node) *ast.Node {
	body := fn.Body()
	if body.Kind != ast.KindBlock {
		return ast.SkipParentheses(body)
	}
	statements := body.AsBlock().Statements
	if statements == nil || len(statements.Nodes) != 1 || statements.Nodes[0].Kind != ast.KindReturnStatement {
		return nil
	}
	if expression := statements.Nodes[0].AsReturnStatement().Expression; expression != nil {
		return ast.SkipParentheses(expression)
	}
	return nil
}
