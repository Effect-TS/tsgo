use ets_layergraph::ExtractLayerGraphOptions;
use ets_shim::ast::{Node, SourceFile};
use ets_shim::checker::Type;
use ets_shim::core::P;

use crate::*;

// go: etsgoapi/layer_magic.go:11 LayerMagicNode
// LayerMagicNode represents a layer expression annotated with the combinator
// choice needed to compose it.
#[derive(Clone, Debug)]
pub struct LayerMagicNode {
    pub merges: bool,
    pub provides: bool,
    pub node: P<Node>,
    pub provided_types: Vec<P<Type>>,
    pub actual_provided_types: Vec<P<Type>>,
    pub required_types: Vec<P<Type>>,
}

// go: etsgoapi/layer_magic.go:21 LayerMagicResult
// LayerMagicResult holds the layer magic extraction result.
#[derive(Clone, Debug, Default)]
pub struct LayerMagicResult {
    pub nodes: Vec<LayerMagicNode>,
    pub missing_output_types: Vec<P<Type>>,
}

impl TypeParser<'_> {
    // go: etsgoapi/layer_magic.go:29 TypeParser.ExtractLayerMagic
    // ExtractLayerMagic extracts layer magic from nodes that already represent layers.
    // It does not decompose pipe/call/array expressions; callers are responsible for
    // passing the layer nodes they want included and the target output types.
    //
    // Rust: Go's nil result is `None`. Go also returns nil for a nil outline graph, which `ExtractOutlineGraph`
    // never returns (the Rust one returns the graph by value).
    pub fn extract_layer_magic(
        &mut self,
        source_file: impl Into<Option<P<SourceFile>>>,
        nodes: &[P<Node>],
        target_output_types: &[P<Type>],
    ) -> Option<LayerMagicResult> {
        let source_file = source_file.into()?;
        if nodes.is_empty() {
            return None;
        }

        let full_graph = ets_layergraph::extract_layer_graph(
            &mut self.inner,
            nodes,
            Some(source_file),
            ExtractLayerGraphOptions { skip_explode: true, ..Default::default() },
        );
        let outline_graph = ets_layergraph::extract_outline_graph(&mut self.inner, &full_graph);

        layer_magic_result_from_internal(Some(
            ets_layergraph::convert_outline_graph_to_layer_magic(
                &self.inner,
                &outline_graph,
                target_output_types,
            ),
        ))
    }
}

// go: etsgoapi/layer_magic.go:45 layerMagicResultFromInternal
pub(crate) fn layer_magic_result_from_internal(
    result: Option<ets_layergraph::LayerMagicResult>,
) -> Option<LayerMagicResult> {
    let result = result?;
    let mut nodes = Vec::with_capacity(result.nodes.len());
    for node in result.nodes {
        nodes.push(LayerMagicNode {
            merges: node.merges,
            provides: node.provides,
            node: node.node,
            provided_types: node.provided_types,
            actual_provided_types: node.actual_provided_types,
            required_types: node.required_types,
        });
    }
    Some(LayerMagicResult { nodes, missing_output_types: result.missing_output_types })
}
