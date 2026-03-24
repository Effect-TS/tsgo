// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// runEffectApis lists the Effect module run APIs that should not be called inside Effect generators.
var runEffectApis = []string{"runSync", "runPromise", "runFork", "runCallback"}

// RunEffectInsideEffect detects Effect.runSync, Effect.runPromise, Effect.runFork, and
// Effect.runCallback call expressions inside Effect generator contexts and suggests alternatives.
// This rule is V3-only — it is not applicable to V4.
var RunEffectInsideEffect = rule.Rule{
	Name:            "runEffectInsideEffect",
	Group:           "antipattern",
	Description:     "Suggests using Runtime methods instead of Effect.run* inside Effect contexts",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3"},
	Codes: []int32{
		tsdiag.Using_0_inside_an_Effect_is_not_recommended_Effects_inside_generators_can_usually_just_be_yielded_effect_runEffectInsideEffect.Code(),
		tsdiag.Using_0_inside_an_Effect_is_not_recommended_The_same_runtime_should_generally_be_used_instead_to_run_child_effects_Consider_extracting_the_Runtime_by_using_for_example_Effect_runtime_and_then_use_Runtime_1_with_the_extracted_runtime_instead_effect_runEffectInsideEffect.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		// V3-only rule: skip for V4
		if typeparser.SupportedEffectVersion(ctx.Checker) == typeparser.EffectMajorV4 {
			return nil
		}

		matches := AnalyzeRunEffectInsideEffect(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, 0, len(matches))
		for _, m := range matches {
			calleeText := scanner.GetSourceTextOfNodeFromSourceFile(m.SourceFile, m.CalleeNode, false)
			if m.IsNestedScope {
				diags = append(diags, ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.Using_0_inside_an_Effect_is_not_recommended_The_same_runtime_should_generally_be_used_instead_to_run_child_effects_Consider_extracting_the_Runtime_by_using_for_example_Effect_runtime_and_then_use_Runtime_1_with_the_extracted_runtime_instead_effect_runEffectInsideEffect, nil, calleeText, m.MethodName))
			} else {
				diags = append(diags, ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.Using_0_inside_an_Effect_is_not_recommended_Effects_inside_generators_can_usually_just_be_yielded_effect_runEffectInsideEffect, nil, calleeText))
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
	GeneratorFunction *ast.FunctionExpression // The enclosing Effect generator function
}

// AnalyzeRunEffectInsideEffect finds all Effect.run* calls inside Effect generators,
// returning matches with structured data for both diagnostics and quick-fixes.
func AnalyzeRunEffectInsideEffect(c *checker.Checker, sf *ast.SourceFile) []RunEffectInsideEffectMatch {
	var matches []RunEffectInsideEffectMatch

	var walk ast.Visitor
	walk = func(n *ast.Node) bool {
		if n == nil {
			return false
		}

		if n.Kind == ast.KindCallExpression {
			if m, ok := analyzeRunEffectInsideEffectNode(c, sf, n); ok {
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
func analyzeRunEffectInsideEffectNode(c *checker.Checker, sf *ast.SourceFile, node *ast.Node) (RunEffectInsideEffectMatch, bool) {
	if node.Kind != ast.KindCallExpression {
		return RunEffectInsideEffectMatch{}, false
	}
	call := node.AsCallExpression()

	// Must have at least one argument (matching the TS reference: node.arguments.length === 0 => continue)
	if call.Arguments == nil || len(call.Arguments.Nodes) == 0 {
		return RunEffectInsideEffectMatch{}, false
	}

	callee := call.Expression

	// Check if the callee is one of the Effect.run* APIs
	methodName := matchRunEffectApi(c, callee)
	if methodName == "" {
		return RunEffectInsideEffectMatch{}, false
	}

	genFn := typeparser.GetEffectYieldGeneratorFunction(c, node)
	if genFn == nil {
		for current := node.Parent; current != nil; current = current.Parent {
			if typeparser.GetEffectContextFlags(c, current)&typeparser.EffectContextFlagCanYieldEffect != 0 {
				genFn = typeparser.GetEffectYieldGeneratorFunction(c, current)
				if genFn != nil {
					break
				}
			}
		}
	}
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

// matchRunEffectApi checks if the node references one of the Effect.run* APIs and returns the method name.
// Returns empty string if no match.
func matchRunEffectApi(c *checker.Checker, node *ast.Node) string {
	for _, name := range runEffectApis {
		if typeparser.IsNodeReferenceToEffectModuleApi(c, node, name) {
			return name
		}
	}
	return ""
}
