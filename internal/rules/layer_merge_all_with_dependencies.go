package rules

import (
	"strings"

	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/checkerutils"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// LayerMergeAllWithDependencies detects interdependencies in Layer.mergeAll calls
// where one layer provides a service that another layer requires. Since mergeAll
// creates layers in parallel, these dependencies will not be satisfied.
var LayerMergeAllWithDependencies = rule.Rule{
	Name:            "layerMergeAllWithDependencies",
	Description:     "Detects interdependencies in Layer.mergeAll calls where one layer provides a service that another layer requires",
	DefaultSeverity: etscore.SeverityWarning,
	Codes:           []int32{tsdiag.This_layer_provides_0_which_is_required_by_another_layer_in_the_same_Layer_mergeAll_call_Layer_mergeAll_creates_layers_in_parallel_so_dependencies_between_layers_will_not_be_satisfied_Consider_moving_this_layer_into_a_Layer_provideMerge_after_the_Layer_mergeAll_effect_layerMergeAllWithDependencies.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeLayerMergeAllWithDependencies(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.This_layer_provides_0_which_is_required_by_another_layer_in_the_same_Layer_mergeAll_call_Layer_mergeAll_creates_layers_in_parallel_so_dependencies_between_layers_will_not_be_satisfied_Consider_moving_this_layer_into_a_Layer_provideMerge_after_the_Layer_mergeAll_effect_layerMergeAllWithDependencies, nil, m.ProvidedTypes)
		}
		return diags
	},
}

// LayerMergeAllWithDependenciesMatch holds the AST nodes needed by both the
// diagnostic rule and the quick-fix for the layerMergeAllWithDependencies pattern.
type LayerMergeAllWithDependenciesMatch struct {
	SourceFile      *ast.SourceFile
	Location        core.TextRange // The error range on the provider argument
	CallNode        *ast.Node      // The full Layer.mergeAll(...) call expression node
	ProviderArg     *ast.Node      // The specific argument node that is the dependency provider
	ProviderIndex   int            // Index of the provider argument in the call's argument list
	AllArgs       []*ast.Node // All arguments of the mergeAll call
	ProvidedTypes string      // Formatted string of provided type names
}

// AnalyzeLayerMergeAllWithDependencies finds all Layer.mergeAll calls with
// interdependencies where one layer provides a service required by another.
func AnalyzeLayerMergeAllWithDependencies(c *checker.Checker, sf *ast.SourceFile) []LayerMergeAllWithDependenciesMatch {
	var matches []LayerMergeAllWithDependenciesMatch

	// Stack-based traversal
	nodeToVisit := make([]*ast.Node, 0)
	pushChild := func(child *ast.Node) bool {
		nodeToVisit = append(nodeToVisit, child)
		return false
	}
	sf.AsNode().ForEachChild(pushChild)

	for len(nodeToVisit) > 0 {
		node := nodeToVisit[len(nodeToVisit)-1]
		nodeToVisit = nodeToVisit[:len(nodeToVisit)-1]

		if node.Kind == ast.KindCallExpression {
			if result := analyzeLayerMergeAllCall(c, sf, node); len(result) > 0 {
				matches = append(matches, result...)
			}
		}

		// Enqueue children
		node.ForEachChild(pushChild)
	}

	return matches
}

// layerInfo holds parsed layer information for a mergeAll argument.
type layerInfo struct {
	arg              *ast.Node
	requirementsType *checker.Type
}

// analyzeLayerMergeAllCall checks a call expression for Layer.mergeAll interdependencies.
func analyzeLayerMergeAllCall(c *checker.Checker, sf *ast.SourceFile, node *ast.Node) []LayerMergeAllWithDependenciesMatch {
	if node.Kind != ast.KindCallExpression {
		return nil
	}
	call := node.AsCallExpression()

	// Check if this is Layer.mergeAll
	if !typeparser.IsNodeReferenceToEffectLayerModuleApi(c, call.Expression, "mergeAll") {
		return nil
	}

	args := call.Arguments
	if args == nil || len(args.Nodes) < 2 {
		return nil
	}

	// Verify this is a property access expression (X.mergeAll shape)
	if call.Expression.Kind != ast.KindPropertyAccessExpression {
		return nil
	}

	// Parse all layer arguments
	var layerInfos []layerInfo
	// Map of actually provided types -> argument node that provides them
	actuallyProvidedMap := make(map[*checker.Type]*ast.Node)

	for _, arg := range args.Nodes {
		argType := checkerutils.GetTypeAtLocation(c, arg)
		if argType == nil {
			continue
		}

		layer := typeparser.LayerType(c, argType, arg)
		if layer == nil {
			continue
		}

		// Unroll union members for provided types (ROut)
		providedMembers := typeparser.UnrollUnionMembers(layer.ROut)

		// Filter out never types and pass-through types
		for _, providedType := range providedMembers {
			if providedType.Flags()&checker.TypeFlagsNever != 0 {
				continue
			}
			// A pass-through type is both provided (ROut) and required (RIn) by the same layer
			if checker.Checker_isTypeAssignableTo(c, providedType, layer.RIn) {
				continue
			}
			actuallyProvidedMap[providedType] = arg
		}

		layerInfos = append(layerInfos, layerInfo{
			arg:              arg,
			requirementsType: layer.RIn,
		})
	}

	// Check for interdependencies: build a map of provider -> consumed types
	type providerConsumer struct {
		providedType *checker.Type
	}
	providerToConsumers := make(map[*ast.Node][]providerConsumer)

	for _, li := range layerInfos {
		for providedType, providerArg := range actuallyProvidedMap {
			// Skip self-references
			if providerArg == li.arg {
				continue
			}
			// Check if this provided type satisfies the layer's requirements
			if checker.Checker_isTypeAssignableTo(c, providedType, li.requirementsType) {
				providerToConsumers[providerArg] = append(providerToConsumers[providerArg], providerConsumer{
					providedType: providedType,
				})
			}
		}
	}

	// Build matches for providers, iterating in argument order for determinism
	var matches []LayerMergeAllWithDependenciesMatch
	for argIndex, arg := range args.Nodes {
		consumers, ok := providerToConsumers[arg]
		if !ok || len(consumers) == 0 {
			continue
		}

		// Collect unique type names
		seen := make(map[string]bool)
		var typeNames []string
		for _, consumer := range consumers {
			name := c.TypeToString(consumer.providedType)
			if !seen[name] {
				seen[name] = true
				typeNames = append(typeNames, name)
			}
		}

		matches = append(matches, LayerMergeAllWithDependenciesMatch{
			SourceFile:      sf,
			Location:        scanner.GetErrorRangeForNode(sf, arg),
			CallNode:        node,
			ProviderArg:     arg,
			ProviderIndex:   argIndex,
			AllArgs:       args.Nodes,
			ProvidedTypes: strings.Join(typeNames, ", "),
		})
	}

	return matches
}
