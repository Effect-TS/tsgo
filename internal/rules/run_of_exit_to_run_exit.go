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
	RunnerNameNode     *ast.Node
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
	for _, flow := range tp.PipingFlows(sf, false) {
		for index := 0; index+1 < len(flow.Transformations); index++ {
			exitTransformation := &flow.Transformations[index]
			if exitTransformation.Callee == nil || len(exitTransformation.Args) != 0 ||
				!tp.IsNodeReferenceToEffectModuleApi(exitTransformation.Callee, "exit") {
				continue
			}

			runnerTransformation := &flow.Transformations[index+1]
			runnerNameNode, runnerName, replacementName := runOfExitRunner(tp, runnerTransformation.Callee)
			if runnerName == "" {
				continue
			}

			matches = appendRunOfExitMatch(matches, RunOfExitToRunExitMatch{
				SourceFile:         sf,
				Location:           runOfExitLocation(sf, runnerTransformation.Callee, runnerNameNode),
				ExitTransformation: exitTransformation,
				RunnerNameNode:     runnerNameNode,
				RunnerName:         runnerName,
				ReplacementName:    replacementName,
			})
		}
	}

	// Non-dual runners with RunOptions are not piping-flow transformations,
	// because their effect parameter is one of multiple arguments. Inspect the
	// runner call itself, then still use a piping flow for its effect argument so
	// direct Effect.exit calls and pipe tails share the same matching logic.
	var walk func(*ast.Node)
	walk = func(node *ast.Node) {
		if node == nil {
			return
		}
		if node.Kind == ast.KindCallExpression {
			call := node.AsCallExpression()
			if call != nil && call.Expression != nil && call.Arguments != nil && len(call.Arguments.Nodes) > 0 {
				runnerNameNode, runnerName, replacementName := runOfExitRunner(tp, call.Expression)
				if runnerName != "" {
					argumentFlow := tp.LongestPipingFlowAt(call.Arguments.Nodes[0], false)
					if argumentFlow != nil && len(argumentFlow.Transformations) > 0 {
						exitTransformation := &argumentFlow.Transformations[len(argumentFlow.Transformations)-1]
						if exitTransformation.Callee != nil && len(exitTransformation.Args) == 0 &&
							tp.IsNodeReferenceToEffectModuleApi(exitTransformation.Callee, "exit") {
							matches = appendRunOfExitMatch(matches, RunOfExitToRunExitMatch{
								SourceFile:         sf,
								Location:           runOfExitLocation(sf, call.Expression, runnerNameNode),
								ExitTransformation: exitTransformation,
								RunnerNameNode:     runnerNameNode,
								RunnerName:         runnerName,
								ReplacementName:    replacementName,
							})
						}
					}
				}
			}
		}
		node.ForEachChild(func(child *ast.Node) bool {
			walk(child)
			return false
		})
	}
	walk(sf.AsNode())

	sort.Slice(matches, func(i, j int) bool {
		return matches[i].Location.Pos() < matches[j].Location.Pos()
	})
	return matches
}

func appendRunOfExitMatch(matches []RunOfExitToRunExitMatch, candidate RunOfExitToRunExitMatch) []RunOfExitToRunExitMatch {
	for _, match := range matches {
		if match.Location.Pos() == candidate.Location.Pos() && match.Location.End() == candidate.Location.End() {
			return matches
		}
	}
	return append(matches, candidate)
}

func runOfExitLocation(sf *ast.SourceFile, callee, nameNode *ast.Node) core.TextRange {
	locationNode := callee
	if nameNode != nil && nameNode.Parent != nil {
		locationNode = nameNode.Parent
	}
	return scanner.GetErrorRangeForNode(sf, locationNode)
}

func runOfExitRunner(tp *typeparser.TypeParser, callee *ast.Node) (nameNode *ast.Node, runnerName string, replacementName string) {
	if callee == nil {
		return nil, "", ""
	}

	target := callee
	if callee.Kind == ast.KindCallExpression {
		call := callee.AsCallExpression()
		if call == nil || call.Expression == nil {
			return nil, "", ""
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
		return nameNode, candidate.runner, candidate.replacement
	}

	return nil, "", ""
}
