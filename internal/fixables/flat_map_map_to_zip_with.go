package fixables

import (
	"slices"

	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var FlatMapMapToZipWithFix = fixable.Fixable{
	Name:        "flatMapMapToZipWith",
	Description: "Replace Effect.flatMap with Effect.zipWith",
	ErrorCodes: []int32{
		tsdiag.Effect_zipWith_combines_these_two_independent_effects_directly_instead_of_Effect_flatMap_nesting_an_Effect_map_over_an_effect_that_does_not_use_the_outer_result_effect_flatMapMapToZipWith.Code(),
	},
	FixIDs: []string{"flatMapMapToZipWith_fix"},
	Run:    runFlatMapMapToZipWithFix,
}

func runFlatMapMapToZipWithFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzeFlatMapMapToZipWith(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if !flatMapMapToZipWithFixIsSafe(ctx, &match) {
			continue
		}
		name := "zipWith"
		if flatMapMapToZipWithMapCallbackIsZip(ctx.TypeParser, &match) {
			name = "zip"
		}
		member := effectModuleMember(ctx, name)
		target := resolveModuleMember(ctx, match.Step.Callee(), typeparser.FindEffectModuleReference(match.SourceFile), member)
		if target == nil {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect." + name,
			Run: func(tracker *rewriter.Tracker) {
				arguments := []*ast.Node{tracker.DeepCloneNode(match.ReturnedFlow.Subject().Node())}
				if name == "zipWith" {
					arguments = append(arguments, flatMapMapToZipWithCombiner(tracker, &match))
				}
				tracker.ReplacePipingFlowTransformation(match.SourceFile, &match.Step, rewriter.PipingFlowTransformationReplacement{
					Callee:    target.expression(tracker, member),
					Arguments: tracker.NewNodeList(arguments),
				})
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}

func flatMapMapToZipWithFixIsSafe(ctx *fixable.Context, match *rules.FlatMapMapToZipWithMatch) bool {
	return flatMapMapToZipWithThatCanRunFirst(ctx.TypeParser, match) &&
		!flatMapMapToZipWithDropsExplicitTypes(match) &&
		!flatMapMapToZipWithCallbacksDeclareTypeParameters(match) &&
		!flatMapMapToZipWithOuterParameterIsDestructured(match) &&
		flatMapMapToZipWithParameterListsCanConcatenate(match) &&
		!flatMapMapToZipWithMapParametersCaptureOuterNames(ctx, match) &&
		!flatMapMapToZipWithMapCallbackIsTiedToItsFunction(ctx, match)
}

func flatMapMapToZipWithThatCanRunFirst(tp *typeparser.TypeParser, match *rules.FlatMapMapToZipWithMatch) bool {
	if match.ReturnedFlow.Transformations().Len() != 1 {
		return false
	}
	that := match.ReturnedFlow.Subject().Node()
	return tp.IsExpressionValueStableAtLocation(that, match.Step.Callee()) || tp.IsDeclarationFileModuleConstant(that)
}

func flatMapMapToZipWithDropsExplicitTypes(match *rules.FlatMapMapToZipWithMatch) bool {
	return match.Step.TypeArguments() != nil ||
		match.ReturnedFlow.Transformations().At(0).TypeArguments() != nil ||
		match.FlatMapCallback.Type() != nil
}

func flatMapMapToZipWithCallbacksDeclareTypeParameters(match *rules.FlatMapMapToZipWithMatch) bool {
	return len(match.FlatMapCallback.TypeParameters()) > 0 || len(match.MapCallback.TypeParameters()) > 0
}

func flatMapMapToZipWithOuterParameterIsDestructured(match *rules.FlatMapMapToZipWithMatch) bool {
	return match.FlatMapCallback.Parameters()[0].Name().Kind != ast.KindIdentifier
}

func flatMapMapToZipWithParameterListsCanConcatenate(match *rules.FlatMapMapToZipWithMatch) bool {
	outerParameter := match.FlatMapCallback.Parameters()[0]
	mapParameters := match.MapCallback.Parameters()
	// A `this` parameter is not a value parameter and must stay first, so the
	// two parameter lists cannot be concatenated around it (TS2680).
	if slices.ContainsFunc(append([]*ast.Node{outerParameter}, mapParameters...), ast.IsThisParameter) {
		return false
	}
	// The outer parameter becomes the first of the combiner's parameters, and a
	// required parameter cannot follow an optional one (TS1016).
	if outerParameter.AsParameterDeclaration().QuestionToken != nil {
		for _, parameter := range mapParameters {
			if parameter.AsParameterDeclaration().QuestionToken == nil {
				return false
			}
		}
	}
	return true
}

func flatMapMapToZipWithMapParametersCaptureOuterNames(ctx *fixable.Context, match *rules.FlatMapMapToZipWithMatch) bool {
	flatMapCallback := match.FlatMapCallback
	outerParameter := flatMapCallback.Parameters()[0]
	for name := range ast.TSGetSymbolTableEntries(match.MapCallback.Locals()) {
		if ast.TSGetSymbolTableEntry(flatMapCallback.Locals(), name) != nil {
			return true
		}
		captured := ctx.Checker.ResolveName(name, flatMapCallback, ast.SymbolFlagsAll, false)
		if captured != nil && ctx.TypeParser.ReferencesToSymbol(outerParameter, captured).Len() > 0 {
			return true
		}
	}
	return false
}

func flatMapMapToZipWithMapCallbackIsTiedToItsFunction(ctx *fixable.Context, match *rules.FlatMapMapToZipWithMatch) bool {
	tp := ctx.TypeParser
	mapCallback := match.MapCallback
	// A direct eval reads any binding by name at run time, including the
	// function-own ones (arguments, this) that UsesFunctionOwnBindings checks
	// syntactically, so code that may call it cannot move.
	eval := ctx.Checker.GetGlobalSymbol("eval", ast.SymbolFlagsValue, nil)
	moved := append(slices.Clone(mapCallback.Parameters()), mapCallback.Type(), mapCallback.Body())
	for _, part := range moved {
		if part != nil && (tp.UsesFunctionOwnBindings(match.FlatMapCallback, part) || tp.UsesFunctionOwnBindings(mapCallback, part) ||
			eval != nil && tp.ReferencesToSymbol(part, eval).Len() > 0) {
			return true
		}
	}
	return false
}

func flatMapMapToZipWithMapCallbackIsZip(tp *typeparser.TypeParser, match *rules.FlatMapMapToZipWithMatch) bool {
	mapCallback := match.MapCallback
	pair := match.MapResult
	if pair == nil || len(mapCallback.Parameters()) != 1 {
		return false
	}
	zipWouldDropReturnType := mapCallback.Type() != nil
	if zipWouldDropReturnType {
		return false
	}
	outerParameter := match.FlatMapCallback.Parameters()[0]
	mapParameter := mapCallback.Parameters()[0]
	for _, parameter := range []*ast.Node{outerParameter, mapParameter} {
		if parameter.Name().Kind != ast.KindIdentifier || parameter.Type() != nil || parameter.AsParameterDeclaration().QuestionToken != nil {
			return false
		}
	}
	if pair.Kind != ast.KindArrayLiteralExpression {
		return false
	}
	elements := pair.AsArrayLiteralExpression().Elements
	if elements == nil || len(elements.Nodes) != 2 {
		return false
	}
	for i, parameter := range []*ast.Node{outerParameter, mapParameter} {
		element := elements.Nodes[i]
		if element.Kind != ast.KindIdentifier || tp.GetSymbolAtLocation(element) != tp.GetSymbolAtLocation(parameter.Name()) {
			return false
		}
	}
	return true
}

func flatMapMapToZipWithCombiner(tracker *rewriter.Tracker, match *rules.FlatMapMapToZipWithMatch) *ast.Node {
	mapCallback := match.MapCallback
	parameters := []*ast.Node{tracker.DeepCloneNode(match.FlatMapCallback.Parameters()[0])}
	for _, parameter := range mapCallback.Parameters() {
		parameters = append(parameters, tracker.DeepCloneNode(parameter))
	}
	var returnType *ast.Node
	if original := mapCallback.Type(); original != nil {
		returnType = tracker.DeepCloneNode(original)
	}
	body := tracker.DeepCloneNode(mapCallback.Body())
	if mapCallback.Kind == ast.KindFunctionExpression {
		var name *ast.Node
		if original := mapCallback.Name(); original != nil {
			name = tracker.DeepCloneNode(original)
		}
		return tracker.NewFunctionExpression(nil, nil, name, nil, tracker.NewNodeList(parameters), returnType, nil, body)
	}
	return tracker.NewArrowFunction(nil, nil, tracker.NewNodeList(parameters), returnType, nil, tracker.NewToken(ast.KindEqualsGreaterThanToken), body)
}
