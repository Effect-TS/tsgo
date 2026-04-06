package effecttest

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"testing"
	"unicode/utf8"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/graph"
	"github.com/effect-ts/tsgo/internal/layergraph"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
	"github.com/microsoft/typescript-go/shim/testutil/baseline"
	"github.com/microsoft/typescript-go/shim/testutil/harnessutil"
)

// TestDataPath returns the path to our testdata directory.
func TestDataPath() string {
	return filepath.Join(bundledeffect.EffectTsGoRootPath(), "testdata")
}

// BaselineLocalPath returns the path to write local baselines.
func BaselineLocalPath(subfolder string) string {
	return filepath.Join(TestDataPath(), "baselines", "local", subfolder)
}

// BaselineReferencePath returns the path to reference baselines.
func BaselineReferencePath(subfolder string) string {
	return filepath.Join(TestDataPath(), "baselines", "reference", subfolder)
}

// DoEffectErrorBaseline generates an error baseline for Effect tests.
// This writes to our testdata directory instead of typescript-go's.
func DoEffectErrorBaseline(
	t *testing.T,
	baselineName string,
	allFiles []*harnessutil.TestFile,
	diagnostics []*ast.Diagnostic,
	_ bool,
	subfolder string,
	effectVersion string,
) {
	// Generate the baseline content
	baselineContent := generateErrorBaseline(effectVersion, allFiles, diagnostics)

	// Run baseline comparison
	runEffectBaseline(t, baselineName+".errors.txt", baselineContent, subfolder)
}

// nonWhitespace matches non-whitespace characters for preserving indentation in squiggles.
var nonWhitespace = regexp.MustCompile(`\S`)

// generateErrorBaseline creates an error baseline string matching the TypeScript harness format.
func generateErrorBaseline(effectVersion string, inputFiles []*harnessutil.TestFile, diagnostics []*ast.Diagnostic) string {
	var sb strings.Builder

	sb.WriteString("=== Metadata ===\n")
	fmt.Fprintf(&sb, "Effect version: %s\n", normalizeEffectVersionForBaseline(effectVersion))
	sb.WriteString("\n")

	// Filter out diagnostics from /node_modules/ files — these are internal
	// dependency errors (e.g. incomplete type declarations in third-party
	// packages) that are not relevant to Effect diagnostic test baselines.
	var filtered []*ast.Diagnostic
	for _, diag := range diagnostics {
		if diag.File() != nil && strings.Contains(diag.File().FileName(), "/node_modules/") {
			continue
		}
		filtered = append(filtered, diag)
	}

	// Sort diagnostics by file and position
	sortedDiags := make([]*ast.Diagnostic, len(filtered))
	copy(sortedDiags, filtered)
	sort.Slice(sortedDiags, func(i, j int) bool {
		if sortedDiags[i].File() != sortedDiags[j].File() {
			fi, fj := "", ""
			if sortedDiags[i].File() != nil {
				fi = sortedDiags[i].File().FileName()
			}
			if sortedDiags[j].File() != nil {
				fj = sortedDiags[j].File().FileName()
			}
			return fi < fj
		}
		return sortedDiags[i].Loc().Pos() < sortedDiags[j].Loc().Pos()
	})

	// Write error summary at the top
	for _, diag := range sortedDiags {
		fileName := "<global>"
		if diag.File() != nil {
			fileName = diag.File().FileName()
		}
		line := 0
		var char core.UTF16Offset
		if diag.File() != nil && diag.Loc().Pos() >= 0 {
			line, char = scanner.GetECMALineAndUTF16CharacterOfPosition(diag.File(), diag.Loc().Pos())
		}
		categoryStr := categoryToString(diag.Category())
		fmt.Fprintf(&sb, "%s(%d,%d): %s TS%d: %s\n",
			fileName, line+1, char+1, categoryStr, diag.Code(), diag.String())
	}

	sb.WriteString("\n")

	// For each input file, show the file content with error markers
	for _, file := range inputFiles {
		// Filter diagnostics for this file
		var fileErrors []*ast.Diagnostic
		for _, diag := range sortedDiags {
			if diag.File() != nil && diag.File().FileName() == file.UnitName {
				fileErrors = append(fileErrors, diag)
			}
		}

		fmt.Fprintf(&sb, "\n==== %s (%d errors) ====\n", file.UnitName, len(fileErrors))

		// Compute line starts for position calculations
		lineStarts := core.ComputeECMALineStarts(file.Content)
		lines := lineDelimiter.Split(file.Content, -1)

		for lineIndex, line := range lines {
			// Strip trailing \r if present
			if len(line) > 0 && line[len(line)-1] == '\r' {
				line = line[:len(line)-1]
			}

			thisLineStart := int(lineStarts[lineIndex])
			var nextLineStart int
			if lineIndex == len(lines)-1 {
				nextLineStart = len(file.Content)
			} else {
				nextLineStart = int(lineStarts[lineIndex+1])
			}

			// Emit this line from the original file with 4-space indentation
			sb.WriteString("    ")
			sb.WriteString(line)
			sb.WriteString("\n")

			// Check each error to see if it applies to this line
			for _, errDiag := range fileErrors {
				errStart := errDiag.Loc().Pos()
				errLen := errDiag.Loc().End() - errDiag.Loc().Pos()
				if errLen <= 0 {
					errLen = 1
				}
				end := errStart + errLen

				// Does this error start or continue on this line?
				if end >= thisLineStart && (errStart < nextLineStart || lineIndex == len(lines)-1) {
					// How many characters from the start of this line the error starts at
					relativeOffset := errStart - thisLineStart
					// How many characters of the error are on this line
					length := errLen - max(0, thisLineStart-errStart)
					// Calculate the start of the squiggle
					squiggleStart := max(0, relativeOffset)

					// Write squiggle line with indentation
					sb.WriteString("    ")
					sb.WriteString(nonWhitespace.ReplaceAllString(line[:min(squiggleStart, len(line))], " "))
					squiggleEnd := max(squiggleStart, min(squiggleStart+length, len(line)))
					if squiggleEnd > squiggleStart {
						sb.WriteString(strings.Repeat("~", utf8.RuneCountInString(line[squiggleStart:squiggleEnd])))
					}
					sb.WriteString("\n")

					// If the error ends on this line (or we're at EOF), emit the error message
					if lineIndex == len(lines)-1 || nextLineStart > end {
						fmt.Fprintf(&sb, "!!! %s TS%d: %s\n",
							categoryToString(errDiag.Category()), errDiag.Code(), errDiag.String())

						appendRelatedDiagnostics(&sb, errDiag.RelatedInformation(), 0)
					}
				}
			}
		}
	}

	return sb.String()
}

func normalizeEffectVersionForBaseline(version string) string {
	base, _, found := strings.Cut(version, "-")
	if found {
		return base
	}
	return version
}

func appendRelatedDiagnostics(sb *strings.Builder, related []*ast.Diagnostic, depth int) {
	indent := strings.Repeat("  ", depth)
	for _, info := range related {
		if info.File() != nil {
			relLine, relChar := scanner.GetECMALineAndUTF16CharacterOfPosition(info.File(), info.Loc().Pos())
			fmt.Fprintf(sb, "%s!!! related TS%d %s(%d,%d): %s\n",
				indent, info.Code(), info.File().FileName(), relLine+1, relChar+1, info.String())
		}
		appendRelatedDiagnostics(sb, info.RelatedInformation(), depth+1)
	}
}

// categoryToString converts a diagnostic category to its lowercase string representation.
func categoryToString(category tsdiag.Category) string {
	switch category {
	case tsdiag.CategoryError:
		return "error"
	case tsdiag.CategoryWarning:
		return "warning"
	case tsdiag.CategorySuggestion:
		return "suggestion"
	case tsdiag.CategoryMessage:
		return "message"
	default:
		return "error"
	}
}

// DoPipingFlowBaseline generates a .pipings.txt baseline for Effect tests.
func DoPipingFlowBaseline(
	t *testing.T,
	baselineName string,
	c *checker.Checker,
	inputFiles []*harnessutil.TestFile,
	sourceFileGetter func(string) *ast.SourceFile,
	subfolder string,
) {
	content := generatePipingFlowBaseline(c, inputFiles, sourceFileGetter)
	runEffectBaseline(t, baselineName+".pipings.txt", content, subfolder)
}

// generatePipingFlowBaseline creates a piping flow baseline string.
func generatePipingFlowBaseline(
	c *checker.Checker,
	inputFiles []*harnessutil.TestFile,
	sourceFileGetter func(string) *ast.SourceFile,
) string {
	var sb strings.Builder
	tp := typeparser.NewTypeParser(c.Program(), c)

	for _, file := range inputFiles {
		sf := sourceFileGetter(file.UnitName)
		if sf == nil {
			fmt.Fprintf(&sb, "==== %s (0 flows) ====\n", file.UnitName)
			continue
		}

		flows := tp.PipingFlows(sf, true)
		fmt.Fprintf(&sb, "==== %s (%d flows) ====\n", file.UnitName, len(flows))

		for _, flow := range flows {
			sb.WriteString("\n=== Piping Flow ===\n")

			// Location
			startLine, startChar := scanner.GetECMALineAndUTF16CharacterOfPosition(sf, flow.Node.Pos())
			endLine, endChar := scanner.GetECMALineAndUTF16CharacterOfPosition(sf, flow.Node.End())
			fmt.Fprintf(&sb, "Location: %d:%d - %d:%d\n", startLine+1, startChar+1, endLine+1, endChar+1)

			// Node text
			nodeText := scanner.GetSourceTextOfNodeFromSourceFile(sf, flow.Node, false)
			fmt.Fprintf(&sb, "Node: %s\n", escapeNewlines(nodeText))

			// Node kind
			fmt.Fprintf(&sb, "Node Kind: %s\n", flow.Node.Kind.String())
			sb.WriteString("\n")

			// Subject
			subjectText := scanner.GetSourceTextOfNodeFromSourceFile(sf, flow.Subject.Node, false)
			fmt.Fprintf(&sb, "Subject: %s\n", escapeNewlines(subjectText))
			if flow.Subject.OutType != nil {
				fmt.Fprintf(&sb, "Subject Type: %s\n", c.TypeToStringEx(flow.Subject.OutType, nil, checker.TypeFormatFlagsNoTruncation))
			} else {
				sb.WriteString("Subject Type: \n")
			}
			sb.WriteString("\n")

			// Transformations
			fmt.Fprintf(&sb, "Transformations (%d):\n", len(flow.Transformations))
			for i, t := range flow.Transformations {
				calleeText := ""
				if t.Callee != nil {
					calleeText = scanner.GetSourceTextOfNodeFromSourceFile(sf, t.Callee, false)
				}

				argsDisplay := "(constant)"
				if len(t.Args) > 0 {
					var argTexts []string
					for _, arg := range t.Args {
						argTexts = append(argTexts, escapeNewlines(scanner.GetSourceTextOfNodeFromSourceFile(sf, arg, false)))
					}
					argsDisplay = "[" + strings.Join(argTexts, ", ") + "]"
				}

				outTypeStr := ""
				if t.OutType != nil {
					outTypeStr = c.TypeToStringEx(t.OutType, nil, checker.TypeFormatFlagsNoTruncation)
				}

				fmt.Fprintf(&sb, "  [%d] kind: %s\n", i, string(t.Kind))
				fmt.Fprintf(&sb, "      callee: %s\n", escapeNewlines(calleeText))
				fmt.Fprintf(&sb, "      args: %s\n", argsDisplay)
				fmt.Fprintf(&sb, "      outType: %s\n", outTypeStr)
			}
		}
	}

	return sb.String()
}

// DoExecutionFlowBaseline generates a .flows.txt index baseline and per-source
// Mermaid baselines for Effect tests.
func DoExecutionFlowBaseline(
	t *testing.T,
	baselineName string,
	c *checker.Checker,
	inputFiles []*harnessutil.TestFile,
	sourceFileGetter func(string) *ast.SourceFile,
	subfolder string,
) {
	indexContent, mermaidFiles := generateExecutionFlowBaseline(c, baselineName, inputFiles, sourceFileGetter)
	runEffectBaseline(t, baselineName+".flows.txt", indexContent, subfolder)
	for _, mermaidFile := range mermaidFiles {
		runEffectBaseline(t, mermaidFile.fileName, mermaidFile.content, subfolder)
	}
}

type executionFlowMermaidBaseline struct {
	fileName string
	content  string
}

func generateExecutionFlowBaseline(
	c *checker.Checker,
	baselineName string,
	inputFiles []*harnessutil.TestFile,
	sourceFileGetter func(string) *ast.SourceFile,
) (string, []executionFlowMermaidBaseline) {
	var index strings.Builder
	tp := typeparser.NewTypeParser(c.Program(), c)
	var mermaidFiles []executionFlowMermaidBaseline
	usedNames := make(map[string]int)

	for _, file := range inputFiles {
		sf := sourceFileGetter(file.UnitName)
		if sf == nil {
			continue
		}

		flow := tp.ExecutionFlow(sf)
		if flow == nil {
			continue
		}
		fileName := nextExecutionFlowMermaidFileName(baselineName, sf.FileName(), usedNames)
		mermaidFiles = append(mermaidFiles, executionFlowMermaidBaseline{
			fileName: fileName,
			content: flow.ToMermaid(graph.MermaidOptions[typeparser.ExecutionNode, typeparser.ExecutionLink]{
				NodeLabel: func(node typeparser.ExecutionNode) string {
					return formatExecutionFlowNodeLabel(c, sf, node)
				},
				NodeShape: func(node typeparser.ExecutionNode) (string, string) {
					switch node.Kind {
					case typeparser.ExecutionNodeKindLogicMerge:
						return "(((", ")))"
					case typeparser.ExecutionNodeKindValue:
						return "[/", "/]"
					}
					return "[", "]"
				},
				EdgeLabel: func(edge typeparser.ExecutionLink) string {
					return formatExecutionFlowEdgeLabel(sf, edge)
				},
			}),
		})
		index.WriteString(sf.FileName())
		index.WriteString(" -> ")
		index.WriteString(fileName)
		index.WriteString("\n")
	}

	return index.String(), mermaidFiles
}

func nextExecutionFlowMermaidFileName(baselineName string, sourceFileName string, usedNames map[string]int) string {
	base := filepath.Base(sourceFileName)
	ext := filepath.Ext(base)
	base = strings.TrimSuffix(base, ext)
	if base == "" {
		base = "source"
	}

	count := usedNames[base]
	usedNames[base] = count + 1
	if count > 0 {
		base = fmt.Sprintf("%s%d", base, count)
	}

	return fmt.Sprintf("%s.flows.%s.mermaid", baselineName, base)
}

func formatExecutionFlowNodeLabel(c *checker.Checker, sf *ast.SourceFile, node typeparser.ExecutionNode) string {
	var lines []string
	switch node.Kind {
	case typeparser.ExecutionNodeKindValue:
		lines = append(lines, "type: "+formatExecutionType(c, node.Type))
		lines = append(lines, "node: "+formatExecutionSourceNode(sf, node.Node))
		return strings.Join(lines, "\n")
	case typeparser.ExecutionNodeKindLogicMerge:
		lines = append(lines, "type: "+formatExecutionType(c, node.Type))
		lines = append(lines, "node: "+formatExecutionSourceNode(sf, node.Node))
		return strings.Join(lines, "\n")
	default:
		lines = append(lines, "type: "+formatExecutionType(c, node.Type))
		lines = append(lines, "callee: "+formatExecutionSourceNode(sf, node.Callee))
	}

	args := "[]"
	if len(node.Args) > 0 {
		parts := make([]string, 0, len(node.Args))
		for _, arg := range node.Args {
			parts = append(parts, formatExecutionSourceNode(sf, arg))
		}
		args = "[" + strings.Join(parts, ", ") + "]"
	}
	lines = append(lines, "args: "+args)
	return strings.Join(lines, "\n")
}

func formatExecutionFlowEdgeLabel(_ *ast.SourceFile, edge typeparser.ExecutionLink) string {
	var lines []string
	lines = append(lines, "kind: "+string(edge.Kind))
	return strings.Join(lines, "\n")
}

func formatExecutionType(c *checker.Checker, typ *checker.Type) string {
	if typ == nil {
		return ""
	}
	return c.TypeToStringEx(typ, nil, checker.TypeFormatFlagsNoTruncation)
}

func formatExecutionSourceNode(sf *ast.SourceFile, node *ast.Node) string {
	if sf == nil || node == nil {
		return ""
	}
	return escapeNewlines(scanner.GetSourceTextOfNodeFromSourceFile(sf, node, false))
}

// escapeNewlines replaces newlines with \n for baseline display.
func escapeNewlines(s string) string {
	s = strings.ReplaceAll(s, "\r\n", "\\n")
	s = strings.ReplaceAll(s, "\n", "\\n")
	return s
}

// DoLayerGraphBaseline generates a .layers.txt baseline for Effect tests.
func DoLayerGraphBaseline(
	t *testing.T,
	baselineName string,
	c *checker.Checker,
	inputFiles []*harnessutil.TestFile,
	sourceFileGetter func(string) *ast.SourceFile,
	subfolder string,
) {
	content := generateLayerGraphBaseline(c, inputFiles, sourceFileGetter)
	runEffectBaseline(t, baselineName+".layers.txt", content, subfolder)
}

// generateLayerGraphBaseline creates a layer graph baseline string.
func generateLayerGraphBaseline(
	c *checker.Checker,
	inputFiles []*harnessutil.TestFile,
	sourceFileGetter func(string) *ast.SourceFile,
) string {
	var sb strings.Builder
	tp := typeparser.NewTypeParser(c.Program(), c)

	// Read follow depth from program-level plugin options (populated from tsconfig.json)
	followDepth := 0
	if effectConfig := c.Program().Options().Effect; effectConfig != nil {
		followDepth = effectConfig.GetLayerGraphFollowDepth()
	}

	for _, file := range inputFiles {
		sf := sourceFileGetter(file.UnitName)
		if sf == nil {
			fmt.Fprintf(&sb, "==== %s (0 layer exports) ====\n", file.UnitName)
			continue
		}

		// Collect exported const declarations with Layer-typed initializers
		type layerExport struct {
			name        string
			initializer *ast.Node
		}
		var exports []layerExport

		for _, stmt := range sf.AsSourceFile().Statements.Nodes {
			if stmt.Kind != ast.KindVariableStatement {
				continue
			}
			// Check for export modifier
			if !ast.HasSyntacticModifier(stmt, ast.ModifierFlagsExport) {
				continue
			}
			declList := stmt.AsVariableStatement().DeclarationList
			if declList == nil {
				continue
			}
			for _, decl := range declList.AsVariableDeclarationList().Declarations.Nodes {
				if decl.Kind != ast.KindVariableDeclaration {
					continue
				}
				vd := decl.AsVariableDeclaration()
				if vd.Initializer == nil {
					continue
				}
				if vd.Name().Kind != ast.KindIdentifier {
					continue
				}
				t := tp.GetTypeAtLocation(vd.Initializer)
				if !tp.IsLayerType(t, vd.Initializer) {
					continue
				}
				exports = append(exports, layerExport{
					name:        vd.Name().Text(),
					initializer: vd.Initializer,
				})
			}
		}

		fmt.Fprintf(&sb, "==== %s (%d layer exports) ====\n", file.UnitName, len(exports))

		for _, export := range exports {
			fmt.Fprintf(&sb, "\n=== %s ===\n", export.name)

			opts := layergraph.ExtractLayerGraphOptions{
				FollowSymbolsDepth:    followDepth,
				ArrayLiteralAsMerge:   false,
				ExplodeOnlyLayerCalls: false,
			}

			fullGraph := layergraph.ExtractLayerGraph(tp, c, export.initializer, sf, opts)
			outlineGraph := layergraph.ExtractOutlineGraph(c, fullGraph)
			providersAndRequirers := layergraph.ExtractProvidersAndRequirers(c, fullGraph)

			sb.WriteString("\n--- output ---\n")
			sb.WriteString(layergraph.FormatLayerGraph(c, fullGraph, sf))
			sb.WriteString("\n")

			sb.WriteString("\n--- nested ---\n")
			sb.WriteString(layergraph.FormatNestedLayerGraph(c, fullGraph, sf))
			sb.WriteString("\n")

			sb.WriteString("\n--- outline ---\n")
			sb.WriteString(layergraph.FormatOutlineGraph(c, outlineGraph, sf))
			sb.WriteString("\n")

			sb.WriteString("\n--- quickinfo ---\n")
			sb.WriteString(layergraph.FormatQuickInfo(c, providersAndRequirers, sf))
			sb.WriteString("\n")
		}
	}

	return sb.String()
}

// runEffectBaseline compares actual output with reference baseline.
// It only writes to the filesystem when the actual content differs from the
// reference (or when the reference doesn't exist yet), so that Go's test
// cache remains valid for passing tests.
func runEffectBaseline(t *testing.T, fileName string, actual string, subfolder string) {
	localDir := BaselineLocalPath(subfolder)
	referenceDir := BaselineReferencePath(subfolder)

	localPath := filepath.Join(localDir, fileName)
	referencePath := filepath.Join(referenceDir, fileName)

	// Read reference first to decide whether any writes are needed.
	referenceContent, err := os.ReadFile(referencePath)
	if err != nil {
		if os.IsNotExist(err) {
			// New baseline — write both local and reference files.
			if err := os.MkdirAll(localDir, 0o755); err != nil {
				t.Fatalf("Failed to create local baseline directory: %v", err)
			}
			if err := os.WriteFile(localPath, []byte(actual), 0o644); err != nil {
				t.Fatalf("Failed to write local baseline: %v", err)
			}
			if err := os.MkdirAll(referenceDir, 0o755); err != nil {
				t.Fatalf("Failed to create reference baseline directory: %v", err)
			}
			if err := os.WriteFile(referencePath, []byte(actual), 0o644); err != nil {
				t.Fatalf("Failed to write reference baseline: %v", err)
			}
			t.Logf("Created new baseline at %s", referencePath)
			return
		}
		t.Fatalf("Failed to read reference baseline: %v", err)
	}

	// Compare actual against reference.
	expected := string(referenceContent)
	if actual == expected {
		return
	}

	// Mismatch — write local baseline so developers can inspect the diff.
	if err := os.MkdirAll(localDir, 0o755); err != nil {
		t.Fatalf("Failed to create local baseline directory: %v", err)
	}
	if err := os.WriteFile(localPath, []byte(actual), 0o644); err != nil {
		t.Fatalf("Failed to write local baseline: %v", err)
	}

	diff := baseline.DiffText(referencePath, localPath, expected, actual)
	// Indent the diff for readability
	diffLines := strings.Split(diff, "\n")
	for i := range diffLines {
		diffLines[i] = "  " + diffLines[i]
	}
	t.Errorf("Baseline mismatch:\n%s", strings.Join(diffLines, "\n"))
}
