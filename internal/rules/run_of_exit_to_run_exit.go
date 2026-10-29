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

// RunOfExitToRunExit suggests using the dedicated promise Exit runner instead
// of running an Effect that has first been transformed with Effect.exit.
var RunOfExitToRunExit = rule.Rule{
	Name:            "runOfExitToRunExit",
	Group:           "style",
	Description:     "Suggests using Effect.runPromiseExit instead of passing Effect.exit to Effect.runPromise",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.Effect_0_of_Effect_exit_re_implements_Effect_1_Use_the_dedicated_Exit_runner_directly_effect_runOfExitToRunExit.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeRunOfExitToRunExit(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diagnostics := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diagnostics[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.Effect_0_of_Effect_exit_re_implements_Effect_1_Use_the_dedicated_Exit_runner_directly_effect_runOfExitToRunExit,
				nil,
				match.RunnerName,
				match.ReplacementName,
			)
		}
		return diagnostics
	},
}

// RunOfExitToRunExitMatch holds the transformations needed by the diagnostic
// and quick fix.
type RunOfExitToRunExitMatch struct {
	SourceFile         *ast.SourceFile
	Location           core.TextRange
	ExitTransformation *typeparser.PipingFlowTransformation
	RunnerCallee       *ast.Node
	RunnerName         string
	ReplacementName    string
}

// AnalyzeRunOfExitToRunExit finds Effect.exit immediately followed by a
// non-Exit runner in a normalized piping flow. Adjacency is important: an
// Effect.exit followed by another transformation is not the runner's input.
func AnalyzeRunOfExitToRunExit(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []RunOfExitToRunExitMatch {
	if tp == nil || sf == nil {
		return nil
	}

	var matches []RunOfExitToRunExitMatch
	for _, flow := range tp.PipingFlows(sf, false).All() {
		sequences := flow.FindTransformationSequences(
			func(transformation typeparser.PipingFlowTransformation) bool {
				return transformation.Callee() != nil && transformation.Args().Len() == 0 &&
					tp.IsNodeReferenceToEffectModuleApi(transformation.Callee(), "exit")
			},
			func(transformation typeparser.PipingFlowTransformation) bool {
				_, _, runnerName, _ := runOfExitRunner(tp, transformation.Callee())
				return runnerName != ""
			},
		)
		for _, sequence := range sequences.All() {
			exitTransformation := flow.Transformations().At(sequence.Start())
			runnerTransformation := flow.Transformations().At(sequence.Start() + 1)
			runnerCallee, runnerNameNode, runnerName, replacementName := runOfExitRunner(tp, runnerTransformation.Callee())

			matches = append(matches, RunOfExitToRunExitMatch{
				SourceFile:         sf,
				Location:           runOfExitLocation(sf, runnerTransformation.Callee(), runnerNameNode),
				ExitTransformation: &exitTransformation,
				RunnerCallee:       runnerCallee,
				RunnerName:         runnerName,
				ReplacementName:    replacementName,
			})
		}
	}

	sort.Slice(matches, func(i, j int) bool {
		return matches[i].Location.Pos() < matches[j].Location.Pos()
	})
	return matches
}

func runOfExitLocation(sf *ast.SourceFile, callee, nameNode *ast.Node) core.TextRange {
	locationNode := callee
	if nameNode != nil && nameNode.Parent != nil {
		locationNode = nameNode.Parent
	}
	return scanner.GetErrorRangeForNode(sf, locationNode)
}

func runOfExitRunner(tp *typeparser.TypeParser, callee *ast.Node) (targetNode *ast.Node, nameNode *ast.Node, runnerName string, replacementName string) {
	if callee == nil {
		return nil, nil, "", ""
	}

	target := callee
	if callee.Kind == ast.KindCallExpression {
		call := callee.AsCallExpression()
		if call == nil || call.Expression == nil {
			return nil, nil, "", ""
		}
		target = call.Expression
	}

	for _, candidate := range []struct {
		runner      string
		replacement string
	}{
		{runner: "runPromise", replacement: "runPromiseExit"},
		{runner: "runPromiseWith", replacement: "runPromiseExitWith"},
	} {
		if !tp.IsNodeReferenceToEffectModuleApi(target, candidate.runner) {
			continue
		}
		if target.Kind == ast.KindPropertyAccessExpression {
			nameNode = target.AsPropertyAccessExpression().Name()
		}
		return target, nameNode, candidate.runner, candidate.replacement
	}

	return nil, nil, "", ""
}
