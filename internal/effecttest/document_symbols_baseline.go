package effecttest

import (
	"fmt"
	"strings"
	"testing"

	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

// DoDocumentSymbolsBaseline generates a .symbols.txt baseline for Effect tests.
func DoDocumentSymbolsBaseline(
	t *testing.T,
	baselineName string,
	subfolder string,
	fileSymbols []DocumentSymbolsFileResult,
) {
	content := generateDocumentSymbolsBaseline(fileSymbols)
	runEffectBaseline(t, baselineName+".symbols.txt", content, subfolder)
}

// DocumentSymbolsFileResult stores document-symbol results for one file.
type DocumentSymbolsFileResult struct {
	FileName      string
	DocumentTrees [][]*lsproto.DocumentSymbol
	FlatLists     [][]*lsproto.SymbolInformation
}

func generateDocumentSymbolsBaseline(fileSymbols []DocumentSymbolsFileResult) string {
	var sb strings.Builder

	for i, fileResult := range fileSymbols {
		fmt.Fprintf(&sb, "==== %s ====\n", fileResult.FileName)

		for i, tree := range fileResult.DocumentTrees {
			fmt.Fprintf(&sb, "\n-- hierarchical[%d] --\n", i)
			if len(tree) == 0 {
				sb.WriteString("(none)\n")
			} else {
				writeDocumentSymbolTree(&sb, tree, 0)
			}
		}

		for i, flat := range fileResult.FlatLists {
			fmt.Fprintf(&sb, "\n-- flat[%d] --\n", i)
			if len(flat) == 0 {
				sb.WriteString("(none)\n")
				continue
			}
			for _, symbol := range flat {
				container := ""
				if symbol.ContainerName != nil {
					container = *symbol.ContainerName
				}
				fmt.Fprintf(&sb, "(%s) %s container=%q range=%s\n",
					symbol.Kind.String(),
					symbol.Name,
					container,
					formatRange(symbol.Location.Range),
				)
			}
		}

		if i+1 < len(fileSymbols) {
			sb.WriteString("\n")
		}
	}

	return sb.String()
}

func writeDocumentSymbolTree(sb *strings.Builder, symbols []*lsproto.DocumentSymbol, indent int) {
	for _, symbol := range symbols {
		detailSuffix := ""
		if symbol.Detail != nil {
			detailSuffix = fmt.Sprintf(" detail=%q", *symbol.Detail)
		}
		fmt.Fprintf(sb, "%s(%s) %s%s range=%s selection=%s\n",
			strings.Repeat("  ", indent),
			symbol.Kind.String(),
			symbol.Name,
			detailSuffix,
			formatRange(symbol.Range),
			formatRange(symbol.SelectionRange),
		)
		if symbol.Children != nil {
			writeDocumentSymbolTree(sb, *symbol.Children, indent+1)
		}
	}
}

func formatRange(r lsproto.Range) string {
	return fmt.Sprintf("%d:%d-%d:%d",
		r.Start.Line+1,
		r.Start.Character+1,
		r.End.Line+1,
		r.End.Character+1,
	)
}
