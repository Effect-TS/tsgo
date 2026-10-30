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

// SuspendFailToFailSync suggests Effect.failSync (or Effect.dieSync on Effect
// v3) when an Effect.suspend thunk only returns a flow ending in Effect.fail
// (or Effect.die).
var SuspendFailToFailSync = rule.Rule{
	Name:            "suspendFailToFailSync",
	Group:           "style",
	Description:     "Suggests Effect.failSync/dieSync instead of Effect.suspend wrapping a single Effect.fail/die",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_0_builds_this_1_lazily_directly_instead_of_wrapping_Effect_2_in_Effect_suspend_effect_suspendFailToFailSync.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeSuspendFailToFailSync(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_0_builds_this_1_lazily_directly_instead_of_wrapping_Effect_2_in_Effect_suspend_effect_suspendFailToFailSync,
				nil,
				match.ReplacementName,
				match.Kind,
				match.InnerName,
			)
		}
		return diags
	},
}

// SuspendFailToFailSyncMatch holds the nodes needed by the diagnostic and quick fix.
type SuspendFailToFailSyncMatch struct {
	SourceFile      *ast.SourceFile
	Location        core.TextRange
	ReplacementName string // failSync or dieSync
	Kind            string // error or defect
	InnerName       string // fail or die
	// Suspend is the Effect.suspend step; the thunk is the subject of its flow.
	Suspend typeparser.PipingFlowTransformation
	Thunk   *typeparser.ParsedLazyExpression
	// Inner is the flow of the thunk's returned expression, which it covers
	// whole; its last step is Effect.fail or Effect.die.
	Inner *typeparser.PipingFlow
	// Value is the input of that last step, when it is a standalone node.
	Value              *ast.Node
	ValueTypeArguments *ast.NodeList
	// Fixable is false when the rewrite would change the inferred error type.
	Fixable bool
}

// AnalyzeSuspendFailToFailSync finds piping flows that apply Effect.suspend to
// a thunk whose returned flow ends in Effect.fail, or Effect.die on Effect v3,
// which has Effect.dieSync.
func AnalyzeSuspendFailToFailSync(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []SuspendFailToFailSyncMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}
	allowDie := tp.SupportedEffectVersion() != typeparser.EffectMajorV4

	noTypeArguments := func(transformation typeparser.PipingFlowTransformation) bool {
		return transformation.TypeArguments() == nil || len(transformation.TypeArguments().Nodes) == 0
	}
	isEffectApi := func(name string) func(typeparser.PipingFlowTransformation) bool {
		return func(transformation typeparser.PipingFlowTransformation) bool {
			return transformation.Callee() != nil && transformation.Args().Len() == 0 &&
				tp.IsNodeReferenceToEffectModuleApi(transformation.Callee(), name)
		}
	}
	// An arrow or function expression is always the subject of its flow, so
	// Effect.suspend is the first transformation.
	isThunkSubject := func(subject typeparser.PipingFlowSubject) bool {
		return subject.Node() != nil &&
			(subject.Node().Kind == ast.KindArrowFunction || subject.Node().Kind == ast.KindFunctionExpression)
	}
	// Explicit type arguments on suspend pin an error channel that failSync would narrow.
	isSuspend := func(transformation typeparser.PipingFlowTransformation) bool {
		return noTypeArguments(transformation) && isEffectApi("suspend")(transformation)
	}

	var matches []SuspendFailToFailSyncMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		if !flow.MatchesPrefix(isThunkSubject, isSuspend) {
			continue
		}
		thunk := typeparser.ParseLazyExpression(flow.Subject().Node(), typeparser.LazyExpressionThunk)
		// A return type annotation pins the error channel independently of the value.
		if thunk == nil || thunk.Expression() == nil || thunk.Node().Type() != nil {
			continue
		}
		inner := tp.LongestPipingFlowAt(thunk.Expression(), true)
		if inner == nil || inner.Node() != ast.SkipParentheses(thunk.Expression()) || inner.Transformations().Len() == 0 {
			continue
		}
		lastIndex := inner.Transformations().Len() - 1
		last := inner.Transformations().At(lastIndex)

		suspend := flow.Transformations().At(0)
		match := SuspendFailToFailSyncMatch{
			SourceFile:         sf,
			Location:           scanner.GetErrorRangeForNode(sf, suspend.Callee()),
			Suspend:            suspend,
			Thunk:              thunk,
			Inner:              inner,
			Value:              inner.TransformationInputNode(lastIndex),
			ValueTypeArguments: last.TypeArguments(),
		}
		switch {
		case isEffectApi("fail")(last):
			match.ReplacementName, match.Kind, match.InnerName = "failSync", "error", "fail"
			// Explicit type arguments pin the error type in both forms.
			match.Fixable = !noTypeArguments(last) ||
				failSyncKeepsErrorType(tp, c, last, inner.TransformationInputType(lastIndex))
		case allowDie && isEffectApi("die")(last):
			match.ReplacementName, match.Kind, match.InnerName = "dieSync", "defect", "die"
			match.Fixable = true
		default:
			continue
		}
		matches = append(matches, match)
	}
	return matches
}

// failSyncKeepsErrorType reports whether Effect.failSync(() => value) infers the
// same error type as Effect.fail(value). Effect.fail infers its error from a
// call argument, which widens literal unions, while a failSync thunk returns the
// value against LazyArg<E>, which keeps them: for `cond ? "A" : "B"` the first
// infers string and the second "A" | "B". A single literal widens in both forms.
// The input type of a flow step is a regular literal type, which only the base
// type of a literal widens.
func failSyncKeepsErrorType(tp *typeparser.TypeParser, c *checker.Checker, failStep typeparser.PipingFlowTransformation, valueType *checker.Type) bool {
	effect := tp.EffectType(failStep.OutType())
	if effect == nil || effect.E() == nil || valueType == nil {
		return false
	}
	sameType := func(t *checker.Type) bool {
		return checker.Checker_isTypeAssignableTo(c, t, effect.E()) &&
			checker.Checker_isTypeAssignableTo(c, effect.E(), t)
	}
	return sameType(valueType) ||
		valueType.Flags()&checker.TypeFlagsUnit != 0 && sameType(c.GetBaseTypeOfLiteralType(valueType))
}
