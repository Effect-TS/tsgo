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

var FlatMapProvideServiceToProvideServiceEffect = rule.Rule{
	Name:            "flatMapProvideServiceToProvideServiceEffect",
	Group:           "style",
	Description:     "Suggests Effect.provideServiceEffect when a flatMap or andThen callback only forwards its parameter to Effect.provideService",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_provideServiceEffect_provides_this_service_from_the_acquiring_effect_directly_instead_of_Effect_0_forwarding_its_parameter_to_Effect_provideService_effect_flatMapProvideServiceToProvideServiceEffect.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeFlatMapProvideServiceToProvideServiceEffect(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(match.SourceFile, match.Location,
				tsdiag.Effect_provideServiceEffect_provides_this_service_from_the_acquiring_effect_directly_instead_of_Effect_0_forwarding_its_parameter_to_Effect_provideService_effect_flatMapProvideServiceToProvideServiceEffect, nil, match.Method)
		}
		return diagnostics
	},
}

// FlatMapProvideServiceToProvideServiceEffectMatch is a flatMap or andThen step
// whose callback only provides its parameter as a service:
//
//	outer: acquire... -> [Index] flatMap((p) => inner)
//	inner: self... -> [last] provideService(Key, p)
type FlatMapProvideServiceToProvideServiceEffectMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	Method     string
	Flow       *typeparser.PipingFlow
	Index      int
	Inner      *typeparser.PipingFlow
	Key        *ast.Node
}

// AnalyzeFlatMapProvideServiceToProvideServiceEffect finds flatMap and andThen
// callbacks that are a manual Effect.provideServiceEffect.
func AnalyzeFlatMapProvideServiceToProvideServiceEffect(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []FlatMapProvideServiceToProvideServiceEffectMatch {
	var matches []FlatMapProvideServiceToProvideServiceEffectMatch
	isFlatMapOrAndThen := func(step typeparser.PipingFlowTransformation) bool {
		callee := step.Callee()
		return callee != nil && callee.Flags&ast.NodeFlagsOptionalChain == 0 && step.Args().Len() == 1 &&
			(tp.IsNodeReferenceToEffectModuleApi(callee, "flatMap") || tp.IsNodeReferenceToEffectModuleApi(callee, "andThen"))
	}
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for _, sequence := range flow.FindTransformationSequences(isFlatMapOrAndThen).All() {
			i := sequence.Start()
			step := sequence.Transformations().At(0)
			callee := step.Callee()
			method := "flatMap"
			if tp.IsNodeReferenceToEffectModuleApi(callee, "andThen") {
				method = "andThen"
			}
			callback := typeparser.ParseLazyExpression(step.Args().At(0), typeparser.LazyExpressionNone)
			if callback == nil || callback.Params().Len() != 1 {
				continue
			}
			parameter := callback.Params().At(0).AsParameterDeclaration()
			if !plainTapParameter(parameter) {
				continue
			}
			parameterSymbol := tp.GetSymbolAtLocation(parameter.Name())
			if parameterSymbol == nil {
				continue
			}

			returned := ast.SkipParentheses(callback.Expression())
			inner := tp.LongestPipingFlowAt(returned, false)
			if inner == nil || inner.Node() != returned || inner.Transformations().Len() == 0 {
				continue
			}
			last := inner.Transformations().Len() - 1
			provide := inner.Transformations().At(last)
			if provide.Callee() == nil || provide.Callee().Flags&ast.NodeFlagsOptionalChain != 0 || provide.Args().Len() != 2 ||
				!tp.IsNodeReferenceToEffectModuleApi(provide.Callee(), "provideService") {
				continue
			}
			implementation := ast.SkipParentheses(provide.Args().At(1))
			if implementation.Kind != ast.KindIdentifier || tp.GetSymbolAtLocation(implementation) != parameterSymbol {
				continue
			}
			if mentionsCallbackBinding(tp, c, ast.SkipParentheses(step.Args().At(0)), returned, implementation) {
				continue
			}

			key := provide.Args().At(0)
			acquired := joinedStrictEffectChannels(tp, c, flow.TransformationInputType(i))
			if acquired == nil || joinedStrictEffectChannels(tp, c, inner.TransformationInputType(last)) == nil {
				continue
			}
			service := tp.ServiceTypeAtLocation(key)
			if service == nil {
				service = tp.ContextTagAtLocation(key)
			}
			if service == nil || service.Shape() == nil || !checker.Checker_isTypeAssignableTo(c, acquired.a, service.Shape()) {
				continue
			}
			if !stepTypeIdenticalToProvideServiceEffectResult(tp, c, step, provide, acquired) {
				continue
			}

			matches = append(matches, FlatMapProvideServiceToProvideServiceEffectMatch{
				SourceFile: sf,
				Location:   scanner.GetErrorRangeForNode(sf, callee),
				Method:     method,
				Flow:       flow,
				Index:      i,
				Inner:      inner,
				Key:        key,
			})
		}
	}
	return matches
}

func mentionsCallbackBinding(tp *typeparser.TypeParser, c *checker.Checker, callback, root, skip *ast.Node) bool {
	resolvesToCallbackBinding := func(symbol *ast.Symbol, node *ast.Node) bool {
		return declaredInCallbackOutsideRoot(symbol, callback, root) || isBuiltinArguments(symbol) && callbackIsThisContainer(callback, node)
	}
	var visit func(node *ast.Node) bool
	visit = func(node *ast.Node) bool {
		if node == skip {
			return false
		}
		switch node.Kind {
		case ast.KindShorthandPropertyAssignment:
			if resolvesToCallbackBinding(c.GetShorthandAssignmentValueSymbol(node), node) {
				return true
			}
		case ast.KindIdentifier:
			if resolvesToCallbackBinding(tp.GetSymbolAtLocation(node), node) {
				return true
			}
		case ast.KindThisKeyword, ast.KindSuperKeyword:
			if callbackIsThisContainer(callback, node) {
				return true
			}
		case ast.KindMetaProperty:
			if node.AsMetaProperty().KeywordToken == ast.KindNewKeyword && callbackIsThisContainer(callback, node) {
				return true
			}
		}
		//ets:ast-walk scans the returned expression, outside the service argument, for references to the callback's parameter, own name, this, arguments or new.target
		return node.ForEachChild(visit)
	}
	return visit(root)
}

func declaredInCallbackOutsideRoot(symbol *ast.Symbol, callback, root *ast.Node) bool {
	if symbol == nil {
		return false
	}
	for _, declaration := range ast.TSGetSymbolDeclarations(symbol) {
		if declaration.Pos() >= callback.Pos() && declaration.End() <= callback.End() &&
			(declaration.Pos() < root.Pos() || declaration.End() > root.End()) {
			return true
		}
	}
	return false
}

func callbackIsThisContainer(callback, node *ast.Node) bool {
	return ast.GetThisContainer(node, false, false) == callback
}

// isBuiltinArguments reports whether symbol is a function's implicit arguments
// object, which the checker creates without declarations.
func isBuiltinArguments(symbol *ast.Symbol) bool {
	return symbol != nil && ast.TSGetSymbolName(symbol) == "arguments" && len(ast.TSGetSymbolDeclarations(symbol)) == 0
}

func stepTypeIdenticalToProvideServiceEffectResult(tp *typeparser.TypeParser, c *checker.Checker, step, provide typeparser.PipingFlowTransformation, acquired *effectChannels) bool {
	if provide.TypeArguments() != nil {
		return false
	}
	stepChannels := joinedStrictEffectChannels(tp, c, step.OutType())
	provided := joinedStrictEffectChannels(tp, c, provide.OutType())
	if stepChannels == nil || provided == nil {
		return false
	}
	return checker.Checker_isTypeIdenticalTo(c, stepChannels.a, provided.a) &&
		checker.Checker_isTypeIdenticalTo(c, stepChannels.e, c.GetUnionType([]*checker.Type{acquired.e, provided.e})) &&
		checker.Checker_isTypeIdenticalTo(c, stepChannels.r, c.GetUnionType([]*checker.Type{acquired.r, provided.r}))
}

type effectChannels struct {
	a, e, r *checker.Type
}

func joinedStrictEffectChannels(tp *typeparser.TypeParser, c *checker.Checker, t *checker.Type) *effectChannels {
	if t == nil {
		return nil
	}
	if t.Flags()&checker.TypeFlagsUnion == 0 {
		effect := tp.StrictEffectType(t)
		if effect == nil {
			return nil
		}
		return &effectChannels{a: effect.A(), e: effect.E(), r: effect.R()}
	}
	members := t.Types()
	a := make([]*checker.Type, len(members))
	e := make([]*checker.Type, len(members))
	r := make([]*checker.Type, len(members))
	for i, member := range members {
		effect := tp.StrictEffectType(member)
		if effect == nil {
			return nil
		}
		a[i], e[i], r[i] = effect.A(), effect.E(), effect.R()
	}
	return &effectChannels{a: c.GetUnionType(a), e: c.GetUnionType(e), r: c.GetUnionType(r)}
}
