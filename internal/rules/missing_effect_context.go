// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"fmt"
	"strings"

	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/layergraph"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// MissingEffectContext detects when an Effect has context requirements that are not
// handled by the expected type. This happens when assigning an Effect with requirements
// to a variable/parameter expecting an Effect with fewer or no requirements.
var MissingEffectContext = rule.Rule{
	Name:            "missingEffectContext",
	Group:           "correctness",
	Description:     "Detects Effect values with unhandled context requirements",
	DefaultSeverity: etscore.SeverityError,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Missing_context_0_in_the_expected_Effect_type_effect_missingEffectContext.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic

		for _, re := range ctx.Checker.GetRelationErrors(ctx.SourceFile) {
			// Parse both types as Effects
			srcEffect := typeparser.EffectType(ctx.Checker, re.Source, re.ErrorNode)
			tgtEffect := typeparser.EffectType(ctx.Checker, re.Target, re.ErrorNode)

			// Both must be Effect types
			if srcEffect == nil || tgtEffect == nil {
				continue
			}

			// Find unhandled context types by checking each source requirement member
			// against the target requirement type
			unhandledContexts := findUnhandledContexts(ctx.Checker, srcEffect.R, tgtEffect.R)
			if len(unhandledContexts) > 0 {
				contextTypeStr := formatContextTypes(ctx.Checker, unhandledContexts)
				diag := ctx.NewDiagnostic(
					ctx.SourceFile,
					ctx.GetErrorRange(re.ErrorNode),
					tsdiag.Missing_context_0_in_the_expected_Effect_type_effect_missingEffectContext,
					missingEffectContextRelatedInformation(ctx.Checker, ctx, re.ErrorNode, unhandledContexts),
					contextTypeStr,
				)
				diags = append(diags, diag)
			}
		}

		return diags
	},
}

// findUnhandledContexts returns the source context types that are not assignable to the target context type.
func findUnhandledContexts(c *checker.Checker, srcR, tgtR *checker.Type) []*checker.Type {
	// Unroll source context union into individual members
	srcMembers := typeparser.UnrollUnionMembers(srcR)

	var unhandled []*checker.Type
	for _, member := range srcMembers {
		// Check if this specific member is assignable to target
		if !checker.Checker_isTypeAssignableTo(c, member, tgtR) {
			unhandled = append(unhandled, member)
		}
	}
	return unhandled
}

// formatContextTypes formats a slice of context types as a union string (e.g., "EnvA | EnvB").
func formatContextTypes(c *checker.Checker, types []*checker.Type) string {
	if len(types) == 0 {
		return ""
	}
	if len(types) == 1 {
		return c.TypeToString(types[0])
	}
	var result strings.Builder
	result.WriteString(c.TypeToString(types[0]))
	for i := 1; i < len(types); i++ {
		result.WriteString(" | ")
		result.WriteString(c.TypeToString(types[i]))
	}
	return result.String()
}

func missingEffectContextRelatedInformation(c *checker.Checker, ctx *rule.Context, errorNode *ast.Node, missingTypes []*checker.Type) []*ast.Diagnostic {
	provideLocation := findRelatedProvideLocation(c, ctx.SourceFile, errorNode)
	if provideLocation.Node == nil || provideLocation.LayerNode == nil {
		return nil
	}
	layerDiagnostics := findRelatedLayerProviderDiagnostics(c, ctx, provideLocation.LayerNode, missingTypes)
	if len(layerDiagnostics) == 0 {
		return nil
	}
	provideDiagnostic := ctx.NewDiagnostic(
		ctx.SourceFile,
		provideLocation.Location,
		tsdiag.Adjusting_this_layer_composition_could_provide_the_missing_service_effect_missingEffectContext,
		nil,
	)
	relatedInformation := make([]*ast.Diagnostic, 0, 1+len(layerDiagnostics))
	relatedInformation = append(relatedInformation, provideDiagnostic)
	relatedInformation = append(relatedInformation, layerDiagnostics...)
	return relatedInformation
}

type provideLocation struct {
	Node      *ast.Node
	LayerNode *ast.Node
	Location  core.TextRange
}

func findRelatedProvideLocation(c *checker.Checker, sf *ast.SourceFile, errorNode *ast.Node) provideLocation {
	if c == nil || sf == nil || errorNode == nil {
		return provideLocation{}
	}
	flows := typeparser.PipingFlows(c, sf, true)
	for _, flow := range flows {
		if flow == nil || flow.Node == nil {
			continue
		}
		if !nodeContains(flow.Node, errorNode) {
			continue
		}
		errorTransformationIndex := findTransformationIndexContainingNode(flow, errorNode)
		if errorTransformationIndex < 0 {
			continue
		}
		for i := errorTransformationIndex - 1; i >= 0; i-- {
			transformation := flow.Transformations[i]
			if !typeparser.IsNodeReferenceToEffectModuleApi(c, transformation.Callee, "provide") {
				continue
			}
			callNode := transformation.Node
			if callNode == nil {
				callNode = transformation.Callee
			}
			return provideLocation{
				Node:      callNode,
				LayerNode: firstNode(transformation.Args),
				Location:  scanner.GetErrorRangeForNode(sf, callNode),
			}
		}
	}
	return provideLocation{}
}

func findRelatedLayerProviderDiagnostics(
	c *checker.Checker,
	ctx *rule.Context,
	layerNode *ast.Node,
	missingTypes []*checker.Type,
) []*ast.Diagnostic {
	if c == nil || ctx == nil || layerNode == nil || len(missingTypes) == 0 {
		return nil
	}
	rootLayerProvides := rootLayerProvidesTypes(c, layerNode)
	fullGraph := layergraph.ExtractLayerGraph(c, layerNode, ctx.SourceFile, layergraph.ExtractLayerGraphOptions{
		FollowSymbolsDepth: 2,
	})
	outlineGraph := layergraph.ExtractOutlineGraph(c, fullGraph)
	if outlineGraph == nil {
		return nil
	}
	var related []*ast.Diagnostic
	seen := make(map[string]struct{})
	for _, missingType := range missingTypes {
		if missingType == nil {
			continue
		}
		if rootProvidesType(c, rootLayerProvides, missingType) {
			continue
		}
		missingTypeText := c.TypeToString(missingType)
		for _, nodeInfo := range outlineGraph.Nodes() {
			displayNode := nodeInfo.DisplayNode
			if displayNode == nil {
				displayNode = nodeInfo.Node
			}
			if displayNode == nil || !outlineNodeProvidesType(c, nodeInfo, missingType) {
				continue
			}
			key := missingTypeText
			if displayNode.Pos() >= 0 {
				key += fmt.Sprintf(":%d", displayNode.Pos())
			}
			if _, ok := seen[key]; ok {
				continue
			}
			seen[key] = struct{}{}
			related = append(related, ctx.NewDiagnostic(
				ast.GetSourceFileOfNode(displayNode),
				scanner.GetErrorRangeForNode(ast.GetSourceFileOfNode(displayNode), displayNode),
				tsdiag.This_layer_provides_the_missing_service_0_effect_missingEffectContext,
				nil,
				missingTypeText,
			))
		}
	}
	return related
}

func rootLayerProvidesTypes(c *checker.Checker, layerNode *ast.Node) []*checker.Type {
	if c == nil || layerNode == nil {
		return nil
	}
	layerType := typeparser.LayerType(c, typeparser.GetTypeAtLocation(c, layerNode), layerNode)
	if layerType == nil {
		return nil
	}
	return typeparser.UnrollUnionMembers(layerType.ROut)
}

func rootProvidesType(c *checker.Checker, providedTypes []*checker.Type, target *checker.Type) bool {
	for _, providedType := range providedTypes {
		if providedType == target {
			return true
		}
		if checker.Checker_isTypeAssignableTo(c, providedType, target) &&
			checker.Checker_isTypeAssignableTo(c, target, providedType) {
			return true
		}
	}
	return false
}

func outlineNodeProvidesType(c *checker.Checker, node layergraph.LayerOutlineGraphNodeInfo, target *checker.Type) bool {
	for _, providedType := range node.ActualProvides {
		if providedType == target {
			return true
		}
		if checker.Checker_isTypeAssignableTo(c, providedType, target) &&
			checker.Checker_isTypeAssignableTo(c, target, providedType) {
			return true
		}
	}
	return false
}

func firstNode(nodes []*ast.Node) *ast.Node {
	if len(nodes) == 0 {
		return nil
	}
	return nodes[0]
}

func findTransformationIndexContainingNode(flow *typeparser.PipingFlow, target *ast.Node) int {
	if flow == nil || target == nil {
		return -1
	}
	for i := range flow.Transformations {
		transformation := flow.Transformations[i]
		if transformation.Node == target || transformation.Callee == target {
			return i
		}
	}
	return -1
}

func nodeContains(ancestor *ast.Node, target *ast.Node) bool {
	if ancestor == nil || target == nil {
		return false
	}
	return ancestor.Pos() <= target.Pos() && target.End() <= ancestor.End()
}
