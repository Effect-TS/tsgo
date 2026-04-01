package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// MultipleEffectProvide detects consecutive Effect.provide calls with Layer-typed
// arguments within piping flows and warns that they should be merged into a single
// provide call to avoid service lifecycle issues.
var MultipleEffectProvide = rule.Rule{
	Name:            "multipleEffectProvide",
	Group:           "antipattern",
	Description:     "Warns against chaining Effect.provide calls which can cause service lifecycle issues",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_expression_chains_multiple_Effect_provide_calls_Providing_Layers_in_multiple_calls_in_a_chain_can_break_service_lifecycle_behavior_compared_with_a_single_combined_provide_with_merged_layers_effect_multipleEffectProvide.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeMultipleEffectProvide(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.This_expression_chains_multiple_Effect_provide_calls_Providing_Layers_in_multiple_calls_in_a_chain_can_break_service_lifecycle_behavior_compared_with_a_single_combined_provide_with_merged_layers_effect_multipleEffectProvide, nil)
		}
		return diags
	},
}

// MultipleEffectProvideMatch holds the AST nodes needed by both the
// diagnostic rule and the quick-fix for the multipleEffectProvide pattern.
type MultipleEffectProvideMatch struct {
	SourceFile       *ast.SourceFile // The source file where the diagnostic should be reported
	Location         core.TextRange  // The diagnostic span (error range on the first call in the chunk)
	Chunk            []*ast.Node     // The list of call expression nodes in the consecutive provide chain (2+ elements)
	LayerArgs        []*ast.Node     // The layer argument nodes from each Effect.provide(layer) call
	EffectModuleNode *ast.Node       // The Effect module identifier node from the first chunk callee
}

// AnalyzeMultipleEffectProvide finds all consecutive Effect.provide call chains
// with Layer-typed arguments within piping flows.
func AnalyzeMultipleEffectProvide(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []MultipleEffectProvideMatch {
	var matches []MultipleEffectProvideMatch

	flows := tp.PipingFlows(sf, true)
	for _, flow := range flows {
		// Track chunks of consecutive Effect.provide calls with Layer arguments.
		var currentChunk []*ast.Node
		var currentLayerArgs []*ast.Node
		var currentEffectModuleNode *ast.Node

		finalizeChunk := func() {
			if len(currentChunk) >= 2 {
				matches = append(matches, MultipleEffectProvideMatch{
					SourceFile:       sf,
					Location:         scanner.GetErrorRangeForNode(sf, currentChunk[0]),
					Chunk:            currentChunk,
					LayerArgs:        currentLayerArgs,
					EffectModuleNode: currentEffectModuleNode,
				})
			}
			currentChunk = nil
			currentLayerArgs = nil
			currentEffectModuleNode = nil
		}

		for _, transformation := range flow.Transformations {
			// Check if this is an Effect.provide call
			if !tp.IsNodeReferenceToEffectModuleApi(transformation.Callee, "provide") {
				finalizeChunk()
				continue
			}

			// Must have arguments
			if len(transformation.Args) == 0 {
				finalizeChunk()
				continue
			}

			// Check if the first argument is a Layer type
			arg := transformation.Args[0]
			argType := tp.GetTypeAtLocation(arg)
			if argType == nil {
				finalizeChunk()
				continue
			}

			if tp.LayerType(argType, arg) == nil {
				// provide call but not with a Layer argument — breaks the chain
				finalizeChunk()
				continue
			}

			// Find the enclosing call expression for this provide
			callNode := ast.FindAncestorKind(transformation.Callee, ast.KindCallExpression)
			if callNode == nil {
				finalizeChunk()
				continue
			}

			// Capture Effect module node from the callee's property access expression
			if len(currentChunk) == 0 && transformation.Callee.Kind == ast.KindPropertyAccessExpression {
				currentEffectModuleNode = transformation.Callee.AsPropertyAccessExpression().Expression
			}

			currentChunk = append(currentChunk, callNode)
			currentLayerArgs = append(currentLayerArgs, arg)
		}

		// Finalize the last chunk after the loop
		finalizeChunk()
	}

	return matches
}
