use ets_shim::ast::Node;
use ets_shim::checker::Type;
use ets_shim::core::P;
use ets_typeparser::Layer;

// go: internal/layergraph/types.go:15 LayerGraphNodeInfo a1abda71618aa704
// LayerGraphNodeInfo holds data for each node in the layer graph.
//
// Rust: Go's nil and empty type slices are both `Vec::new()`: no in-scope reader tells them apart.
#[derive(Clone, Debug)]
pub struct LayerGraphNodeInfo {
    pub node: P<Node>,                 // The AST expression node
    pub display_node: P<Node>, // The node used for display (e.g. variable name instead of initializer)
    pub layer_type: Option<Layer>, // Parsed Layer type (ROut, E, RIn) or nil
    pub provides: Vec<P<Type>>, // Individual service types from ROut (unrolled intersection, Never filtered)
    pub actual_provides: Vec<P<Type>>, // Provides minus pass-through (types also in Requires)
    pub requires: Vec<P<Type>>, // Individual service types from RIn (unrolled intersection, Never filtered)
}

// go: internal/layergraph/types.go:25 EdgeRelationship 7441560e32339e31
// go: internal/layergraph/types.go:27 EdgeRelationshipCall c46bd37125abb265
// EdgeRelationship is the type of relationship between two nodes in the layer graph.
//
// Rust: Go's `type EdgeRelationship string` is only used through its constants (porting/PORTING.md, "Naming").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EdgeRelationship {
    Call,
    Pipe,
    ArrayLiteral,
    Symbol,
}

// go: internal/layergraph/types.go:36 LayerGraphEdgeInfo 19e636e243c1292d
// LayerGraphEdgeInfo describes the relationship between two nodes in the layer graph.
// It is a discriminated union keyed on Relationship.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LayerGraphEdgeInfo {
    pub relationship: EdgeRelationship, // One of: "call", "pipe", "arrayLiteral", "symbol"
    pub argument_index: i64, // Only meaningful for "call" edges (0-based argument position)
    pub index: i64,          // Only meaningful for "arrayLiteral" edges (0-based element position)
}

// go: internal/layergraph/types.go:25 EdgeRelationship 7441560e32339e31
impl EdgeRelationship {
    /// Rust-only: the Go string value (`string(relationship)`).
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeRelationship::Call => "call",
            EdgeRelationship::Pipe => "pipe",
            EdgeRelationship::ArrayLiteral => "arrayLiteral",
            EdgeRelationship::Symbol => "symbol",
        }
    }
}

impl LayerGraphEdgeInfo {
    /// Rust-only: Go's `LayerGraphEdgeInfo{Relationship: r}` (the other fields zero).
    pub fn new(relationship: EdgeRelationship) -> Self {
        LayerGraphEdgeInfo { relationship, argument_index: 0, index: 0 }
    }

    // go: internal/layergraph/types.go:49 LayerGraphEdgeInfo.MarshalJSON 30067835cd253404
    // MarshalJSON produces JSON matching the reference format, only including
    // fields relevant to each relationship type:
    //
    //	{"relationship":"call","argumentIndex":0}
    //	{"relationship":"pipe"}
    //	{"relationship":"arrayLiteral","index":0}
    //	{"relationship":"symbol"}
    //
    // Rust: Go's `encoding/json` output of these structs, written directly (the relationship strings need no
    // escaping); Go's error is always nil.
    pub fn marshal_json(&self) -> Vec<u8> {
        let relationship = self.relationship.as_str();
        match self.relationship {
            EdgeRelationship::Call => format!(
                "{{\"relationship\":\"{relationship}\",\"argumentIndex\":{}}}",
                self.argument_index
            ),
            EdgeRelationship::ArrayLiteral => {
                format!("{{\"relationship\":\"{relationship}\",\"index\":{}}}", self.index)
            }
            _ => format!("{{\"relationship\":\"{relationship}\"}}"),
        }
        .into_bytes()
    }
}

// go: internal/layergraph/types.go:69 LayerOutlineGraphNodeInfo 0f14578c0af4021f
// LayerOutlineGraphNodeInfo holds data for nodes in the simplified outline graph.
#[derive(Clone, Debug)]
pub struct LayerOutlineGraphNodeInfo {
    pub node: P<Node>,                 // The AST node
    pub display_node: P<Node>,         // The node used for display
    pub provides: Vec<P<Type>>,        // Service types provided
    pub actual_provides: Vec<P<Type>>, // Actual (non-pass-through) provides
    pub requires: Vec<P<Type>>,        // Service types required
}

// go: internal/layergraph/types.go:79 LayerMagicNode e319cdbad58c8b55
// LayerMagicNode represents a single node in the layer magic result,
// annotated with flags that determine which Layer.* combinator to use.
#[derive(Clone, Debug)]
pub struct LayerMagicNode {
    pub merges: bool, // Whether this node should be merged (provides a target output type)
    pub provides: bool, // Whether this node provides services
    pub node: P<Node>, // The layer expression AST node
    pub provided_types: Vec<P<Type>>, // Service types provided
    pub actual_provided_types: Vec<P<Type>>, // Actual (non-pass-through) provides
    pub required_types: Vec<P<Type>>, // Service types required
}

// go: internal/layergraph/types.go:89 LayerMagicResult 99a70178bd27be02
// LayerMagicResult holds the output of ConvertOutlineGraphToLayerMagic.
#[derive(Clone, Debug, Default)]
pub struct LayerMagicResult {
    pub nodes: Vec<LayerMagicNode>, // Ordered list of layer nodes with merge/provide flags
    pub missing_output_types: Vec<P<Type>>, // Target output types not satisfied by any node
}

// go: internal/layergraph/types.go:95 ProviderRequirerKind a8e214157b0052ef
// go: internal/layergraph/types.go:97 ProviderRequirerKindProvided 792c9d92a7bb9ab0
// ProviderRequirerKind distinguishes between providers and requirers.
//
// Rust: Go's `type ProviderRequirerKind string` with its two constants, the only values it takes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ProviderRequirerKind {
    Provided,
    Required,
}

// go: internal/layergraph/types.go:95 ProviderRequirerKind a8e214157b0052ef
impl ProviderRequirerKind {
    /// Rust-only: the Go string value (`string(kind)`).
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderRequirerKind::Provided => "provided",
            ProviderRequirerKind::Required => "required",
        }
    }
}

// go: internal/layergraph/types.go:103 ProviderRequirerInfo ca75da09cbd993e7
// ProviderRequirerInfo summarizes which leaf layers provide or require a service type.
#[derive(Clone, Debug)]
pub struct ProviderRequirerInfo {
    pub kind: ProviderRequirerKind,  // "provided" or "required"
    pub type_: P<Type>,              // The service type
    pub nodes: Vec<P<Node>>,         // The leaf nodes that provide/require this type
    pub display_nodes: Vec<P<Node>>, // Display nodes for those leaves
}

// go: internal/layergraph/types.go:111 ExtractLayerGraphOptions b259ab0facc808ff
// ExtractLayerGraphOptions controls the behavior of ExtractLayerGraph.
#[derive(Clone, Copy, Default, Debug)]
pub struct ExtractLayerGraphOptions {
    pub array_literal_as_merge: bool, // Treat array literals [l1, l2] as implicit Layer.mergeAll (default: false)
    pub explode_only_layer_calls: bool, // Only explode calls to Layer module APIs (default: false)
    pub follow_symbols_depth: i64, // How many levels deep to follow identifier references (default: 0)
    pub skip_explode: bool,        // Do not decompose pipe/call/array expressions (default: false)
}
