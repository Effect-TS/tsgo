package etsapi

import (
	"github.com/effect-ts/tsgo/internal/layergraph"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// LayerMagicNode represents a layer expression annotated with the combinator
// choice needed to compose it.
type LayerMagicNode struct {
	Merges   bool
	Provides bool
	Node     *ast.Node
}

// LayerMagicResult holds the layer magic extraction result.
type LayerMagicResult struct {
	Nodes              []LayerMagicNode
	MissingOutputTypes []*checker.Type
}

// ExtractLayerMagic extracts layer magic from nodes that already represent layers.
// It does not decompose pipe/call/array expressions; callers are responsible for
// passing the layer nodes they want included and the target output types.
func (tp *TypeParser) ExtractLayerMagic(sourceFile *ast.SourceFile, nodes []*ast.Node, targetOutputTypes []*checker.Type) *LayerMagicResult {
	if tp == nil || tp.inner == nil || tp.checker == nil || sourceFile == nil || len(nodes) == 0 {
		return nil
	}

	fullGraph := layergraph.ExtractLayerGraph(tp.inner, tp.checker, nodes, sourceFile, layergraph.ExtractLayerGraphOptions{
		SkipExplode: true,
	})
	outlineGraph := layergraph.ExtractOutlineGraph(tp.checker, fullGraph)
	if outlineGraph == nil {
		return nil
	}

	return layerMagicResultFromInternal(layergraph.ConvertOutlineGraphToLayerMagic(tp.inner, outlineGraph, targetOutputTypes))
}

func layerMagicResultFromInternal(result *layergraph.LayerMagicResult) *LayerMagicResult {
	if result == nil {
		return nil
	}
	nodes := make([]LayerMagicNode, 0, len(result.Nodes))
	for _, node := range result.Nodes {
		nodes = append(nodes, LayerMagicNode{
			Merges:   node.Merges,
			Provides: node.Provides,
			Node:     node.Node,
		})
	}
	return &LayerMagicResult{
		Nodes:              nodes,
		MissingOutputTypes: result.MissingOutputTypes,
	}
}
