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

var PromiseAllInEffect = rule.Rule{
	Name:            "promiseAllInEffect",
	Group:           "effectNative",
	Description:     "Warns when Promise.all or Promise.allSettled fans out over an array inside Effect code instead of Effect.forEach or Effect.all with a concurrency option",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.X_0_starts_every_promise_at_once_with_no_concurrency_bound_and_they_keep_running_when_the_fiber_is_interrupted_Use_1_with_a_concurrency_option_instead_effect_promiseAllInEffect.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzePromiseAllInEffect(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		if len(matches) == 0 {
			return nil
		}
		settledReplacement := `Effect.all(..., { mode: "result" })`
		if ctx.TypeParser.SupportedEffectVersion() == typeparser.EffectMajorV3 {
			settledReplacement = `Effect.all(..., { mode: "either" })`
		}
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			replacement := "Effect.forEach"
			if match.Method == PromiseMethodAllSettled {
				replacement = settledReplacement
			}
			diags[i] = ctx.NewDiagnostic(
				ctx.SourceFile,
				match.Location,
				tsdiag.X_0_starts_every_promise_at_once_with_no_concurrency_bound_and_they_keep_running_when_the_fiber_is_interrupted_Use_1_with_a_concurrency_option_instead_effect_promiseAllInEffect,
				nil,
				"Promise."+string(match.Method),
				replacement,
			)
		}
		return diags
	},
}

type PromiseMethod string

const (
	PromiseMethodAll        PromiseMethod = "all"
	PromiseMethodAllSettled PromiseMethod = "allSettled"
)

type PromiseAllInEffectMatch struct {
	Location core.TextRange
	Method   PromiseMethod
	Flow     *typeparser.PipingFlow
	Index    int
}

func AnalyzePromiseAllInEffect(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []PromiseAllInEffectMatch {
	if tp == nil || c == nil || sf == nil {
		return nil
	}
	globalPromise := c.ResolveName("Promise", nil, ast.SymbolFlagsValue, false)
	if globalPromise == nil {
		return nil
	}
	memberNode := func(callee *ast.Node) *ast.Node {
		if callee != nil && (ast.IsPropertyAccessExpression(callee) || ast.IsElementAccessExpression(callee)) {
			return ast.GetElementOrPropertyAccessName(callee)
		}
		return nil
	}
	isGlobalPromiseReceiver := func(callee *ast.Node) bool {
		receiver := ast.SkipParentheses(callee.Expression())
		return tp.ResolveToGlobalSymbol(tp.GetSymbolAtLocation(receiver)) == globalPromise
	}
	combinatorName := func(transformation typeparser.PipingFlowTransformation) PromiseMethod {
		symbol := tp.ReferenceSymbolAtNode(memberNode(transformation.Callee()))
		if symbol == nil {
			return ""
		}
		if method := PromiseMethod(ast.TSGetSymbolName(symbol)); method == PromiseMethodAll || method == PromiseMethodAllSettled {
			return method
		}
		return ""
	}
	isCandidate := func(transformation typeparser.PipingFlowTransformation) bool {
		return combinatorName(transformation) != ""
	}

	var matches []PromiseAllInEffectMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for _, sequence := range flow.FindTransformationSequences(isCandidate).All() {
			index := sequence.Start()
			transformation := flow.Transformations().At(index)
			callee := transformation.Callee()
			method := combinatorName(transformation)
			if !isGlobalPromiseReceiver(callee) ||
				!tp.IsNodeReferenceToGlobalMember(memberNode(callee), "Promise", string(method)) ||
				tp.GetEffectContextFlags(callee)&typeparser.EffectContextFlagInEffect == 0 ||
				!isVariableLengthArrayType(tp, c, flow.TransformationInputType(index)) {
				continue
			}
			matches = append(matches, PromiseAllInEffectMatch{
				Location: scanner.GetErrorRangeForNode(sf, callee),
				Method:   method,
				Flow:     flow,
				Index:    index,
			})
		}
	}
	return matches
}

func isVariableLengthArrayType(tp *typeparser.TypeParser, c *checker.Checker, t *checker.Type) bool {
	if t == nil {
		return false
	}
	if t.Flags()&checker.TypeFlagsUnion != 0 {
		for _, member := range tp.UnrollUnionMembers(t) {
			if !isVariableLengthArrayType(tp, c, member) {
				return false
			}
		}
		return true
	}
	if t.Flags()&checker.TypeFlagsIntersection != 0 {
		return isVariableLengthIntersection(tp, c, t)
	}
	if t.Flags()&checker.TypeFlagsInstantiable != 0 {
		constraint := c.GetBaseConstraintOfType(t)
		return constraint != nil && constraint != t && isVariableLengthArrayType(tp, c, constraint)
	}
	if tp.ArrayType(t) != nil {
		return true
	}
	return checker.IsTupleType(t) && tupleHasVariableElement(t)
}

func isVariableLengthIntersection(tp *typeparser.TypeParser, c *checker.Checker, t *checker.Type) bool {
	constituents := t.Types()
	return !slices.ContainsFunc(constituents, isFixedLengthTuple) &&
		slices.ContainsFunc(constituents, func(constituent *checker.Type) bool {
			return isVariableLengthArrayType(tp, c, constituent)
		})
}

func isFixedLengthTuple(t *checker.Type) bool {
	return checker.IsTupleType(t) && !tupleHasVariableElement(t)
}

func tupleHasVariableElement(t *checker.Type) bool {
	for _, flags := range t.TargetTupleType().ElementFlags() {
		if flags&checker.ElementFlagsVariable != 0 {
			return true
		}
	}
	return false
}
