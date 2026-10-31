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

// SleepThenEffectToDelay suggests Effect.delay when Effect.sleep is sequenced
// in front of an effect, which is how Effect.delay itself is implemented.
var SleepThenEffectToDelay = rule.Rule{
	Name:            "sleepThenEffectToDelay",
	Group:           "style",
	Description:     "Suggests Effect.delay instead of sequencing an effect after Effect.sleep",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_sleep_sequenced_before_this_effect_re_implements_Effect_delay_Effect_delay_expresses_the_postponed_effect_directly_effect_sleepThenEffectToDelay.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeSleepThenEffectToDelay(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_sleep_sequenced_before_this_effect_re_implements_Effect_delay_Effect_delay_expresses_the_postponed_effect_directly_effect_sleepThenEffectToDelay,
				nil,
			)
		}
		return diagnostics
	},
}

// SleepThenEffectToDelayForm describes how the continuation becomes the
// delayed effect.
type SleepThenEffectToDelayForm uint8

const (
	// SleepThenEffectToDelayEffectValue: the continuation is the Effect itself.
	SleepThenEffectToDelayEffectValue SleepThenEffectToDelayForm = iota + 1
	// SleepThenEffectToDelayInlineResult: a thunk returns a value that is stable
	// at the sequencer, so the returned expression is used directly.
	SleepThenEffectToDelayInlineResult
	// SleepThenEffectToDelaySuspendThunk: a zero-parameter function builds the
	// Effect, so it is kept lazy with Effect.suspend.
	SleepThenEffectToDelaySuspendThunk
	// SleepThenEffectToDelaySuspendThunkDropParam: an arrow with one unused
	// parameter builds the Effect; Effect.suspend needs it without the parameter.
	SleepThenEffectToDelaySuspendThunkDropParam
	// SleepThenEffectToDelaySuspendReference: a reference to a zero-parameter
	// function returning an Effect.
	SleepThenEffectToDelaySuspendReference
)

// SleepThenEffectToDelayMatch holds the nodes needed by the diagnostic and quick fix.
type SleepThenEffectToDelayMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	Flow       *typeparser.PipingFlow
	SleepIndex int
	Sequencer  typeparser.PipingFlowTransformation
	// Duration is the argument of the Effect.sleep call.
	Duration *ast.Node
	// Continuation is the sequencer argument as written.
	Continuation *ast.Node
	// Lazy is the parsed continuation for SleepThenEffectToDelayInlineResult.
	Lazy    *typeparser.ParsedLazyExpression
	Form    SleepThenEffectToDelayForm
	Fixable bool
}

// AnalyzeSleepThenEffectToDelay finds piping flows in which a direct
// Effect.sleep call is immediately sequenced with Effect.andThen,
// Effect.flatMap or Effect.zipRight into a continuation whose result is an
// Effect that ignores the sleep's void result.
func AnalyzeSleepThenEffectToDelay(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []SleepThenEffectToDelayMatch {
	isSleep := func(step typeparser.PipingFlowTransformation) bool {
		return step.Kind() == typeparser.TransformationKindCall && step.Args().Len() == 0 && step.Callee() != nil &&
			tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "sleep")
	}
	isSequencer := func(step typeparser.PipingFlowTransformation) bool {
		// A single argument also rules out v3 zipRight options, which may run
		// both effects concurrently.
		if step.Callee() == nil || step.Args().Len() != 1 || step.Args().At(0).Kind == ast.KindSpreadElement {
			return false
		}
		return tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "andThen") ||
			tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "flatMap") ||
			tp.IsNodeReferenceToEffectModuleApi(step.Callee(), "zipRight")
	}

	var matches []SleepThenEffectToDelayMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for _, sequence := range flow.FindTransformationSequences(isSleep, isSequencer).All() {
			index := sequence.Start()
			sleep := sequence.Transformations().At(0)
			sequencer := sequence.Transformations().At(1)
			duration := flow.TransformationInputNode(index)
			if duration == nil {
				continue
			}
			continuation := sequencer.Args().At(0)
			form, lazy := classifySleepContinuation(tp, c, continuation, sequencer.Callee())
			if form == 0 {
				continue
			}
			matches = append(matches, SleepThenEffectToDelayMatch{
				SourceFile:   sf,
				Location:     scanner.GetErrorRangeForNode(sf, sleep.Callee()),
				Flow:         flow,
				SleepIndex:   index,
				Sequencer:    sequencer,
				Duration:     duration,
				Continuation: continuation,
				Lazy:         lazy,
				Form:         form,
				Fixable: !hasExplicitTypeArguments(&sequencer) &&
					sleepThenEffectToDelayKeepsInvocation(tp, c, form, continuation) &&
					sleepThenEffectToDelayKeepsOrder(tp, sequencer, form, continuation, duration),
			})
		}
	}
	return matches
}

// sleepThenEffectToDelayKeepsOrder reports whether Effect.delay(k, d) keeps
// what the continuation and the duration evaluate to. Only a curried
// sequencer, `andThen(k)(sleep(d))`, already evaluates k first; every other
// form evaluates d, and sleep reads its value, before k. The rewrite is
// therefore safe only when evaluating k cannot change anything, as for
// function literals and stable values (the returned value of InlineResult is
// stable by construction), or when d is a stable primitive, whose value k
// cannot change. A stable binding of an object, such as a tuple, can still be
// mutated by k.
func sleepThenEffectToDelayKeepsOrder(tp *typeparser.TypeParser, sequencer typeparser.PipingFlowTransformation, form SleepThenEffectToDelayForm, continuation *ast.Node, duration *ast.Node) bool {
	if sequencer.Kind() == typeparser.TransformationKindCall {
		return true
	}
	switch form {
	case SleepThenEffectToDelayInlineResult, SleepThenEffectToDelaySuspendThunk, SleepThenEffectToDelaySuspendThunkDropParam:
		return true
	}
	location := sequencer.Callee()
	return tp.IsExpressionValueStableAtLocation(continuation, location) ||
		tp.IsExpressionValueStableAtLocation(duration, location) && isPrimitiveTyped(tp, duration)
}

// isPrimitiveTyped reports whether every value of node's type is a string,
// number, bigint or boolean.
func isPrimitiveTyped(tp *typeparser.TypeParser, node *ast.Node) bool {
	t := tp.GetTypeAtLocation(node)
	if t == nil {
		return false
	}
	for _, member := range tp.UnrollUnionMembers(t) {
		if member.Flags()&(checker.TypeFlagsStringLike|checker.TypeFlagsNumberLike|checker.TypeFlagsBigIntLike|checker.TypeFlagsBooleanLike) == 0 {
			return false
		}
	}
	return true
}

// sleepThenEffectToDelayKeepsInvocation reports whether passing the
// continuation to Effect.suspend calls it as the sequencer did. The sequencer
// calls it with the sleep result, Effect.suspend with no arguments, which a
// non-arrow function can observe through `arguments` or `this` even without
// declared parameters. Inline arrows qualify, and so do references to a const
// binding initialized with an arrow; other references may be bound to any
// function.
func sleepThenEffectToDelayKeepsInvocation(tp *typeparser.TypeParser, c *checker.Checker, form SleepThenEffectToDelayForm, continuation *ast.Node) bool {
	switch form {
	case SleepThenEffectToDelaySuspendThunk:
		return ast.SkipParentheses(continuation).Kind == ast.KindArrowFunction
	case SleepThenEffectToDelaySuspendReference:
		reference := ast.SkipParentheses(continuation)
		if reference.Kind != ast.KindIdentifier {
			return false
		}
		symbol := tp.GetSymbolAtLocation(reference)
		if symbol == nil || !checker.Checker_isConstantVariable(c, symbol) {
			return false
		}
		declaration := ast.TSGetSymbolValueDeclaration(symbol)
		return declaration != nil && ast.IsVariableDeclaration(declaration) && declaration.Initializer() != nil &&
			ast.SkipParentheses(declaration.Initializer()).Kind == ast.KindArrowFunction
	}
	return true
}

// classifySleepContinuation returns how continuation becomes the delayed
// effect, or zero when it is not an Effect continuation that ignores its input.
func classifySleepContinuation(tp *typeparser.TypeParser, c *checker.Checker, continuation *ast.Node, location *ast.Node) (SleepThenEffectToDelayForm, *typeparser.ParsedLazyExpression) {
	fn := ast.SkipParentheses(continuation)
	switch fn.Kind {
	case ast.KindArrowFunction, ast.KindFunctionExpression:
		if !continuationFunctionIgnoresInput(tp, c, fn) || !returnsSingleStrictEffect(tp, c, tp.GetTypeAtLocation(fn), false) {
			return 0, nil
		}
		// Statements before the return run after the sleep, so only a lone
		// returned value can be inlined.
		if lazy := typeparser.ParseLazyExpression(fn, typeparser.LazyExpressionNone); lazy != nil &&
			tp.IsExpressionValueStableAtLocation(lazy.Expression(), location) {
			return SleepThenEffectToDelayInlineResult, lazy
		}
		if params := typeparser.GetFunctionLikeParameters(fn); params != nil && len(params.Nodes) > 0 {
			return SleepThenEffectToDelaySuspendThunkDropParam, nil
		}
		return SleepThenEffectToDelaySuspendThunk, nil
	}
	if tp.StrictEffectTypeAtLocation(continuation) != nil {
		return SleepThenEffectToDelayEffectValue, nil
	}
	if returnsSingleStrictEffect(tp, c, tp.GetTypeAtLocation(continuation), true) {
		return SleepThenEffectToDelaySuspendReference, nil
	}
	return 0, nil
}

// continuationFunctionIgnoresInput accepts arrows that never read their
// parameter and synchronous, non-generic function expressions without
// parameters. Such a function expression still receives the void sleep result
// in its arguments object, so sleepThenEffectToDelayKeepsInvocation withholds
// fixes that would call it differently.
func continuationFunctionIgnoresInput(tp *typeparser.TypeParser, c *checker.Checker, fn *ast.Node) bool {
	if fn.Kind == ast.KindArrowFunction {
		return arrowIgnoresParameter(tp, c, fn)
	}
	expression := fn.AsFunctionExpression()
	if expression.AsteriskToken != nil || ast.GetCombinedModifierFlags(fn)&ast.ModifierFlagsAsync != 0 {
		return false
	}
	if params := typeparser.GetFunctionLikeTypeParameters(fn); params != nil && len(params.Nodes) != 0 {
		return false
	}
	params := typeparser.GetFunctionLikeParameters(fn)
	return params == nil || len(params.Nodes) == 0
}

// returnsSingleStrictEffect reports whether t has exactly one call signature,
// without parameters when requireNoParameters is set, that returns an Effect.
func returnsSingleStrictEffect(tp *typeparser.TypeParser, c *checker.Checker, t *checker.Type, requireNoParameters bool) bool {
	if t == nil {
		return false
	}
	signatures := c.GetSignaturesOfType(t, checker.SignatureKindCall)
	if len(signatures) != 1 || requireNoParameters && len(signatures[0].Parameters()) != 0 {
		return false
	}
	return tp.StrictEffectType(c.GetReturnTypeOfSignature(signatures[0])) != nil
}
