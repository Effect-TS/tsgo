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

// FailCauseToConstructor suggests Effect.fail, Effect.die, Effect.failSync and
// Effect.dieSync when Effect.failCause or Effect.failCauseSync wraps a single
// Cause.fail or Cause.die.
var FailCauseToConstructor = rule.Rule{
	Name:            "failCauseToConstructor",
	Group:           "style",
	Description:     "Suggests Effect.fail/die/failSync/dieSync instead of failCause/failCauseSync wrapping a single Cause.fail/die",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_0_expresses_this_failure_directly_instead_of_wrapping_Cause_2_in_Effect_1_effect_failCauseToConstructor.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeFailCauseToConstructor(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_0_expresses_this_failure_directly_instead_of_wrapping_Cause_2_in_Effect_1_effect_failCauseToConstructor,
				nil,
				match.ReplacementName,
				match.SourceName,
				match.ConstructorName,
			)
		}
		return diags
	},
}

// FailCauseToConstructorMatch holds the nodes needed by the diagnostic and quick fix.
type FailCauseToConstructorMatch struct {
	SourceFile         *ast.SourceFile
	Location           core.TextRange
	ReplacementName    string    // fail, die, failSync or dieSync
	SourceName         string    // failCause or failCauseSync
	ConstructorName    string    // fail or die
	EffectModuleNode   *ast.Node // The Effect module identifier (e.g., "Effect" in Effect.failCause)
	ValueNode          *ast.Node // The value passed to Cause.fail / Cause.die
	ValueTypeArguments *ast.NodeList
	// Eager fix: the flow prefix up to and including Effect.failCause is
	// replaced. A nil Flow means the match is reported without a fix.
	Flow                *typeparser.PipingFlow
	TransformationCount int
	// Lazy fix: the Cause constructor call in the thunk is replaced by its
	// value and the failCauseSync callee is renamed. A nil LazyCauseNode means
	// the match is reported without a fix.
	LazyCallee         *ast.Node
	LazyCauseNode      *ast.Node
	LazyExpressionBody bool
}

// AnalyzeFailCauseToConstructor finds piping flows in which Cause.fail or
// Cause.die feeds directly into Effect.failCause, and Effect.failCauseSync
// thunks whose returned flow ends in Cause.fail (or Cause.die on Effect v3,
// which has Effect.dieSync).
func AnalyzeFailCauseToConstructor(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []FailCauseToConstructorMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}

	noTypeArguments := func(transformation typeparser.PipingFlowTransformation) bool {
		return transformation.TypeArguments() == nil || len(transformation.TypeArguments().Nodes) == 0
	}
	isCauseConstructor := func(name string) func(typeparser.PipingFlowTransformation) bool {
		return func(transformation typeparser.PipingFlowTransformation) bool {
			return transformation.Callee() != nil && transformation.Args().Len() == 0 &&
				tp.IsNodeReferenceToEffectCauseModuleApi(transformation.Callee(), name)
		}
	}
	isEffectApi := func(name string) func(typeparser.PipingFlowTransformation) bool {
		return func(transformation typeparser.PipingFlowTransformation) bool {
			return transformation.Callee() != nil && transformation.Args().Len() == 0 && noTypeArguments(transformation) &&
				tp.IsNodeReferenceToEffectModuleApi(transformation.Callee(), name)
		}
	}
	allowLazyDie := tp.SupportedEffectVersion() != typeparser.EffectMajorV4
	// A callee without an Effect module receiver (a named import or an alias) is
	// rewritten through the file's Effect module identifier, which must resolve
	// to the Effect module where the edit lands.
	canRewriteCallee := func(callee *ast.Node) bool {
		return effectModuleReceiver(callee) != nil || tp.EffectModuleIdentifierAt(callee) != ""
	}

	var matches []FailCauseToConstructorMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for _, constructorName := range []string{"fail", "die"} {
			sequences := flow.FindTransformationSequences(isCauseConstructor(constructorName), isEffectApi("failCause"))
			for _, sequence := range sequences.All() {
				causeIndex := sequence.Start()
				causeStep := flow.Transformations().At(causeIndex)
				failStep := flow.Transformations().At(causeIndex + 1)
				value := flow.TransformationInputNode(causeIndex)
				match := FailCauseToConstructorMatch{
					SourceFile:         sf,
					Location:           scanner.GetErrorRangeForNode(sf, failStep.Callee()),
					ReplacementName:    constructorName,
					SourceName:         "failCause",
					ConstructorName:    constructorName,
					EffectModuleNode:   effectModuleReceiver(failStep.Callee()),
					ValueNode:          value,
					ValueTypeArguments: causeStep.TypeArguments(),
				}
				if value != nil && canRewriteCallee(failStep.Callee()) && (failStep.Kind() == typeparser.TransformationKindCall ||
					failStep.Kind() == typeparser.TransformationKindPipe ||
					failStep.Kind() == typeparser.TransformationKindPipeable) {
					match.Flow = flow
					match.TransformationCount = causeIndex + 2
				}
				matches = append(matches, match)
			}
		}

		// An arrow or function expression is always the subject of its flow,
		// so Effect.failCauseSync is the first transformation.
		isThunkSubject := func(subject typeparser.PipingFlowSubject) bool {
			return subject.Node() != nil &&
				(subject.Node().Kind == ast.KindArrowFunction || subject.Node().Kind == ast.KindFunctionExpression)
		}
		if !flow.MatchesPrefix(isThunkSubject, isEffectApi("failCauseSync")) {
			continue
		}
		lazy := typeparser.ParseLazyExpression(flow.Subject().Node(), typeparser.LazyExpressionThunk)
		// A return type annotation pins the cause type independently of the value.
		if lazy == nil || lazy.Expression() == nil || lazy.Node().Type() != nil {
			continue
		}
		inner := tp.LongestPipingFlowAt(lazy.Expression(), true)
		if inner == nil || inner.Transformations().Len() == 0 {
			continue
		}
		lastIndex := inner.Transformations().Len() - 1
		last := inner.Transformations().At(lastIndex)
		var constructorName string
		switch {
		case isCauseConstructor("fail")(last):
			constructorName = "fail"
		case allowLazyDie && isCauseConstructor("die")(last):
			constructorName = "die"
		default:
			continue
		}

		failStep := flow.Transformations().At(0)
		match := FailCauseToConstructorMatch{
			SourceFile:       sf,
			Location:         scanner.GetErrorRangeForNode(sf, failStep.Callee()),
			ReplacementName:  constructorName + "Sync",
			SourceName:       "failCauseSync",
			ConstructorName:  constructorName,
			EffectModuleNode: effectModuleReceiver(failStep.Callee()),
		}
		body := ast.SkipParentheses(lazy.Expression())
		value := inner.TransformationInputNode(lastIndex)
		if failStep.Kind() == typeparser.TransformationKindCall && noTypeArguments(last) && value != nil &&
			body.Kind == ast.KindCallExpression && body.AsCallExpression().Expression == last.Callee() &&
			canRewriteCallee(failStep.Callee()) &&
			(constructorName == "die" || lazyFailKeepsErrorType(tp, c, failStep, value)) {
			match.ValueNode = value
			match.LazyCallee = failStep.Callee()
			match.LazyCauseNode = body
			match.LazyExpressionBody = !ast.IsBlock(lazy.Body()) && lazy.Expression() == body
		}
		matches = append(matches, match)
	}
	return matches
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

func effectModuleReceiver(callee *ast.Node) *ast.Node {
	if callee != nil && callee.Kind == ast.KindPropertyAccessExpression {
		return callee.AsPropertyAccessExpression().Expression
	}
	return nil
}
