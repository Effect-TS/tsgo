// Package layergraph models how Effect Layers are composed in source code.
// It builds directed graphs where nodes represent Layer expressions and edges
// represent composition relationships (pipe, call, array literal, symbol).
package layergraph

import (
	"encoding/json"

	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// LayerGraphNodeInfo holds data for each node in the layer graph.
type LayerGraphNodeInfo struct {
	Node           *ast.Node        // The AST expression node
	DisplayNode    *ast.Node        // The node used for display (e.g. variable name instead of initializer)
	LayerType      *typeparser.Layer // Parsed Layer type (ROut, E, RIn) or nil
	Provides       []*checker.Type  // Individual service types from ROut (unrolled intersection, Never filtered)
	ActualProvides []*checker.Type  // Provides minus pass-through (types also in Requires)
	Requires       []*checker.Type  // Individual service types from RIn (unrolled intersection, Never filtered)
}

// EdgeRelationship is the type of relationship between two nodes in the layer graph.
type EdgeRelationship string

const (
	EdgeRelationshipCall         EdgeRelationship = "call"
	EdgeRelationshipPipe         EdgeRelationship = "pipe"
	EdgeRelationshipArrayLiteral EdgeRelationship = "arrayLiteral"
	EdgeRelationshipSymbol       EdgeRelationship = "symbol"
)

// LayerGraphEdgeInfo describes the relationship between two nodes in the layer graph.
// It is a discriminated union keyed on Relationship.
type LayerGraphEdgeInfo struct {
	Relationship  EdgeRelationship // One of: "call", "pipe", "arrayLiteral", "symbol"
	ArgumentIndex int              // Only meaningful for "call" edges (0-based argument position)
	Index         int              // Only meaningful for "arrayLiteral" edges (0-based element position)
}

// MarshalJSON produces JSON matching the reference format, only including
// fields relevant to each relationship type:
//
//	{"relationship":"call","argumentIndex":0}
//	{"relationship":"pipe"}
//	{"relationship":"arrayLiteral","index":0}
//	{"relationship":"symbol"}
func (e LayerGraphEdgeInfo) MarshalJSON() ([]byte, error) {
	switch e.Relationship {
	case EdgeRelationshipCall:
		return json.Marshal(struct {
			Relationship  EdgeRelationship `json:"relationship"`
			ArgumentIndex int              `json:"argumentIndex"`
		}{e.Relationship, e.ArgumentIndex})
	case EdgeRelationshipArrayLiteral:
		return json.Marshal(struct {
			Relationship EdgeRelationship `json:"relationship"`
			Index        int              `json:"index"`
		}{e.Relationship, e.Index})
	default:
		return json.Marshal(struct {
			Relationship EdgeRelationship `json:"relationship"`
		}{e.Relationship})
	}
}

// LayerOutlineGraphNodeInfo holds data for nodes in the simplified outline graph.
type LayerOutlineGraphNodeInfo struct {
	Node           *ast.Node       // The AST node
	DisplayNode    *ast.Node       // The node used for display
	Provides       []*checker.Type // Service types provided
	ActualProvides []*checker.Type // Actual (non-pass-through) provides
	Requires       []*checker.Type // Service types required
}

// LayerMagicNode represents a single node in the layer magic result,
// annotated with flags that determine which Layer.* combinator to use.
type LayerMagicNode struct {
	Merges   bool      // Whether this node should be merged (provides a target output type)
	Provides bool      // Whether this node provides services
	Node     *ast.Node // The layer expression AST node
}

// LayerMagicResult holds the output of ConvertOutlineGraphToLayerMagic.
type LayerMagicResult struct {
	Nodes              []LayerMagicNode // Ordered list of layer nodes with merge/provide flags
	MissingOutputTypes []*checker.Type  // Target output types not satisfied by any node
}

// ProviderRequirerKind distinguishes between providers and requirers.
type ProviderRequirerKind string

const (
	ProviderRequirerKindProvided ProviderRequirerKind = "provided"
	ProviderRequirerKindRequired ProviderRequirerKind = "required"
)

// ProviderRequirerInfo summarizes which leaf layers provide or require a service type.
type ProviderRequirerInfo struct {
	Kind         ProviderRequirerKind // "provided" or "required"
	Type         *checker.Type        // The service type
	Nodes        []*ast.Node          // The leaf nodes that provide/require this type
	DisplayNodes []*ast.Node          // Display nodes for those leaves
}

// ExtractLayerGraphOptions controls the behavior of ExtractLayerGraph.
type ExtractLayerGraphOptions struct {
	ArrayLiteralAsMerge   bool // Treat array literals [l1, l2] as implicit Layer.mergeAll (default: false)
	ExplodeOnlyLayerCalls bool // Only explode calls to Layer module APIs (default: false)
	FollowSymbolsDepth    int  // How many levels deep to follow identifier references (default: 0)
}
