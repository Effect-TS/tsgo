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
	SourceFile      *ast.SourceFile
	Location        core.TextRange
	ReplacementName string // fail, die, failSync or dieSync
	SourceName      string // failCause or failCauseSync
	ConstructorName string // fail or die
	// Flow's first TransformationCount steps end in Effect.failCause, after
	// Cause.fail or Cause.die, or are the single Effect.failCauseSync step
	// applied to Lazy.
	Flow                *typeparser.PipingFlow
	TransformationCount int
	// Lazy is the failCauseSync thunk, and Inner the flow of its returned
	// expression, which ends in Cause.fail or Cause.die; both are nil for
	// Effect.failCause.
	Lazy  *typeparser.ParsedLazyExpression
	Inner *typeparser.PipingFlow
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

	var matches []FailCauseToConstructorMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		for _, constructorName := range []string{"fail", "die"} {
			sequences := flow.FindTransformationSequences(isCauseConstructor(constructorName), isEffectApi("failCause"))
			for _, sequence := range sequences.All() {
				failStep := flow.Transformations().At(sequence.Start() + 1)
				matches = append(matches, FailCauseToConstructorMatch{
					SourceFile:          sf,
					Location:            scanner.GetErrorRangeForNode(sf, failStep.Callee()),
					ReplacementName:     constructorName,
					SourceName:          "failCause",
					ConstructorName:     constructorName,
					Flow:                flow,
					TransformationCount: sequence.Start() + 2,
				})
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
		last := inner.Transformations().At(inner.Transformations().Len() - 1)
		var constructorName string
		switch {
		case isCauseConstructor("fail")(last):
			constructorName = "fail"
		case allowLazyDie && isCauseConstructor("die")(last):
			constructorName = "die"
		default:
			continue
		}

		matches = append(matches, FailCauseToConstructorMatch{
			SourceFile:          sf,
			Location:            scanner.GetErrorRangeForNode(sf, flow.Transformations().At(0).Callee()),
			ReplacementName:     constructorName + "Sync",
			SourceName:          "failCauseSync",
			ConstructorName:     constructorName,
			Flow:                flow,
			TransformationCount: 1,
			Lazy:                lazy,
			Inner:               inner,
		})
	}
	return matches
}
