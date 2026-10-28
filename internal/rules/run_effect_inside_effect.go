// Package rules contains all Effect diagnostic rule implementations.
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

// runEffectApis lists the Effect module run APIs that should not be called inside Effect generators.
var runEffectApis = []string{"runSync", "runSyncExit", "runPromise", "runPromiseExit", "runFork", "runCallback"}

// platformRunMainApi is the platform runtime API (NodeRuntime, BunRuntime, BrowserRuntime)
// that starts a separate root runtime and should only be called at the application entrypoint.
const platformRunMainApi = "runMain"

// RunEffectInsideEffect detects calls to the runEffectApis and to the platform runMain inside
// Effect contexts and suggests alternatives.
var RunEffectInsideEffect = rule.Rule{
	Name:            "runEffectInsideEffect",
	Group:           "antipattern",
	Description:     "Suggests using Runtime or Effect.run*With methods instead of Effect.run* inside Effect contexts, and reports platform runMain calls there",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.X_0_is_called_inside_an_existing_Effect_context_Here_the_inner_Effect_can_be_used_directly_effect_runEffectInsideEffect.Code(),
		tsdiag.X_0_is_called_inside_an_Effect_with_a_separate_runtime_invocation_In_this_context_run_child_Effects_with_the_surrounding_runtime_which_can_be_accessed_through_Effect_runtime_and_Runtime_1_effect_runEffectInsideEffect.Code(),
		tsdiag.X_0_is_called_inside_an_Effect_with_a_separate_services_invocation_In_this_context_child_Effects_run_with_the_surrounding_services_which_can_be_accessed_through_Effect_context_and_Effect_1_With_effect_runEffectInsideEffect.Code(),
		tsdiag.X_0_is_called_inside_an_Effect_context_and_starts_a_separate_root_runtime_which_the_surrounding_Effect_does_not_own_Return_the_Effect_instead_and_run_it_with_runMain_once_at_the_application_entrypoint_effect_runEffectInsideEffect.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		supportedEffect := ctx.TypeParser.SupportedEffectVersion()

		matches := AnalyzeRunEffectInsideEffect(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, 0, len(matches))
		for _, m := range matches {
			calleeText := scanner.GetSourceTextOfNodeFromSourceFile(m.SourceFile, m.CalleeNode, false)
			if m.IsPlatformRunMain {
				diags = append(diags, ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.X_0_is_called_inside_an_Effect_context_and_starts_a_separate_root_runtime_which_the_surrounding_Effect_does_not_own_Return_the_Effect_instead_and_run_it_with_runMain_once_at_the_application_entrypoint_effect_runEffectInsideEffect, nil, calleeText))
			} else if m.IsNestedScope {
				if supportedEffect == typeparser.EffectMajorV4 {
					diags = append(diags, ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.X_0_is_called_inside_an_Effect_with_a_separate_services_invocation_In_this_context_child_Effects_run_with_the_surrounding_services_which_can_be_accessed_through_Effect_context_and_Effect_1_With_effect_runEffectInsideEffect, nil, calleeText, m.MethodName))
				} else {
					diags = append(diags, ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.X_0_is_called_inside_an_Effect_with_a_separate_runtime_invocation_In_this_context_run_child_Effects_with_the_surrounding_runtime_which_can_be_accessed_through_Effect_runtime_and_Runtime_1_effect_runEffectInsideEffect, nil, calleeText, m.MethodName))
				}
			} else {
				diags = append(diags, ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.X_0_is_called_inside_an_existing_Effect_context_Here_the_inner_Effect_can_be_used_directly_effect_runEffectInsideEffect, nil, calleeText))
			}
		}
		return diags
	},
}

// RunEffectInsideEffectMatch holds the diagnostic and AST nodes needed by both the
// diagnostic rule and the quick-fix for the runEffectInsideEffect pattern.
type RunEffectInsideEffectMatch struct {
	SourceFile        *ast.SourceFile         // The source file where this match was found
	Location          core.TextRange          // Pre-computed error range (on the callee expression)
	CallNode          *ast.Node               // The full call expression node (e.g., Effect.runPromise(check))
	CalleeNode        *ast.Node               // The callee expression (e.g., Effect.runPromise)
	MethodName        string                  // The matched run API name (e.g., "runPromise")
	IsNestedScope     bool                    // True when call is in a nested scope rather than direct generator scope
	IsPlatformRunMain bool                    // True when the callee is a platform runMain (e.g., NodeRuntime.runMain)
	GeneratorFunction *ast.FunctionExpression // The enclosing Effect generator function, nil for a platform runMain in an Effect constructor thunk
}

// AnalyzeRunEffectInsideEffect finds all Effect.run* calls inside Effect generators, and the
// platform runMain calls inside Effect generators or Effect constructor thunks, returning matches with structured data for both diagnostics and quick-fixes.
func AnalyzeRunEffectInsideEffect(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []RunEffectInsideEffectMatch {
	var matches []RunEffectInsideEffectMatch

	var walk ast.Visitor
	walk = func(n *ast.Node) bool {
		if n == nil {
			return false
		}

		if n.Kind == ast.KindCallExpression {
			if m, ok := analyzeRunEffectInsideEffectNode(tp, c, sf, n); ok {
				matches = append(matches, m)
			}
		}

		n.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())
	return matches
}

// analyzeRunEffectInsideEffectNode checks a single call expression for Effect.run* inside an Effect generator.
func analyzeRunEffectInsideEffectNode(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, node *ast.Node) (RunEffectInsideEffectMatch, bool) {
	if node.Kind != ast.KindCallExpression {
		return RunEffectInsideEffectMatch{}, false
	}
	call := node.AsCallExpression()

	// Must have at least one argument (matching the TS reference: node.arguments.length === 0 => continue)
	if call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
		return RunEffectInsideEffectMatch{}, false
	}

	callee := call.Expression

	// A platform runMain starts its own root runtime, so it is reported in every Effect context
	if tp.IsNodeReferenceToPlatformRuntimeApi(callee, platformRunMainApi) {
		return analyzePlatformRunMainNode(tp, sf, node, callee)
	}

	// Check if the callee is one of the Effect.run* APIs
	methodName := matchRunEffectApi(tp, c, callee)
	if methodName == "" {
		return RunEffectInsideEffectMatch{}, false
	}

	genFn := findEnclosingEffectGeneratorFunction(tp, node)
	if genFn == nil {
		return RunEffectInsideEffectMatch{}, false
	}

	// Check that the generator body has at least one statement
	if genFn.Body == nil || genFn.Body.Kind != ast.KindBlock {
		return RunEffectInsideEffectMatch{}, false
	}
	block := genFn.Body.AsBlock()
	if block.Statements == nil || len(block.Statements.Nodes) == 0 {
		return RunEffectInsideEffectMatch{}, false
	}

	isNestedScope := ast.GetContainingFunction(node) != genFn.AsNode()

	return RunEffectInsideEffectMatch{
		SourceFile:        sf,
		Location:          scanner.GetErrorRangeForNode(sf, callee),
		CallNode:          node,
		CalleeNode:        callee,
		MethodName:        methodName,
		IsNestedScope:     isNestedScope,
		GeneratorFunction: genFn,
	}, true
}

// analyzePlatformRunMainNode reports a platform runMain call found inside an Effect generator,
// including its nested functions, or directly inside an Effect constructor thunk such as Effect.sync.
func analyzePlatformRunMainNode(tp *typeparser.TypeParser, sf *ast.SourceFile, node *ast.Node, callee *ast.Node) (RunEffectInsideEffectMatch, bool) {
	genFn := findEnclosingEffectGeneratorFunction(tp, node)
	if genFn == nil && tp.GetEffectContextFlags(node)&typeparser.EffectContextFlagInEffectConstructorThunk == 0 {
		return RunEffectInsideEffectMatch{}, false
	}

	return RunEffectInsideEffectMatch{
		SourceFile:        sf,
		Location:          scanner.GetErrorRangeForNode(sf, callee),
		CallNode:          node,
		CalleeNode:        callee,
		MethodName:        platformRunMainApi,
		IsNestedScope:     genFn != nil && ast.GetContainingFunction(node) != genFn.AsNode(),
		IsPlatformRunMain: true,
		GeneratorFunction: genFn,
	}, true
}

// findEnclosingEffectGeneratorFunction returns the Effect generator node yields to, or else the
// generator of the closest ancestor that can yield an Effect.
func findEnclosingEffectGeneratorFunction(tp *typeparser.TypeParser, node *ast.Node) *ast.FunctionExpression {
	genFn := tp.GetEffectYieldGeneratorFunction(node)
	if genFn == nil {
		for current := node.Parent; current != nil; current = current.Parent {
			if tp.GetEffectContextFlags(current)&typeparser.EffectContextFlagCanYieldEffect != 0 {
				genFn = tp.GetEffectYieldGeneratorFunction(current)
				if genFn != nil {
					break
				}
			}
		}
	}
	return genFn
}

// matchRunEffectApi checks if the node references one of the Effect.run* APIs and returns the method name.
// Returns empty string if no match.
func matchRunEffectApi(tp *typeparser.TypeParser, _ *checker.Checker, node *ast.Node) string {
	for _, name := range runEffectApis {
		if tp.IsNodeReferenceToEffectModuleApi(node, name) {
			return name
		}
	}
	return ""
}
