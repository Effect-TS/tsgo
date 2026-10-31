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

var SleepThenEffectToDelayFix = fixable.Fixable{
	Name:        "sleepThenEffectToDelay",
	Description: "Rewrite to Effect.delay",
	ErrorCodes: []int32{
		tsdiag.Effect_sleep_sequenced_before_this_effect_re_implements_Effect_delay_Effect_delay_expresses_the_postponed_effect_directly_effect_sleepThenEffectToDelay.Code(),
	},
	FixIDs: []string{"sleepThenEffectToDelay_fix"},
	Run:    runSleepThenEffectToDelayFix,
}

func runSleepThenEffectToDelayFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile
	for _, match := range rules.AnalyzeSleepThenEffectToDelay(ctx.TypeParser, ctx.Checker, sf) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if !match.Fixable {
			continue
		}
		delay := effectModuleMemberCallee(ctx, match.Sequencer.Callee(), "delay")
		if delay == nil {
			continue
		}
		var suspend func(*rewriter.Tracker) *ast.Node
		if match.Form != rules.SleepThenEffectToDelayEffectValue && match.Form != rules.SleepThenEffectToDelayInlineResult {
			if suspend = effectModuleMemberCallee(ctx, match.Sequencer.Callee(), "suspend"); suspend == nil {
				continue
			}
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Rewrite to Effect.delay",
			Run: func(tracker *rewriter.Tracker) {
				delayed, original := sleepThenEffectToDelayEffect(tracker, match, suspend)
				switch match.Sequencer.Kind() {
				case typeparser.TransformationKindDataFirst, typeparser.TransformationKindCall:
					if original != nil {
						delayed = tracker.DeepCloneNode(original)
					}
					replacement := tracker.NewCallExpression(delay(tracker), nil, nil,
						tracker.NewNodeList([]*ast.Node{delayed, tracker.DeepCloneNode(match.Duration)}), ast.NodeFlagsNone)
					tracker.ReplacePipingFlowPrefix(sf, match.Flow, match.SleepIndex+2, replacement)
				case typeparser.TransformationKindPipe, typeparser.TransformationKindPipeable:
					// The delayed effect takes the place of the sleep call, and the
					// sequencer stage becomes Effect.delay(duration).
					if original != nil {
						sleepCall := match.Flow.Transformations().At(match.SleepIndex).Callee().Parent
						if delayed = subjectInCallPosition(tracker, sf, sleepCall, original); delayed == nil {
							return
						}
					}
					if !tracker.ReplacePipingFlowPrefix(sf, match.Flow, match.SleepIndex+1, delayed) {
						return
					}
					tracker.ReplacePipingFlowTransformation(sf, &match.Sequencer, rewriter.PipingFlowTransformationReplacement{
						Callee:    delay(tracker),
						Arguments: tracker.NewNodeList([]*ast.Node{tracker.DeepCloneNode(match.Duration)}),
					})
				}
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}

// sleepThenEffectToDelayEffect returns the effect to delay: either a source
// expression that callers clone for its new position, or a synthesized
// Effect.suspend call that keeps the continuation lazy.
func sleepThenEffectToDelayEffect(tracker *rewriter.Tracker, match rules.SleepThenEffectToDelayMatch, suspend func(*rewriter.Tracker) *ast.Node) (synthesized *ast.Node, original *ast.Node) {
	var lazy *ast.Node
	switch match.Form {
	case rules.SleepThenEffectToDelayEffectValue:
		return nil, match.Continuation
	case rules.SleepThenEffectToDelayInlineResult:
		return nil, match.Lazy.Expression()
	case rules.SleepThenEffectToDelaySuspendThunkDropParam:
		// Effect.suspend takes a LazyArg, which a one-parameter arrow is not.
		arrow := ast.SkipParentheses(match.Continuation).AsArrowFunction()
		var returnType *ast.Node
		if arrow.Type != nil {
			returnType = tracker.DeepCloneNode(arrow.Type)
		}
		lazy = tracker.NewArrowFunction(nil, nil, tracker.NewNodeList(nil), returnType, nil,
			tracker.NewToken(ast.KindEqualsGreaterThanToken), tracker.DeepCloneNode(arrow.Body))
	default:
		lazy = tracker.DeepCloneNode(match.Continuation)
	}
	return tracker.NewCallExpression(suspend(tracker), nil, nil, tracker.NewNodeList([]*ast.Node{lazy}), ast.NodeFlagsNone), nil
}

// effectModuleMemberCallee returns a builder for a reference to the Effect
// module member name at location. The qualifier of the callee at location is
// kept when it is the Effect module; otherwise the file's Effect import, or an
// import binding in scope, is used. It returns nil when nothing reaches name.
func effectModuleMemberCallee(ctx *fixable.Context, callee *ast.Node, name string) func(*rewriter.Tracker) *ast.Node {
	member := effectModuleMember(ctx, name)
	if ast.IsPropertyAccessExpression(callee) {
		expression := callee.AsPropertyAccessExpression().Expression
		if ctx.TypeParser.IsExpressionEffectModule(expression) && typeHasMember(ctx, ctx.TypeParser.GetTypeAtLocation(expression), member) {
			return func(tracker *rewriter.Tracker) *ast.Node {
				return tracker.NewPropertyAccessExpression(tracker.DeepCloneNode(expression), nil, tracker.NewIdentifier(name), ast.NodeFlagsNone)
			}
		}
	}
	reference := resolveModuleMember(ctx, callee, typeparser.FindEffectModuleReference(ctx.SourceFile), member)
	if reference == nil {
		return nil
	}
	return func(tracker *rewriter.Tracker) *ast.Node { return reference.expression(tracker, member) }
}
