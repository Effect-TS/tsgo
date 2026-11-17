package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var PromiseRaceInEffectFix = fixable.Fixable{
	Name:        "promiseRaceInEffect",
	Description: "Race Effect values with Effect.raceAll or Effect.raceAllFirst instead of racing promises",
	ErrorCodes: []int32{
		tsdiag.This_Effect_code_races_promises_with_Promise_0_the_losing_promises_keep_running_and_are_not_interrupted_Use_Effect_1_over_Effect_values_instead_effect_promiseRaceInEffect.Code(),
	},
	FixIDs: []string{"promiseRaceInEffect_fix"},
	Run:    runPromiseRaceInEffectFix,
}

type promiseRaceInEffectFixPlan struct {
	outer       *typeparser.PipingFlow
	thunk       *ast.Node
	callback    *typeparser.ParsedLazyExpression
	receiver    *ast.Node
	mapName     *ast.Node
	race        moduleMember
	raceRef     *moduleMemberReference
	constructor moduleMember
	ctorRef     *moduleMemberReference
}

func runPromiseRaceInEffectFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzePromiseRaceInEffect(ctx.TypeParser, ctx.Checker, ctx.SourceFile) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		plan := promiseRaceInEffectPlan(ctx, match)
		if plan == nil {
			continue
		}
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect." + plan.race.name,
			Run: func(tracker *rewriter.Tracker) {
				tracker.ReplacePipingFlowPrefix(match.SourceFile, plan.outer, 1, promiseRaceInEffectReplacement(tracker, plan))
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}
	return nil
}

func promiseRaceInEffectPlan(ctx *fixable.Context, match rules.PromiseRaceInEffectMatch) *promiseRaceInEffectFixPlan {
	tp := ctx.TypeParser
	// Effect v4 raceAll never settles on an empty list, while Promise.any([])
	// rejects, so only v3 any and v4 race have a matching replacement.
	var race string
	switch {
	case match.Method == "race" && tp.SupportedEffectVersion() == typeparser.EffectMajorV4:
		race = "raceAllFirst"
	case match.Method == "any" && tp.SupportedEffectVersion() == typeparser.EffectMajorV3:
		race = "raceAll"
	default:
		return nil
	}

	var callback *typeparser.ParsedLazyExpression
	if match.Index != 1 || !match.Flow.MatchesExactly(
		func(subject typeparser.PipingFlowSubject) bool {
			callback = typeparser.ParseLazyExpression(subject.Node(), typeparser.LazyExpressionAllowMultiStatementBlock)
			return callback != nil
		},
		func(step typeparser.PipingFlowTransformation) bool {
			return promiseRaceInEffectIsUntypedNativeMapStep(tp, step)
		},
		promiseRaceInEffectIsBarePromiseMemberStep,
	) {
		return nil
	}
	mapCallee := match.Flow.Transformations().At(0).Callee()

	if !promiseRaceInEffectItemsArePromises(tp, match.Flow.TransformationInputType(1)) ||
		promiseRaceInEffectIsCallbackNameReferenced(tp, callback) ||
		!promiseRaceInEffectParametersBindWithoutRunningCode(callback.Node().Parameters()) {
		return nil
	}

	thunk := promiseRaceInEffectThunk(match.Flow.Node())
	if thunk == nil {
		return nil
	}

	outer, constructor := promiseRaceInEffectConstructorFlow(tp, match.SourceFile, thunk)
	if outer == nil {
		return nil
	}

	receiver := mapCallee.AsPropertyAccessExpression().Expression
	if !tp.IsExpressionValueStableAtLocation(receiver, outer.Node()) ||
		!promiseRaceInEffectThunkParametersStayInBody(ctx, thunk, match.Flow.Node(), callback.Body()) {
		return nil
	}

	effectModule := typeparser.FindEffectModuleReference(match.SourceFile)
	raceMember := effectModuleMember(ctx, race)
	raceRef := resolveModuleMember(ctx, outer.Node(), effectModule, raceMember)
	constructorMember := effectModuleMember(ctx, constructor)
	ctorRef := resolveModuleMember(ctx, callback.Body(), effectModule, constructorMember)
	if raceRef == nil || ctorRef == nil {
		return nil
	}

	return &promiseRaceInEffectFixPlan{
		outer:       outer,
		thunk:       thunk,
		callback:    callback,
		receiver:    receiver,
		mapName:     mapCallee.Name(),
		race:        raceMember,
		raceRef:     raceRef,
		constructor: constructorMember,
		ctorRef:     ctorRef,
	}
}

func promiseRaceInEffectIsUntypedNativeMapStep(tp *typeparser.TypeParser, step typeparser.PipingFlowTransformation) bool {
	callee := step.Callee()
	return step.Args().Len() == 0 && step.TypeArguments() == nil &&
		ast.IsPropertyAccessExpression(callee) && tp.IsNativeArrayMethodReference(callee, "map")
}

func promiseRaceInEffectIsBarePromiseMemberStep(step typeparser.PipingFlowTransformation) bool {
	callee := step.Callee()
	return ast.IsPropertyAccessExpression(callee) && ast.IsIdentifier(callee.Expression())
}

func promiseRaceInEffectItemsArePromises(tp *typeparser.TypeParser, listType *checker.Type) bool {
	itemType := tp.ArrayType(listType)
	if itemType == nil {
		return false
	}
	for _, member := range tp.UnrollUnionMembers(itemType) {
		if tp.PromiseType(member) == nil {
			return false
		}
	}
	return true
}

func promiseRaceInEffectIsCallbackNameReferenced(tp *typeparser.TypeParser, callback *typeparser.ParsedLazyExpression) bool {
	name := callback.Node().Name()
	if name == nil {
		return false
	}
	for _, reference := range tp.ReferencesToSymbol(callback.Node(), tp.GetSymbolAtLocation(name)).All() {
		if reference != name {
			return true
		}
	}
	return false
}

func promiseRaceInEffectParametersBindWithoutRunningCode(parameters []*ast.Node) bool {
	for _, parameter := range parameters {
		if name := parameter.Name(); name == nil || name.Kind != ast.KindIdentifier || parameter.Initializer() != nil {
			return false
		}
	}
	return true
}

func promiseRaceInEffectThunk(raceCall *ast.Node) *ast.Node {
	thunkChild := raceCall
	for thunkChild.Parent != nil && thunkChild.Parent.Kind == ast.KindParenthesizedExpression {
		thunkChild = thunkChild.Parent
	}
	thunk := thunkChild.Parent
	if thunk == nil || thunk.Kind != ast.KindArrowFunction ||
		typeparser.ParseLazyExpression(thunk, typeparser.LazyExpressionNone) == nil ||
		thunk.Body().Kind == ast.KindBlock || ast.SkipParentheses(thunk.Body()) != raceCall {
		return nil
	}
	return thunk
}

func promiseRaceInEffectConstructorFlow(tp *typeparser.TypeParser, sf *ast.SourceFile, thunk *ast.Node) (*typeparser.PipingFlow, string) {
	var constructor string
	for _, flow := range tp.PipingFlows(sf, true).All() {
		if flow.MatchesPrefix(
			func(subject typeparser.PipingFlowSubject) bool { return subject.Node() == thunk },
			func(step typeparser.PipingFlowTransformation) bool {
				if step.Args().Len() != 0 {
					return false
				}
				for _, name := range []string{"promise", "tryPromise"} {
					if tp.IsNodeReferenceToEffectModuleApi(step.Callee(), name) {
						constructor = name
						return true
					}
				}
				return false
			},
		) {
			return flow, constructor
		}
	}
	return nil, ""
}

func promiseRaceInEffectThunkParametersStayInBody(ctx *fixable.Context, thunk *ast.Node, raceCall *ast.Node, body *ast.Node) bool {
	tp := ctx.TypeParser
	for _, parameter := range thunk.Parameters() {
		name := parameter.Name()
		if name == nil || name.Kind != ast.KindIdentifier {
			return false
		}
		symbol := tp.GetSymbolAtLocation(name)
		if symbol == nil || ctx.Checker.ResolveName(name.Text(), body, ast.SymbolFlagsValue, false) != symbol {
			return false
		}
		for _, reference := range tp.ReferencesToSymbol(raceCall, symbol).All() {
			if reference.Pos() < body.Pos() || reference.End() > body.End() {
				return false
			}
		}
	}
	return true
}

func promiseRaceInEffectReplacement(tracker *rewriter.Tracker, plan *promiseRaceInEffectFixPlan) *ast.Node {
	var returnType *ast.Node
	if original := plan.callback.Node().Type(); original != nil {
		returnType = tracker.DeepCloneNode(original)
	}
	perItemThunk := tracker.NewArrowFunction(
		nil,
		nil,
		promiseRaceInEffectCloneNodes(tracker, plan.thunk.Parameters()),
		returnType,
		nil,
		tracker.NewToken(ast.KindEqualsGreaterThanToken),
		tracker.DeepCloneNode(plan.callback.Body()),
	)
	item := tracker.NewCallExpression(
		plan.ctorRef.expression(tracker, plan.constructor),
		nil,
		nil,
		tracker.NewNodeList([]*ast.Node{perItemThunk}),
		ast.NodeFlagsNone,
	)
	parameters := promiseRaceInEffectCloneNodes(tracker, plan.callback.Node().Parameters())
	var callback *ast.Node
	if plan.callback.Node().Kind == ast.KindFunctionExpression {
		var name *ast.Node
		if original := plan.callback.Node().Name(); original != nil {
			name = tracker.DeepCloneNode(original)
		}
		callback = tracker.NewFunctionExpression(
			nil,
			nil,
			name,
			nil,
			parameters,
			nil,
			nil,
			tracker.NewBlock(tracker.NewNodeList([]*ast.Node{tracker.NewReturnStatement(item)}), true),
		)
	} else {
		callback = tracker.NewArrowFunction(nil, nil, parameters, nil, nil, tracker.NewToken(ast.KindEqualsGreaterThanToken), item)
	}
	mapCall := tracker.NewCallExpression(
		tracker.NewPropertyAccessExpression(tracker.DeepCloneNode(plan.receiver), nil, tracker.DeepCloneNode(plan.mapName), ast.NodeFlagsNone),
		nil,
		nil,
		tracker.NewNodeList([]*ast.Node{callback}),
		ast.NodeFlagsNone,
	)
	return tracker.NewCallExpression(
		plan.raceRef.expression(tracker, plan.race),
		nil,
		nil,
		tracker.NewNodeList([]*ast.Node{mapCall}),
		ast.NodeFlagsNone,
	)
}

func promiseRaceInEffectCloneNodes(tracker *rewriter.Tracker, nodes []*ast.Node) *ast.NodeList {
	clones := make([]*ast.Node, 0, len(nodes))
	for _, node := range nodes {
		clones = append(clones, tracker.DeepCloneNode(node))
	}
	return tracker.NewNodeList(clones)
}
