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

var FailCauseToConstructorFix = fixable.Fixable{
	Name:        "failCauseToConstructor",
	Description: "Replace with the direct Effect failure constructor",
	ErrorCodes: []int32{
		tsdiag.Effect_0_expresses_this_failure_directly_instead_of_wrapping_Cause_2_in_Effect_1_effect_failCauseToConstructor.Code(),
	},
	FixIDs: []string{"failCauseToConstructor_fix"},
	Run:    runFailCauseToConstructorFix,
}

func runFailCauseToConstructorFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile
	for _, match := range rules.AnalyzeFailCauseToConstructor(ctx.TypeParser, ctx.Checker, sf) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if match.Flow == nil || match.TransformationCount <= 0 {
			continue
		}
		failStep := match.Flow.Transformations().At(match.TransformationCount - 1)
		var edit func(tracker *rewriter.Tracker, callee *ast.Node)
		if match.Lazy == nil {
			edit = failCauseToConstructorEagerEdit(sf, match, failStep)
		} else {
			edit = failCauseToConstructorLazyEdit(ctx, match, failStep)
		}
		if edit == nil {
			continue
		}
		// The callee is rewritten through the file's Effect import, which must
		// reach the replacement API where the edit lands.
		target := importedModuleMember(ctx, failStep.Callee(), typeparser.FindEffectModuleReference(sf), effectModuleMember(ctx, match.ReplacementName))
		if target == nil {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect." + match.ReplacementName,
			Run: func(tracker *rewriter.Tracker) {
				edit(tracker, target.expression(tracker, effectModuleMember(ctx, match.ReplacementName)))
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}

// failCauseToConstructorEagerEdit replaces the flow prefix up to and including
// Effect.failCause with a call of the replacement on the Cause constructor's
// value. It returns nil when the value is not a standalone node or the
// failCause step is not applied by a call or a pipe.
func failCauseToConstructorEagerEdit(sf *ast.SourceFile, match rules.FailCauseToConstructorMatch, failStep typeparser.PipingFlowTransformation) func(*rewriter.Tracker, *ast.Node) {
	causeIndex := match.TransformationCount - 2
	value := match.Flow.TransformationInputNode(causeIndex)
	if value == nil || failStep.Kind() != typeparser.TransformationKindCall &&
		failStep.Kind() != typeparser.TransformationKindPipe &&
		failStep.Kind() != typeparser.TransformationKindPipeable {
		return nil
	}
	valueTypeArguments := match.Flow.Transformations().At(causeIndex).TypeArguments()
	return func(tracker *rewriter.Tracker, callee *ast.Node) {
		var typeArguments *ast.NodeList
		if valueTypeArguments != nil && len(valueTypeArguments.Nodes) > 0 {
			arguments := make([]*ast.Node, len(valueTypeArguments.Nodes))
			for i, argument := range valueTypeArguments.Nodes {
				arguments[i] = tracker.DeepCloneNode(argument)
			}
			typeArguments = tracker.NewNodeList(arguments)
		}
		replacement := tracker.NewCallExpression(
			callee,
			nil,
			typeArguments,
			tracker.NewNodeList([]*ast.Node{tracker.DeepCloneNode(value)}),
			ast.NodeFlagsNone,
		)
		tracker.ReplacePipingFlowPrefix(sf, match.Flow, match.TransformationCount, replacement)
	}
}

// failCauseToConstructorLazyEdit replaces the Cause constructor call returned
// by the failCauseSync thunk with its value and renames the failCauseSync
// callee. It returns nil when the thunk returns more than that call, the value
// is not a standalone node, the Cause call has type arguments, or failSync
// would infer another error type.
func failCauseToConstructorLazyEdit(ctx *fixable.Context, match rules.FailCauseToConstructorMatch, failStep typeparser.PipingFlowTransformation) func(*rewriter.Tracker, *ast.Node) {
	lastIndex := match.Inner.Transformations().Len() - 1
	last := match.Inner.Transformations().At(lastIndex)
	value := match.Inner.TransformationInputNode(lastIndex)
	body := ast.SkipParentheses(match.Lazy.Expression())
	if failStep.Kind() != typeparser.TransformationKindCall || rules.HasExplicitTypeArguments(&last) || value == nil ||
		body.Kind != ast.KindCallExpression || body.AsCallExpression().Expression != last.Callee() ||
		match.ConstructorName != "die" && !lazyFailKeepsErrorType(ctx.TypeParser, ctx.Checker, failStep, value) {
		return nil
	}
	expressionBody := !ast.IsBlock(match.Lazy.Body()) && match.Lazy.Expression() == body
	sf := ctx.SourceFile
	return func(tracker *rewriter.Tracker, callee *ast.Node) {
		replacement := tracker.DeepCloneNode(value)
		// An object literal as a concise arrow body would parse as a block.
		if expressionBody && ast.IsObjectLiteralExpression(ast.SkipParentheses(value)) {
			replacement = tracker.NewParenthesizedExpression(replacement)
		}
		tracker.ReplaceNode(sf, body, replacement, nil)
		tracker.ReplaceNode(sf, failStep.Callee(), callee, nil)
	}
}

// lazyFailKeepsErrorType reports whether Effect.failSync(() => value) infers the
// same error type as Effect.failCauseSync(() => Cause.fail(value)). Cause.fail
// infers its error from a call argument, which widens literal unions, while a
// failSync thunk returns the value against LazyArg<E>, which keeps them: for
// `cond ? "A" : "B"` the first infers string and the second "A" | "B". The fix
// is only offered when the value's own type already is the inferred error type.
func lazyFailKeepsErrorType(tp *typeparser.TypeParser, c *checker.Checker, failStep typeparser.PipingFlowTransformation, value *ast.Node) bool {
	effect := tp.EffectType(failStep.OutType())
	valueType := tp.GetTypeAtLocation(value)
	if effect == nil || effect.E() == nil || valueType == nil {
		return false
	}
	return checker.Checker_isTypeAssignableTo(c, valueType, effect.E()) &&
		checker.Checker_isTypeAssignableTo(c, effect.E(), valueType)
}
