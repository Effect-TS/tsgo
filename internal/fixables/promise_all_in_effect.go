package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var PromiseAllInEffectFix = fixable.Fixable{
	Name:        "promiseAllInEffect",
	Description: "Replace with Effect.forEach",
	ErrorCodes: []int32{
		tsdiag.X_0_starts_every_promise_at_once_with_no_concurrency_bound_and_they_keep_running_when_the_fiber_is_interrupted_Use_1_with_a_concurrency_option_instead_effect_promiseAllInEffect.Code(),
	},
	FixIDs: []string{"promiseAllInEffect_fix"},
	Run:    runPromiseAllInEffectFix,
}

// Only Promise.all is fixed: the
// success type of Promise.allSettled differs from any Effect.all mode.
func runPromiseAllInEffectFix(ctx *fixable.Context) []ls.CodeAction {
	tp := ctx.TypeParser
	sf := ctx.SourceFile

	reported := map[*ast.Node]bool{}
	for _, match := range rules.AnalyzePromiseAllInEffect(tp, ctx.Checker, sf) {
		if match.Method != rules.PromiseMethodAll || !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		reported[match.Flow.Transformations().At(match.Index).Callee()] = true
	}
	if len(reported) == 0 {
		return nil
	}

	noTypeArguments := func(transformation typeparser.PipingFlowTransformation) bool {
		return transformation.TypeArguments() == nil || len(transformation.TypeArguments().Nodes) == 0
	}
	isSyncConciseUnannotatedThunk := func(subject typeparser.PipingFlowSubject) bool {
		node := ast.SkipParentheses(subject.Node())
		if node == nil || node.Kind != ast.KindArrowFunction {
			return false
		}
		thunk := typeparser.ParseLazyExpression(node, typeparser.LazyExpressionNone)
		return thunk != nil && !ast.IsBlock(thunk.Body()) && thunk.Params().Len() <= 1 && node.Type() == nil
	}
	isPromiseConstructor := func(transformation typeparser.PipingFlowTransformation) bool {
		return transformation.Kind() == typeparser.TransformationKindCall && transformation.Args().Len() == 0 &&
			noTypeArguments(transformation) &&
			(tp.IsNodeReferenceToEffectModuleApi(transformation.Callee(), "promise") ||
				tp.IsNodeReferenceToEffectModuleApi(transformation.Callee(), "tryPromise"))
	}
	// Effect.forEach passes (item, index), so the callback may declare at most
	// two plain parameters. A `this` pseudo-parameter has no arrow form.
	isInlineCallback := func(subject typeparser.PipingFlowSubject) bool {
		node := ast.SkipParentheses(subject.Node())
		if node == nil || node.Kind != ast.KindArrowFunction && node.Kind != ast.KindFunctionExpression {
			return false
		}
		if node.Kind == ast.KindFunctionExpression && node.AsFunctionExpression().AsteriskToken != nil {
			return false
		}
		if typeParameters := node.TypeParameters(); len(typeParameters) > 0 {
			return false
		}
		parameters := node.Parameters()
		if len(parameters) > 2 {
			return false
		}
		for _, parameter := range parameters {
			if parameter.AsParameterDeclaration().DotDotDotToken != nil || ast.IsThisParameter(parameter) {
				return false
			}
		}
		return true
	}
	isArrayMap := func(transformation typeparser.PipingFlowTransformation) bool {
		callee := transformation.Callee()
		return transformation.Kind() == typeparser.TransformationKindCall && transformation.Args().Len() == 0 &&
			noTypeArguments(transformation) &&
			callee != nil && ast.IsPropertyAccessExpression(callee) &&
			callee.AsPropertyAccessExpression().QuestionDotToken == nil &&
			callee.Name() != nil && callee.Name().Text() == "map" &&
			tp.ArrayType(tp.GetTypeAtLocation(callee.Expression())) != nil
	}
	isReportedPromiseAll := func(transformation typeparser.PipingFlowTransformation) bool {
		return transformation.Kind() == typeparser.TransformationKindCall && noTypeArguments(transformation) &&
			reported[transformation.Callee()]
	}

	for _, outer := range tp.PipingFlows(sf, true).All() {
		if !outer.MatchesPrefix(isSyncConciseUnannotatedThunk, isPromiseConstructor) {
			continue
		}
		thunk := ast.SkipParentheses(outer.Subject().Node())
		body := thunk.Body()
		inner := tp.LongestPipingFlowAt(body, true)
		if inner == nil || inner.Node() != ast.SkipParentheses(body) ||
			!inner.MatchesExactly(isInlineCallback, isArrayMap, isReportedPromiseAll) {
			continue
		}
		callback := ast.SkipParentheses(inner.Subject().Node())
		mapStep := inner.Transformations().At(0)
		receiver := mapStep.Callee().AsPropertyAccessExpression().Expression
		// Each callback result becomes the per-item thunk's result, which
		// Effect.promise and Effect.tryPromise require to be a promise;
		// Promise.all also accepts plain values and unions with them.
		if tp.PromiseType(tp.ArrayType(mapStep.OutType())) == nil {
			continue
		}
		constructorCallee := outer.Transformations().At(0).Callee()
		if !promiseAllCallbackMovesSafely(ctx, thunk, callback, receiver, constructorCallee) {
			continue
		}

		member := effectModuleMember(ctx, "forEach")
		var qualifier *ast.Node
		var target *moduleMemberReference
		if ast.IsPropertyAccessExpression(constructorCallee) {
			expression := constructorCallee.AsPropertyAccessExpression().Expression
			if tp.IsExpressionEffectModule(expression) && typeHasMember(ctx, tp.GetTypeAtLocation(expression), member) {
				qualifier = expression
			}
		}
		if qualifier == nil {
			target = importedModuleMember(ctx, constructorCallee, typeparser.FindEffectModuleReference(sf), member)
			if target == nil {
				continue
			}
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.forEach",
			Run: func(tracker *rewriter.Tracker) {
				var forEach *ast.Node
				if qualifier != nil {
					forEach = tracker.NewPropertyAccessExpression(tracker.DeepCloneNode(qualifier), nil, tracker.NewIdentifier("forEach"), ast.NodeFlagsNone)
				} else {
					forEach = target.expression(tracker, member)
				}
				replacement := tracker.NewCallExpression(
					forEach,
					nil,
					nil,
					tracker.NewNodeList([]*ast.Node{
						tracker.DeepCloneNode(receiver),
						newPromiseAllItemCallback(tracker, thunk, callback, constructorCallee),
						tracker.NewObjectLiteralExpression(tracker.NewNodeList([]*ast.Node{
							tracker.NewPropertyAssignment(nil, tracker.NewIdentifier("concurrency"), nil, nil, tracker.NewStringLiteral("unbounded", ast.TokenFlagsNone)),
						}), false),
					}),
					ast.NodeFlagsNone,
				)
				tracker.ReplacePipingFlowPrefix(sf, outer, 1, replacement)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}

func promiseAllCallbackMovesSafely(ctx *fixable.Context, thunk *ast.Node, callback *ast.Node, receiver *ast.Node, constructorCallee *ast.Node) bool {
	return !copiesFunctionOwnBindings(ctx, callback) &&
		!callbackShadowsConstructorRoot(ctx, callback, constructorCallee) &&
		!callbackShadowsNamesUsedByThunkParameters(ctx, thunk, callback) &&
		!thunkParametersCapturedOrShadowed(ctx, thunk, callback, receiver)
}

func copiesFunctionOwnBindings(ctx *fixable.Context, callback *ast.Node) bool {
	copied := append(append([]*ast.Node{}, callback.Parameters()...), callback.Type(), callback.Body())
	for _, part := range copied {
		if part != nil && ctx.TypeParser.UsesFunctionOwnBindings(callback, part) {
			return true
		}
	}
	return false
}

func shadowedOrUnresolvedInCallback(ctx *fixable.Context, callback *ast.Node, name string, symbol *ast.Symbol) bool {
	return symbol == nil || ctx.Checker.ResolveName(name, callback, ast.SymbolFlagsValue|ast.SymbolFlagsAlias, false) != symbol
}

func callbackShadowsConstructorRoot(ctx *fixable.Context, callback *ast.Node, constructorCallee *ast.Node) bool {
	root := constructorCallee
	for ast.IsPropertyAccessExpression(root) {
		root = root.AsPropertyAccessExpression().Expression
	}
	return !ast.IsIdentifier(root) || shadowedOrUnresolvedInCallback(ctx, callback, root.Text(), ctx.TypeParser.GetSymbolAtLocation(root))
}

func callbackShadowsNamesUsedByThunkParameters(ctx *fixable.Context, thunk *ast.Node, callback *ast.Node) bool {
	for name, local := range ast.TSGetSymbolTableEntries(callback.Locals()) {
		outer := ctx.Checker.ResolveName(name, thunk, ast.SymbolFlagsAll, false)
		if outer == nil || outer == local {
			continue
		}
		for _, thunkParameter := range thunk.Parameters() {
			if ctx.TypeParser.ReferencesToSymbol(thunkParameter, outer).Len() > 0 {
				return true
			}
		}
	}
	return false
}

func thunkParametersCapturedOrShadowed(ctx *fixable.Context, thunk *ast.Node, callback *ast.Node, receiver *ast.Node) bool {
	tp := ctx.TypeParser
	for _, thunkParameter := range thunk.Parameters() {
		name := thunkParameter.Name()
		if name == nil || !ast.IsIdentifier(name) {
			return true
		}
		symbol := tp.GetSymbolAtLocation(name)
		if tp.ReferencesToSymbol(receiver, symbol).Len() > 0 || shadowedOrUnresolvedInCallback(ctx, callback, name.Text(), symbol) {
			return true
		}
		for _, callbackParameter := range callback.Parameters() {
			if tp.ReferencesToSymbol(callbackParameter, symbol).Len() > 0 {
				return true
			}
		}
	}
	return false
}

func newPromiseAllItemCallback(tracker *rewriter.Tracker, thunk *ast.Node, callback *ast.Node, constructorCallee *ast.Node) *ast.Node {
	var modifiers *ast.ModifierList
	if ast.GetCombinedModifierFlags(callback)&ast.ModifierFlagsAsync != 0 {
		modifiers = tracker.NewModifierList([]*ast.Node{tracker.NewModifier(ast.KindAsyncKeyword)})
	}
	itemThunk := tracker.NewArrowFunction(
		modifiers,
		nil,
		tracker.NewNodeList(cloneNodes(tracker, thunk.Parameters())),
		tracker.DeepCloneNode(callback.Type()),
		nil,
		tracker.NewToken(ast.KindEqualsGreaterThanToken),
		tracker.DeepCloneNode(callback.Body()),
	)
	itemEffect := tracker.NewCallExpression(
		tracker.DeepCloneNode(constructorCallee),
		nil,
		nil,
		tracker.NewNodeList([]*ast.Node{itemThunk}),
		ast.NodeFlagsNone,
	)
	return tracker.NewArrowFunction(
		nil,
		nil,
		tracker.NewNodeList(cloneNodes(tracker, callback.Parameters())),
		nil,
		nil,
		tracker.NewToken(ast.KindEqualsGreaterThanToken),
		itemEffect,
	)
}

func cloneNodes(tracker *rewriter.Tracker, nodes []*ast.Node) []*ast.Node {
	clones := make([]*ast.Node, len(nodes))
	for i, node := range nodes {
		clones[i] = tracker.DeepCloneNode(node)
	}
	return clones
}
