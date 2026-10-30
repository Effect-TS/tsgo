package rules

import (
	"sort"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// RunProvidedContextToRunWith suggests the dedicated Effect.run*With runners
// instead of running an effect whose outermost step provides a Context.
var RunProvidedContextToRunWith = rule.Rule{
	Name:            "runProvidedContextToRunWith",
	Group:           "style",
	Description:     "Suggests Effect.run*With instead of running an effect provided with Effect.provideContext or Effect.setContext",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.Effect_0_of_Effect_1_re_implements_Effect_2_Use_Effect_2_context_effect_to_run_the_effect_with_the_context_directly_effect_runProvidedContextToRunWith.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeRunProvidedContextToRunWith(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_0_of_Effect_1_re_implements_Effect_2_Use_Effect_2_context_effect_to_run_the_effect_with_the_context_directly_effect_runProvidedContextToRunWith,
				nil,
				match.RunnerName,
				match.ProvideName,
				match.ReplacementName,
			)
		}
		return diagnostics
	},
}

// RunProvidedContextToRunWithMatch holds the transformations needed by the
// diagnostic and quick fix.
type RunProvidedContextToRunWithMatch struct {
	SourceFile   *ast.SourceFile
	Location     core.TextRange
	Flow         *typeparser.PipingFlow
	ProvideIndex int
	Provide      *typeparser.PipingFlowTransformation
	Runner       *typeparser.PipingFlowTransformation
	RunnerCallee *ast.Node
	// Context is the Context.Context argument of the provide step.
	Context *ast.Node
	// EffectModuleNode is the receiver of RunnerCallee when it is the Effect
	// module, so the fix keeps namespace aliases; nil otherwise.
	EffectModuleNode *ast.Node
	RunnerName       string
	ProvideName      string
	ReplacementName  string
}

var runProvidedContextRunners = []struct {
	runner      string
	replacement string
}{
	{runner: "runPromise", replacement: "runPromiseWith"},
	{runner: "runPromiseExit", replacement: "runPromiseExitWith"},
	{runner: "runSync", replacement: "runSyncWith"},
	{runner: "runSyncExit", replacement: "runSyncExitWith"},
	{runner: "runFork", replacement: "runForkWith"},
	{runner: "runCallback", replacement: "runCallbackWith"},
}

// AnalyzeRunProvidedContextToRunWith finds Effect.provideContext or
// Effect.setContext immediately followed by a non-*With runner in a normalized
// piping flow, including Effect.fn steps. Adjacency is important: a provide
// step followed by another transformation is not the runner's input.
func AnalyzeRunProvidedContextToRunWith(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []RunProvidedContextToRunWithMatch {
	if tp == nil || sf == nil || tp.SupportedEffectVersion() != typeparser.EffectMajorV4 {
		return nil
	}

	var matches []RunProvidedContextToRunWithMatch
	for _, flow := range tp.PipingFlows(sf, true).All() {
		sequences := flow.FindTransformationSequences(
			func(transformation typeparser.PipingFlowTransformation) bool {
				return runProvidedContextProvideName(tp, transformation) != ""
			},
			func(transformation typeparser.PipingFlowTransformation) bool {
				runnerName, _ := runProvidedContextRunner(tp, transformation)
				return runnerName != ""
			},
		)
		for _, sequence := range sequences.All() {
			provide := flow.Transformations().At(sequence.Start())
			runner := flow.Transformations().At(sequence.Start() + 1)
			context := provide.Args().At(0)
			if tp.ContextTypeAtLocation(context) == nil {
				continue
			}
			runnerName, replacementName := runProvidedContextRunner(tp, runner)
			runnerCallee := runner.Callee()

			var effectModuleNode *ast.Node
			if runnerCallee.Kind == ast.KindPropertyAccessExpression {
				if receiver := runnerCallee.AsPropertyAccessExpression().Expression; tp.IsExpressionEffectModule(receiver) {
					effectModuleNode = receiver
				}
			}

			matches = append(matches, RunProvidedContextToRunWithMatch{
				SourceFile:       sf,
				Location:         scanner.GetErrorRangeForNode(sf, runnerCallee),
				Flow:             flow,
				ProvideIndex:     sequence.Start(),
				Provide:          &provide,
				Runner:           &runner,
				RunnerCallee:     runnerCallee,
				Context:          context,
				EffectModuleNode: effectModuleNode,
				RunnerName:       runnerName,
				ProvideName:      runProvidedContextProvideName(tp, provide),
				ReplacementName:  replacementName,
			})
		}
	}

	sort.Slice(matches, func(i, j int) bool {
		return matches[i].Location.Pos() < matches[j].Location.Pos()
	})
	return matches
}

// runProvidedContextProvideName returns "provideContext" or "setContext" when
// transformation provides a single argument through one of those APIs.
func runProvidedContextProvideName(tp *typeparser.TypeParser, transformation typeparser.PipingFlowTransformation) string {
	callee := transformation.Callee()
	if callee == nil || transformation.Args().Len() != 1 || ast.IsSpreadElement(transformation.Args().At(0)) {
		return ""
	}
	switch transformation.Kind() {
	case typeparser.TransformationKindDataFirst, typeparser.TransformationKindCall,
		typeparser.TransformationKindPipe, typeparser.TransformationKindPipeable,
		typeparser.TransformationKindEffectFn, typeparser.TransformationKindEffectFnUntraced:
	default:
		return ""
	}
	for _, name := range []string{"provideContext", "setContext"} {
		if tp.IsNodeReferenceToEffectModuleApi(callee, name) {
			return name
		}
	}
	return ""
}

// runProvidedContextRunner returns the runner name and its *With replacement
// when transformation runs its input with one of the context-free runners.
// Already-applied factories such as Effect.runPromiseWith(context) never match.
func runProvidedContextRunner(tp *typeparser.TypeParser, transformation typeparser.PipingFlowTransformation) (string, string) {
	callee := transformation.Callee()
	if callee == nil || callee.Kind == ast.KindCallExpression {
		return "", ""
	}
	switch transformation.Kind() {
	case typeparser.TransformationKindCall, typeparser.TransformationKindPipe, typeparser.TransformationKindPipeable,
		typeparser.TransformationKindEffectFn, typeparser.TransformationKindEffectFnUntraced:
		if transformation.Args().Len() != 0 {
			return "", ""
		}
	case typeparser.TransformationKindCallWithOptionals:
	default:
		return "", ""
	}
	for _, candidate := range runProvidedContextRunners {
		if tp.IsNodeReferenceToEffectModuleApi(callee, candidate.runner) {
			return candidate.runner, candidate.replacement
		}
	}
	return "", ""
}
