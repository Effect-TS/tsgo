package layergraph

import (
	"encoding/json"
	"fmt"
	"strings"

	"github.com/effect-ts/tsgo/internal/graph"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// mermaidEntityEncode replaces Mermaid-special characters with Mermaid HTML entity codes.
// Uses single-pass character-by-character replacement to avoid double-encoding.
func mermaidEntityEncode(s string) string {
	var b strings.Builder
	b.Grow(len(s))
	for _, r := range s {
		switch r {
		case '#':
			b.WriteString("#35;")
		case '"':
			b.WriteString("#quot;")
		case '<':
			b.WriteString("#lt;")
		case '>':
			b.WriteString("#gt;")
		case '(':
			b.WriteString("#40;")
		case ')':
			b.WriteString("#41;")
		case '{':
			b.WriteString("#123;")
		case '}':
			b.WriteString("#125;")
		default:
			b.WriteRune(r)
		}
	}
	return b.String()
}

// mermaidSafe encodes a string for use in Mermaid subgraph headers (nested format).
// It normalizes whitespace, truncates, and encodes only quotes and angle brackets.
func mermaidSafe(value string) string {
	value = strings.ReplaceAll(value, "\n", " ")
	// Collapse whitespace to single spaces.
	parts := strings.Fields(value)
	value = strings.Join(parts, " ")
	if len(value) > 50 {
		value = value[:50]
	}
	value = strings.ReplaceAll(value, `"`, "#quot;")
	value = strings.ReplaceAll(value, "<", "#lt;")
	value = strings.ReplaceAll(value, ">", "#gt;")
	value = strings.TrimSpace(value)
	return value
}

// getNodeText returns the trimmed source text of a node.
func getNodeText(node *ast.Node, sf *ast.SourceFile) string {
	if node != nil && node.Parent != nil && node.Parent.Kind == ast.KindPropertyDeclaration && node.Parent.Name() == node && node.Parent.Parent != nil && node.Parent.Parent.Kind == ast.KindClassDeclaration {
		if className := node.Parent.Parent.Name(); className != nil {
			classText := strings.TrimSpace(scanner.GetSourceTextOfNodeFromSourceFile(sf, className, false))
			propText := strings.TrimSpace(scanner.GetSourceTextOfNodeFromSourceFile(sf, node, false))
			if classText != "" && propText != "" {
				return classText + "." + propText
			}
		}
	}
	return strings.TrimSpace(scanner.GetSourceTextOfNodeFromSourceFile(sf, node, false))
}

// getNodeSourceFile walks up the parent chain to find the SourceFile ancestor.
func getNodeSourceFile(node *ast.Node) *ast.SourceFile {
	n := node
	for n != nil {
		if n.Kind == ast.KindSourceFile {
			return n.AsSourceFile()
		}
		n = n.Parent
	}
	return nil
}

// formatLocation returns a human-readable location string for a node.
// If the node is in the same source file as fromSourceFile, returns "ln {line} col {character}".
// If different, returns "in {filename} at ln {line} col {character}".
func formatLocation(node *ast.Node, fromSourceFile *ast.SourceFile) string {
	nodeSourceFile := getNodeSourceFile(node)
	if nodeSourceFile == nil {
		return ""
	}
	pos := scanner.GetTokenPosOfNode(node, nodeSourceFile, false)
	line, character := scanner.GetECMALineAndUTF16CharacterOfPosition(nodeSourceFile, pos)
	if fromSourceFile == nil || nodeSourceFile == fromSourceFile {
		return fmt.Sprintf("ln %d col %d", line+1, character)
	}
	fileName := nodeSourceFile.FileName()
	if idx := strings.LastIndex(fileName, "/"); idx >= 0 {
		fileName = fileName[idx+1:]
	}
	return fmt.Sprintf("in %s at ln %d col %d", fileName, line+1, character)
}

// getTypeDisplayName returns the string representation of a type with no truncation.
func getTypeDisplayName(c *checker.Checker, t *checker.Type) string {
	return c.TypeToStringEx(t, nil, checker.TypeFormatFlagsNoTruncation)
}

// mermaidEscapeOutputLabel applies standard Mermaid escaping for backslash, brackets, and pipe
// on top of entity-encoded text. This is used for the output format.
func mermaidEscapeOutputLabel(s string) string {
	s = strings.ReplaceAll(s, `\`, `\\`)
	s = strings.ReplaceAll(s, `[`, `\[`)
	s = strings.ReplaceAll(s, `]`, `\]`)
	s = strings.ReplaceAll(s, `|`, `\|`)
	return s
}

// FormatLayerGraph renders the full layer graph as a Mermaid flowchart TD with entity-encoded labels.
// Intentional divergence from .repos reference: this output format includes cross-file location
// annotations (e.g., "_in simple.ts at ln 6 col 19_") on nodes originating from a different source
// file. The .repos reference omits location annotations from the output (flat graph) format, only
// including them in nested, outline, and quickinfo formats. The Go version adds them here as an
// enhancement to help users locate cross-file nodes in the flat graph.
func FormatLayerGraph(
	c *checker.Checker,
	layerGraph *graph.Graph[LayerGraphNodeInfo, LayerGraphEdgeInfo],
	sf *ast.SourceFile,
) string {
	var lines []string
	lines = append(lines, "flowchart TD")

	// Nodes in index order.
	for idx, nodeInfo := range layerGraph.Nodes() {
		nodeSourceFile := getNodeSourceFile(nodeInfo.Node)
		text := getNodeText(nodeInfo.Node, nodeSourceFile)

		var provides []string
		for _, t := range nodeInfo.Provides {
			provides = append(provides, getTypeDisplayName(c, t))
		}
		var requires []string
		for _, t := range nodeInfo.Requires {
			requires = append(requires, getTypeDisplayName(c, t))
		}

		label := text + "\nprovides: " + strings.Join(provides, ", ") + "\nrequires: " + strings.Join(requires, ", ")
		if nodeSourceFile != nil && nodeSourceFile != sf {
			label += "\n_" + formatLocation(nodeInfo.Node, sf) + "_"
		}
		label = mermaidEntityEncode(label)
		label = strings.ReplaceAll(label, "\n", "<br/>")
		label = mermaidEscapeOutputLabel(label)
		lines = append(lines, fmt.Sprintf("  %d[\"%s\"]", idx, label))
	}

	// Edges in index order.
	for _, edge := range layerGraph.Edges() {
		jsonBytes, err := json.Marshal(edge.Data)
		if err != nil {
			continue
		}
		edgeLabel := mermaidEntityEncode(string(jsonBytes))
		edgeLabel = mermaidEscapeOutputLabel(edgeLabel)
		lines = append(lines, fmt.Sprintf("  %d -->|\"%s\"| %d", edge.Source, edgeLabel, edge.Target))
	}

	return strings.Join(lines, "\n")
}

// FormatNestedLayerGraph renders the layer graph as a Mermaid flowchart TB with nested subgraphs.
func FormatNestedLayerGraph(
	c *checker.Checker,
	layerGraph *graph.Graph[LayerGraphNodeInfo, LayerGraphEdgeInfo],
	sf *ast.SourceFile,
) string {
	var result []string

	// Create subgraph boxes for each node.
	for idx, nodeInfo := range layerGraph.Nodes() {
		var subgraphDefs []string

		for _, kind := range []string{"requires", "provides"} {
			var types []*checker.Type
			if kind == "requires" {
				types = nodeInfo.Requires
			} else {
				types = nodeInfo.Provides
			}

			var typeNodes []string
			for i, t := range types {
				typeName := getTypeDisplayName(c, t)
				typeNodes = append(typeNodes, fmt.Sprintf(`%d_%s_%d["%s"]`, idx, kind, i, mermaidSafe(typeName)))
			}

			if len(typeNodes) > 0 {
				kindLabel := "Requires"
				if kind == "provides" {
					kindLabel = "Provides"
				}
				subgraphDefs = append(subgraphDefs, fmt.Sprintf("subgraph %d_%s [%s]", idx, kind, kindLabel))
				for _, tn := range typeNodes {
					subgraphDefs = append(subgraphDefs, "  "+tn)
				}
				subgraphDefs = append(subgraphDefs, "end")
				subgraphDefs = append(subgraphDefs, fmt.Sprintf("style %d_%s stroke:none", idx, kind))
			}
		}

		// Wrap in inner wrap subgraph.
		var wrapped []string
		wrapped = append(wrapped, fmt.Sprintf(`subgraph %d_wrap[" "]`, idx))
		for _, line := range subgraphDefs {
			wrapped = append(wrapped, "  "+line)
		}
		wrapped = append(wrapped, "end")
		wrapped = append(wrapped, fmt.Sprintf("style %d_wrap fill:transparent", idx))
		wrapped = append(wrapped, fmt.Sprintf("style %d_wrap stroke:none", idx))

		// Get display node text and location.
		displayNode := nodeInfo.DisplayNode
		displaySourceFile := getNodeSourceFile(displayNode)
		nodeText := getNodeText(displayNode, displaySourceFile)
		location := formatLocation(displayNode, sf)

		// Outer subgraph.
		result = append(result, fmt.Sprintf("subgraph %d [\"`%s<br/><small>_%s_</small>`\"]", idx, mermaidSafe(nodeText), mermaidSafe(location)))
		for _, line := range wrapped {
			result = append(result, "  "+line)
		}
		result = append(result, "end")
		result = append(result, fmt.Sprintf("style %d fill:transparent", idx))
	}

	// Create edges between matching type nodes.
	for _, edge := range layerGraph.Edges() {
		sourceData, _ := layerGraph.GetNode(edge.Source)
		targetData, _ := layerGraph.GetNode(edge.Target)

		connected := false
		for _, kind := range []string{"requires", "provides"} {
			var sourceTypes, targetTypes []*checker.Type
			if kind == "requires" {
				sourceTypes = sourceData.Requires
				targetTypes = targetData.Requires
			} else {
				sourceTypes = sourceData.Provides
				targetTypes = targetData.Provides
			}

			for i, sourceType := range sourceTypes {
				for j, targetType := range targetTypes {
					if sourceType == targetType {
						result = append(result, fmt.Sprintf("%d_%s_%d -.-> %d_%s_%d", edge.Source, kind, i, edge.Target, kind, j))
						connected = true
						break
					}
				}
			}
		}

		if !connected {
			result = append(result, fmt.Sprintf("%d -.-x %d", edge.Source, edge.Target))
		}
	}

	if len(result) == 0 {
		return ""
	}

	var lines []string
	lines = append(lines, "flowchart TB")
	for _, line := range result {
		lines = append(lines, "  "+line)
	}
	return strings.Join(lines, "\n")
}

// FormatOutlineGraph renders the outline graph as a Mermaid flowchart TD.
func FormatOutlineGraph(
	_ *checker.Checker,
	outlineGraph *graph.Graph[LayerOutlineGraphNodeInfo, struct{}],
	sf *ast.SourceFile,
) string {
	return outlineGraph.ToMermaid(graph.MermaidOptions[LayerOutlineGraphNodeInfo, struct{}]{
		Direction: "TD",
		NodeLabel: func(nodeInfo LayerOutlineGraphNodeInfo) string {
			displayNode := nodeInfo.DisplayNode
			displaySourceFile := getNodeSourceFile(displayNode)
			nodeText := mermaidEntityEncode(getNodeText(displayNode, displaySourceFile))
			if displaySourceFile == sf {
				return nodeText
			}
			return nodeText + "\n_" + mermaidEntityEncode(formatLocation(displayNode, sf)) + "_"
		},
		EdgeLabel: func(_ struct{}) string {
			return ""
		},
	})
}

// FormatQuickInfo renders provider/requirer info as a JSDoc comment.
func FormatQuickInfo(
	c *checker.Checker,
	providersAndRequirers []ProviderRequirerInfo,
	sf *ast.SourceFile,
) string {
	if len(providersAndRequirers) == 0 {
		return ""
	}

	var providedItems, requiredItems []ProviderRequirerInfo
	for _, item := range providersAndRequirers {
		if item.Kind == ProviderRequirerKindProvided {
			providedItems = append(providedItems, item)
		} else {
			requiredItems = append(requiredItems, item)
		}
	}

	var textualExplanation []string

	appendInfo := func(item ProviderRequirerInfo) {
		typeName := getTypeDisplayName(c, item.Type)

		var positions []string
		for i, displayNode := range item.DisplayNodes {
			displaySourceFile := getNodeSourceFile(displayNode)
			nodeText := getNodeText(displayNode, displaySourceFile)
			nodeText = strings.ReplaceAll(nodeText, "\n", " ")
			if len(nodeText) > 50 {
				nodeText = nodeText[:50]
			}
			loc := formatLocation(displayNode, sf)
			// Intentional fix over .repos reference: the .repos version unconditionally
			// prepends "at " for the first display node, producing a double preposition
			// "provided at in file.ts at ln X col Y" for cross-file references. This
			// check skips the "at " prefix when the location already starts with "in ",
			// yielding the correct "provided in file.ts at ln X col Y".
			if !strings.HasPrefix(loc, "in ") {
				if i == 0 {
					loc = "at " + loc
				}
			}
			positions = append(positions, fmt.Sprintf("%s by `%s`", loc, nodeText))
		}

		textualExplanation = append(textualExplanation, fmt.Sprintf("- %s %s %s", typeName, item.Kind, strings.Join(positions, ", ")))
	}

	for _, item := range providedItems {
		appendInfo(item)
	}
	if len(providedItems) > 0 && len(requiredItems) > 0 {
		textualExplanation = append(textualExplanation, "")
	}
	for _, item := range requiredItems {
		appendInfo(item)
	}

	var lines []string
	for _, l := range textualExplanation {
		lines = append(lines, " * "+l)
	}
	return "/**\n" + strings.Join(lines, "\n") + "\n */"
}
