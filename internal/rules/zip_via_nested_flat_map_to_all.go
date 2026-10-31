package rules

import (
	"slices"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// ZipViaNestedFlatMapToAll suggests Effect.all when an Effect.flatMap callback
// only maps an independent effect and pairs both results into a tuple or a
// two-key record.
var ZipViaNestedFlatMapToAll = rule.Rule{
	Name:            "zipViaNestedFlatMapToAll",
	Group:           "style",
	Description:     "Suggests Effect.all when a nested Effect.flatMap/Effect.map only pairs two independent effects",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.This_nested_Effect_flatMap_Slash_Effect_map_only_pairs_two_independent_effects_use_Effect_all_sequential_by_default_to_express_this_directly_effect_zipViaNestedFlatMapToAll.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeZipViaNestedFlatMapToAll(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.This_nested_Effect_flatMap_Slash_Effect_map_only_pairs_two_independent_effects_use_Effect_all_sequential_by_default_to_express_this_directly_effect_zipViaNestedFlatMapToAll,
				nil,
			)
		}
		return diagnostics
	},
}

// ZipViaNestedFlatMapToAllOperand is the effect consumed by a flow step, which
// the fix passes to Effect.all: either a standalone expression, or the first
// Count arguments of the pipe call that applies the step.
type ZipViaNestedFlatMapToAllOperand struct {
	Node     *ast.Node
	PipeCall *ast.CallExpression
	Count    int
}

// Parts returns the source nodes evaluated to build the operand.
func (o ZipViaNestedFlatMapToAllOperand) Parts() []*ast.Node {
	if o.Node != nil {
		return []*ast.Node{o.Node}
	}
	if o.PipeCall == nil {
		return nil
	}
	return append([]*ast.Node{o.PipeCall.Expression}, o.PipeCall.Arguments.Nodes[:o.Count]...)
}

// ZipViaNestedFlatMapToAllMatch holds the nodes needed by the diagnostic and
// quick fix. The fix replaces the outer flow up to and including the flatMap
// step with Effect.all; it is only offered when Fixable is set.
type ZipViaNestedFlatMapToAllMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	Callee     *ast.Node
	Flow       *typeparser.PipingFlow
	Index      int
	A          ZipViaNestedFlatMapToAllOperand
	B          ZipViaNestedFlatMapToAllOperand
	Record     bool
	Keys       []*ast.Node
	Fixable    bool
}

// AnalyzeZipViaNestedFlatMapToAll finds Effect.flatMap steps whose callback
// returns a flow ending in Effect.map over an effect that does not depend on
// the flatMap value, with a combiner that only packs both values into [x, y]
// or { k1: x, k2: y }.
func AnalyzeZipViaNestedFlatMapToAll(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []ZipViaNestedFlatMapToAllMatch {
	isStep := func(name string) func(typeparser.PipingFlowTransformation) bool {
		return func(step typeparser.PipingFlowTransformation) bool {
			return zipCallback(step) != nil && tp.IsNodeReferenceToEffectModuleApi(step.Callee(), name)
		}
	}
	isFlatMap, isMap := isStep("flatMap"), isStep("map")

	var matches []ZipViaNestedFlatMapToAllMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for _, sequence := range flow.FindTransformationSequences(isFlatMap).All() {
			index := sequence.Start()
			step := flow.Transformations().At(index)
			callee := step.Callee()
			outer := zipCallback(step)

			// The callback returns a flow whose last step is Effect.map; the
			// effect that step consumes is the second operand.
			inner := tp.LongestPipingFlowAt(outer.Expression(), false)
			if inner == nil || inner.Transformations().Len() == 0 {
				continue
			}
			last := inner.Transformations().Len() - 1
			innerStep := inner.Transformations().At(last)
			if !isMap(innerStep) {
				continue
			}
			innerCallback := zipCallback(innerStep)

			outerSymbol := tp.GetSymbolAtLocation(outer.Params().At(0).Name())
			innerSymbol := tp.GetSymbolAtLocation(innerCallback.Params().At(0).Name())
			if outerSymbol == nil || innerSymbol == nil {
				continue
			}
			record, keys, ok := zipCombinerShape(tp, c, innerCallback.Expression(), outerSymbol, innerSymbol)
			if !ok {
				continue
			}

			b, ok := zipStepInput(tp, inner, last)
			if !ok || !zipOperandIndependentOf(tp, c, b, outerSymbol) {
				continue
			}
			if slices.ContainsFunc(b.Parts(), func(part *ast.Node) bool { return !tp.IsExpressionEvaluableAtLocation(part, callee) }) ||
				tp.StrictEffectType(flow.TransformationInputType(index)) == nil ||
				tp.StrictEffectType(inner.TransformationInputType(last)) == nil {
				continue
			}

			// The input of an Effect.fn pipeline step is the generator body's
			// result, which has no expression to pass to Effect.all.
			a, resolved := zipStepInput(tp, flow, index)
			matches = append(matches, ZipViaNestedFlatMapToAllMatch{
				SourceFile: sf,
				Location:   scanner.GetErrorRangeForNode(sf, callee),
				Callee:     callee,
				Flow:       flow,
				Index:      index,
				A:          a,
				B:          b,
				Record:     record,
				Keys:       keys,
				Fixable: resolved && !hasExplicitTypeArguments(&step) && !hasExplicitTypeArguments(&innerStep) &&
					zipOperandClonable(a) && zipOperandClonable(b),
			})
		}
	}

	return matches
}

// zipCallback returns the single arrow callback of a step when it declares one
// plain identifier parameter and returns an expression.
func zipCallback(step typeparser.PipingFlowTransformation) *typeparser.ParsedLazyExpression {
	callee := step.Callee()
	if callee == nil || step.Args().Len() != 1 {
		return nil
	}
	callback := typeparser.ParseLazyExpression(step.Args().At(0), typeparser.LazyExpressionNone)
	if callback == nil || callback.Node().Kind != ast.KindArrowFunction || callback.Expression() == nil ||
		callback.Params().Len() != 1 {
		return nil
	}
	parameter := callback.Params().At(0).AsParameterDeclaration()
	if !plainTapParameter(parameter) || parameter.QuestionToken != nil {
		return nil
	}
	return callback
}

// zipCombinerShape reports whether combiner is exactly [x, y] (optionally
// `as const`) or a two-property object literal whose values are x then y.
// For records it returns the key nodes in order.
func zipCombinerShape(tp *typeparser.TypeParser, c *checker.Checker, combiner *ast.Node, x *ast.Symbol, y *ast.Symbol) (bool, []*ast.Node, bool) {
	combiner = ast.SkipParentheses(combiner)
	if combiner != nil && (combiner.Kind == ast.KindAsExpression || combiner.Kind == ast.KindTypeAssertionExpression) &&
		ast.IsConstTypeReference(combiner.Type()) {
		combiner = ast.SkipParentheses(combiner.Expression())
	}
	if combiner == nil {
		return false, nil, false
	}
	expected := []*ast.Symbol{x, y}

	switch combiner.Kind {
	case ast.KindArrayLiteralExpression:
		elements := combiner.Elements()
		if len(elements) != 2 {
			return false, nil, false
		}
		for i, element := range elements {
			if element.Kind != ast.KindIdentifier || hoistSensitiveReferenceSymbol(tp, c, element) != expected[i] {
				return false, nil, false
			}
		}
		return false, nil, true

	case ast.KindObjectLiteralExpression:
		properties := combiner.Properties()
		if len(properties) != 2 {
			return false, nil, false
		}
		keys := make([]*ast.Node, 0, 2)
		seen := map[string]bool{}
		for i, property := range properties {
			var value *ast.Symbol
			switch property.Kind {
			case ast.KindShorthandPropertyAssignment:
				value = c.GetShorthandAssignmentValueSymbol(property)
			case ast.KindPropertyAssignment:
				initializer := ast.SkipParentheses(property.Initializer())
				if initializer == nil || initializer.Kind != ast.KindIdentifier {
					return false, nil, false
				}
				value = hoistSensitiveReferenceSymbol(tp, c, initializer)
			default:
				return false, nil, false
			}
			key := property.Name()
			if value != expected[i] || key == nil || (key.Kind != ast.KindIdentifier && key.Kind != ast.KindStringLiteral) {
				return false, nil, false
			}
			text := key.Text()
			// Effect.all and Object.keys visit integer-like keys first, and
			// `__proto__` sets the prototype instead of defining a key.
			if seen[text] || text == "__proto__" || isCanonicalArrayIndex(text) {
				return false, nil, false
			}
			seen[text] = true
			keys = append(keys, key)
		}
		return true, keys, true
	}

	return false, nil, false
}

func isCanonicalArrayIndex(text string) bool {
	if text == "" || len(text) > 1 && text[0] == '0' {
		return false
	}
	for _, ch := range text {
		if ch < '0' || ch > '9' {
			return false
		}
	}
	return true
}

// zipStepInput returns the effect consumed by the indexed step of flow. A pipe
// or pipeable step that is not the first argument of its pipe call consumes
// the same call truncated before it.
func zipStepInput(tp *typeparser.TypeParser, flow *typeparser.PipingFlow, index int) (ZipViaNestedFlatMapToAllOperand, bool) {
	step := flow.Transformations().At(index)
	switch step.Kind() {
	case typeparser.TransformationKindEffectFn, typeparser.TransformationKindEffectFnUntraced:
		return ZipViaNestedFlatMapToAllOperand{}, false
	case typeparser.TransformationKindPipe, typeparser.TransformationKindPipeable:
		call, argumentIndex := zipPipeCallOf(step)
		if call == nil || argumentIndex < 0 {
			return ZipViaNestedFlatMapToAllOperand{}, false
		}
		parsed := tp.ParsePipeCall(call.AsNode())
		if parsed == nil {
			return ZipViaNestedFlatMapToAllOperand{}, false
		}
		first := 0
		if step.Kind() == typeparser.TransformationKindPipe {
			first = 1
		}
		switch {
		case argumentIndex == first:
			return ZipViaNestedFlatMapToAllOperand{Node: parsed.Subject()}, parsed.Subject() != nil
		case argumentIndex > first:
			return ZipViaNestedFlatMapToAllOperand{PipeCall: call, Count: argumentIndex}, true
		}
		return ZipViaNestedFlatMapToAllOperand{}, false
	}
	if input := flow.TransformationInputNode(index); input != nil {
		return ZipViaNestedFlatMapToAllOperand{Node: input}, true
	}
	return ZipViaNestedFlatMapToAllOperand{}, false
}

// zipPipeCallOf returns the pipe call that receives a pipe or pipeable step as
// an argument, with the argument's index.
func zipPipeCallOf(step typeparser.PipingFlowTransformation) (*ast.CallExpression, int) {
	argument := step.Callee()
	if parent := argument.Parent; parent != nil && ast.IsCallExpression(parent) && parent.AsCallExpression().Expression == argument {
		argument = parent
	}
	parent := argument.Parent
	if parent == nil || !ast.IsCallExpression(parent) || parent.AsCallExpression().Arguments == nil {
		return nil, -1
	}
	call := parent.AsCallExpression()
	return call, slices.Index(call.Arguments.Nodes, argument)
}

// zipOperandClonable reports whether the fix can rebuild the operand without
// dropping explicit type arguments or optional chaining of a truncated pipe call.
func zipOperandClonable(operand ZipViaNestedFlatMapToAllOperand) bool {
	call := operand.PipeCall
	return call == nil || call.AsNode().Flags&ast.NodeFlagsOptionalChain == 0 &&
		call.Expression.Flags&ast.NodeFlagsOptionalChain == 0 &&
		(call.TypeArguments == nil || len(call.TypeArguments.Nodes) == 0)
}

// zipOperandIndependentOf reports whether no identifier in the operand,
// including nested functions and type positions, refers to symbol.
func zipOperandIndependentOf(tp *typeparser.TypeParser, c *checker.Checker, operand ZipViaNestedFlatMapToAllOperand, symbol *ast.Symbol) bool {
	var references func(node *ast.Node) bool
	references = func(node *ast.Node) bool {
		if node.Kind == ast.KindIdentifier && hoistSensitiveReferenceSymbol(tp, c, node) == symbol {
			return true
		}
		//ets:ast-walk scans the operand for identifiers that refer to the flatMap parameter
		return node.ForEachChild(references)
	}
	return !slices.ContainsFunc(operand.Parts(), references)
}
