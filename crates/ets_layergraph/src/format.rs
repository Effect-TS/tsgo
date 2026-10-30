use ets_graph::{Graph, MermaidOptions};
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::checker::{Checker, Type, TypeFormatFlags};
use ets_shim::core::P;

use crate::*;

// go: internal/layergraph/format.go:16 mermaidEntityEncode 71d245321b180e03
// mermaidEntityEncode replaces Mermaid-special characters with Mermaid HTML entity codes.
// Uses single-pass character-by-character replacement to avoid double-encoding.
pub(crate) fn mermaid_entity_encode(s: &str) -> String {
    let mut b = String::with_capacity(s.len());
    for r in s.chars() {
        match r {
            '#' => b.push_str("#35;"),
            '"' => b.push_str("#quot;"),
            '<' => b.push_str("#lt;"),
            '>' => b.push_str("#gt;"),
            '(' => b.push_str("#40;"),
            ')' => b.push_str("#41;"),
            '{' => b.push_str("#123;"),
            '}' => b.push_str("#125;"),
            _ => b.push(r),
        }
    }
    b
}

/// Rust-only: Go's `s[:n]` on a string longer than `n` bytes. Go keeps a split UTF-8 sequence as invalid bytes;
/// Rust strings cannot hold them, so a split character becomes U+FFFD (porting/DIVERGENCES.md).
fn truncate_bytes(s: &str, n: usize) -> String {
    String::from_utf8_lossy(&s.as_bytes()[..n]).into_owned()
}

// go: internal/layergraph/format.go:46 mermaidSafe 86053efb26d03216
// mermaidSafe encodes a string for use in Mermaid subgraph headers (nested format).
// It normalizes whitespace, truncates, and encodes only quotes and angle brackets.
//
// Rust: Go's `strings.Fields` and `strings.TrimSpace` split on `unicode.IsSpace`, the same set as Rust's
// `char::is_whitespace` (Unicode White_Space).
pub(crate) fn mermaid_safe(value: &str) -> String {
    let mut value = value.replace('\n', " ");
    // Collapse whitespace to single spaces.
    let parts: Vec<&str> = value.split_whitespace().collect();
    value = parts.join(" ");
    if value.len() > 50 {
        value = truncate_bytes(&value, 50);
    }
    value = value.replace('"', "#quot;");
    value = value.replace('<', "#lt;");
    value = value.replace('>', "#gt;");
    value.trim().to_string()
}

// go: internal/layergraph/format.go:62 getNodeText e64e2878ad79794b
// getNodeText returns the trimmed source text of a node.
//
// Rust: Go reads the text of a nil source file (a nil dereference); every node here is in a source file.
pub(crate) fn get_node_text(node: P<Node>, sf: Option<P<SourceFile>>) -> String {
    let sf = sf.expect("a layer graph node belongs to a source file");
    if let Some(parent) = node.parent()
        && parent.kind() == Kind::PropertyDeclaration
        && parent.name() == Some(node)
        && let Some(grandparent) = parent.parent()
        && grandparent.kind() == Kind::ClassDeclaration
        && let Some(class_name) = grandparent.name()
    {
        let class_text =
            ets_shim::scanner::get_source_text_of_node_from_source_file(sf, class_name, false)
                .trim()
                .to_string();
        let prop_text =
            ets_shim::scanner::get_source_text_of_node_from_source_file(sf, node, false)
                .trim()
                .to_string();
        if !class_text.is_empty() && !prop_text.is_empty() {
            return class_text + "." + &prop_text;
        }
    }
    ets_shim::scanner::get_source_text_of_node_from_source_file(sf, node, false).trim().to_string()
}

// go: internal/layergraph/format.go:76 getNodeSourceFile d216f6afca460ed1
// getNodeSourceFile walks up the parent chain to find the SourceFile ancestor.
pub(crate) fn get_node_source_file(node: P<Node>) -> Option<P<SourceFile>> {
    let mut n = Some(node);
    while let Some(current) = n {
        if current.kind() == Kind::SourceFile {
            return Some(current.as_source_file_p());
        }
        n = current.parent();
    }
    None
}

// go: internal/layergraph/format.go:90 formatLocation 2355472c40ffc9de
// formatLocation returns a human-readable location string for a node.
// If the node is in the same source file as fromSourceFile, returns "ln {line} col {character}".
// If different, returns "in {filename} at ln {line} col {character}".
pub(crate) fn format_location(node: P<Node>, from_source_file: Option<P<SourceFile>>) -> String {
    let Some(node_source_file) = get_node_source_file(node) else {
        return String::new();
    };
    let pos = ets_shim::scanner::get_token_pos_of_node(node, node_source_file, false);
    let (line, character) =
        ets_shim::scanner::get_ecma_line_and_utf16_character_of_position(&*node_source_file, pos);
    if from_source_file.is_none_or(|from| node_source_file == from) {
        return format!("ln {} col {}", line + 1, character);
    }
    let mut file_name = node_source_file.file_name();
    if let Some(idx) = file_name.rfind('/') {
        file_name = &file_name[idx + 1..];
    }
    format!("in {} at ln {} col {}", file_name, line + 1, character)
}

// go: internal/layergraph/format.go:108 getTypeDisplayName 7dd3772959ffae07
// getTypeDisplayName returns the string representation of a type with no truncation.
pub(crate) fn get_type_display_name(c: &mut Checker, t: P<Type>) -> String {
    c.type_to_string_ex(t, None, TypeFormatFlags::NoTruncation, None)
}

// go: internal/layergraph/format.go:114 mermaidEscapeOutputLabel 464782c5e6877e5f
// mermaidEscapeOutputLabel applies standard Mermaid escaping for backslash, brackets, and pipe
// on top of entity-encoded text. This is used for the output format.
pub(crate) fn mermaid_escape_output_label(s: &str) -> String {
    let mut s = s.replace('\\', "\\\\");
    s = s.replace('[', "\\[");
    s = s.replace(']', "\\]");
    s = s.replace('|', "\\|");
    s
}

// go: internal/layergraph/format.go:128 FormatLayerGraph e8b6226aa2e4e806
// FormatLayerGraph renders the full layer graph as a Mermaid flowchart TD with entity-encoded labels.
// Intentional divergence from .repos reference: this output format includes cross-file location
// annotations (e.g., "_in simple.ts at ln 6 col 19_") on nodes originating from a different source
// file. The .repos reference omits location annotations from the output (flat graph) format, only
// including them in nested, outline, and quickinfo formats. The Go version adds them here as an
// enhancement to help users locate cross-file nodes in the flat graph.
pub fn format_layer_graph(
    c: &mut Checker,
    layer_graph: &Graph<LayerGraphNodeInfo, LayerGraphEdgeInfo>,
    sf: Option<P<SourceFile>>,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push("flowchart TD".to_string());

    // Nodes in index order.
    for (idx, node_info) in layer_graph.nodes() {
        let node_source_file = get_node_source_file(node_info.node);
        let text = get_node_text(node_info.node, node_source_file);

        let mut provides = Vec::new();
        for &t in &node_info.provides {
            provides.push(get_type_display_name(c, t));
        }
        let mut requires = Vec::new();
        for &t in &node_info.requires {
            requires.push(get_type_display_name(c, t));
        }

        let mut label =
            text + "\nprovides: " + &provides.join(", ") + "\nrequires: " + &requires.join(", ");
        if let Some(node_source_file) = node_source_file
            && Some(node_source_file) != sf
        {
            label += "\n_";
            label += &format_location(node_info.node, sf);
            label += "_";
        }
        label = mermaid_entity_encode(&label);
        label = label.replace('\n', "<br/>");
        label = mermaid_escape_output_label(&label);
        lines.push(format!("  {idx}[\"{label}\"]"));
    }

    // Edges in index order.
    for (_, edge) in layer_graph.edges() {
        let json_bytes = edge.data.marshal_json();
        let edge_label = mermaid_entity_encode(&String::from_utf8_lossy(&json_bytes));
        let edge_label = mermaid_escape_output_label(&edge_label);
        lines.push(format!("  {} -->|\"{edge_label}\"| {}", edge.source, edge.target));
    }

    lines.join("\n")
}

// go: internal/layergraph/format.go:175 FormatNestedLayerGraph 490b0b3b16189b97
// FormatNestedLayerGraph renders the layer graph as a Mermaid flowchart TB with nested subgraphs.
pub fn format_nested_layer_graph(
    c: &mut Checker,
    layer_graph: &Graph<LayerGraphNodeInfo, LayerGraphEdgeInfo>,
    sf: Option<P<SourceFile>>,
) -> String {
    let mut result: Vec<String> = Vec::new();

    // Create subgraph boxes for each node.
    for (idx, node_info) in layer_graph.nodes() {
        let mut subgraph_defs: Vec<String> = Vec::new();

        for kind in ["requires", "provides"] {
            let types = if kind == "requires" { &node_info.requires } else { &node_info.provides };

            let mut type_nodes: Vec<String> = Vec::new();
            for (i, &t) in types.iter().enumerate() {
                let type_name = get_type_display_name(c, t);
                type_nodes.push(format!("{idx}_{kind}_{i}[\"{}\"]", mermaid_safe(&type_name)));
            }

            if !type_nodes.is_empty() {
                let mut kind_label = "Requires";
                if kind == "provides" {
                    kind_label = "Provides";
                }
                subgraph_defs.push(format!("subgraph {idx}_{kind} [{kind_label}]"));
                for tn in type_nodes {
                    subgraph_defs.push(format!("  {tn}"));
                }
                subgraph_defs.push("end".to_string());
                subgraph_defs.push(format!("style {idx}_{kind} stroke:none"));
            }
        }

        // Wrap in inner wrap subgraph.
        let mut wrapped: Vec<String> = Vec::new();
        wrapped.push(format!("subgraph {idx}_wrap[\" \"]"));
        for line in &subgraph_defs {
            wrapped.push(format!("  {line}"));
        }
        wrapped.push("end".to_string());
        wrapped.push(format!("style {idx}_wrap fill:transparent"));
        wrapped.push(format!("style {idx}_wrap stroke:none"));

        // Get display node text and location.
        let display_node = node_info.display_node;
        let display_source_file = get_node_source_file(display_node);
        let node_text = get_node_text(display_node, display_source_file);
        let location = format_location(display_node, sf);

        // Outer subgraph.
        result.push(format!(
            "subgraph {idx} [\"`{}<br/><small>_{}_</small>`\"]",
            mermaid_safe(&node_text),
            mermaid_safe(&location)
        ));
        for line in &wrapped {
            result.push(format!("  {line}"));
        }
        result.push("end".to_string());
        result.push(format!("style {idx} fill:transparent"));
    }

    // Create edges between matching type nodes.
    for (_, edge) in layer_graph.edges() {
        // Go: GetNode of an edge's endpoints always succeeds.
        let source_data = layer_graph.get_node(edge.source).expect("an edge source node");
        let target_data = layer_graph.get_node(edge.target).expect("an edge target node");

        let mut connected = false;
        for kind in ["requires", "provides"] {
            let (source_types, target_types) = if kind == "requires" {
                (&source_data.requires, &target_data.requires)
            } else {
                (&source_data.provides, &target_data.provides)
            };

            for (i, source_type) in source_types.iter().enumerate() {
                for (j, target_type) in target_types.iter().enumerate() {
                    if source_type == target_type {
                        result.push(format!(
                            "{}_{kind}_{i} -.-> {}_{kind}_{j}",
                            edge.source, edge.target
                        ));
                        connected = true;
                        break;
                    }
                }
            }
        }

        if !connected {
            result.push(format!("{} -.-x {}", edge.source, edge.target));
        }
    }

    if result.is_empty() {
        return String::new();
    }

    let mut lines: Vec<String> = Vec::new();
    lines.push("flowchart TB".to_string());
    for line in &result {
        lines.push(format!("  {line}"));
    }
    lines.join("\n")
}

// go: internal/layergraph/format.go:284 FormatOutlineGraph dfdc522464d90dba
// FormatOutlineGraph renders the outline graph as a Mermaid flowchart TD.
pub fn format_outline_graph(
    _c: &mut Checker,
    outline_graph: &Graph<LayerOutlineGraphNodeInfo, ()>,
    sf: Option<P<SourceFile>>,
) -> String {
    outline_graph.to_mermaid(MermaidOptions {
        direction: "TD".to_string(),
        node_label: Box::new(move |node_info: &LayerOutlineGraphNodeInfo| {
            let display_node = node_info.display_node;
            let display_source_file = get_node_source_file(display_node);
            let node_text =
                mermaid_entity_encode(&get_node_text(display_node, display_source_file));
            if display_source_file == sf {
                return node_text;
            }
            node_text + "\n_" + &mermaid_entity_encode(&format_location(display_node, sf)) + "_"
        }),
        node_shape: None,
        edge_label: Box::new(|(): &()| String::new()),
        edge_shape: None,
    })
}

// go: internal/layergraph/format.go:307 FormatQuickInfo bc0c340b28b4ab6a
// FormatQuickInfo renders provider/requirer info as a JSDoc comment.
pub fn format_quick_info(
    c: &mut Checker,
    providers_and_requirers: &[ProviderRequirerInfo],
    sf: Option<P<SourceFile>>,
) -> String {
    if providers_and_requirers.is_empty() {
        return String::new();
    }

    let mut provided_items: Vec<&ProviderRequirerInfo> = Vec::new();
    let mut required_items: Vec<&ProviderRequirerInfo> = Vec::new();
    for item in providers_and_requirers {
        if item.kind == ProviderRequirerKind::Provided {
            provided_items.push(item);
        } else {
            required_items.push(item);
        }
    }

    let mut textual_explanation: Vec<String> = Vec::new();

    let mut append_info = |item: &ProviderRequirerInfo, textual_explanation: &mut Vec<String>| {
        let type_name = get_type_display_name(c, item.type_);

        let mut positions: Vec<String> = Vec::new();
        for (i, &display_node) in item.display_nodes.iter().enumerate() {
            let display_source_file = get_node_source_file(display_node);
            let mut node_text = get_node_text(display_node, display_source_file);
            node_text = node_text.replace('\n', " ");
            if node_text.len() > 50 {
                node_text = truncate_bytes(&node_text, 50);
            }
            let mut loc = format_location(display_node, sf);
            // Intentional fix over .repos reference: the .repos version unconditionally
            // prepends "at " for the first display node, producing a double preposition
            // "provided at in file.ts at ln X col Y" for cross-file references. This
            // check skips the "at " prefix when the location already starts with "in ",
            // yielding the correct "provided in file.ts at ln X col Y".
            if !loc.starts_with("in ") && i == 0 {
                loc = format!("at {loc}");
            }
            positions.push(format!("{loc} by `{node_text}`"));
        }

        textual_explanation.push(format!(
            "- {type_name} {} {}",
            item.kind.as_str(),
            positions.join(", ")
        ));
    };

    for &item in &provided_items {
        append_info(item, &mut textual_explanation);
    }
    if !provided_items.is_empty() && !required_items.is_empty() {
        textual_explanation.push(String::new());
    }
    for &item in &required_items {
        append_info(item, &mut textual_explanation);
    }

    let mut lines: Vec<String> = Vec::new();
    for l in &textual_explanation {
        lines.push(format!(" * {l}"));
    }
    "/**\n".to_string() + &lines.join("\n") + "\n */"
}
