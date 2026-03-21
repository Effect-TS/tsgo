package effecttest

import (
	"fmt"
	"sort"
	"strconv"
	"strings"
	"testing"

	"github.com/microsoft/typescript-go/shim/fourslash"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

// QuickFixInventoryEntry represents one diagnostic occurrence with its available quick fixes.
type QuickFixInventoryEntry struct {
	// ID is the stable diagnostic identifier (e.g., "D1", "D2").
	ID string
	// FileURI is the document URI of the file containing the diagnostic.
	FileURI lsproto.DocumentUri
	// Diagnostic is the LSP diagnostic.
	Diagnostic *lsproto.Diagnostic
	// FixTitles is the list of available quick-fix titles.
	FixTitles []string
}

// QuickFixApplicationResult represents the result of applying one quick fix.
type QuickFixApplicationResult struct {
	// DiagnosticID is the stable diagnostic identifier (e.g., "D1").
	DiagnosticID string
	// FixIndex is the index of the quick fix that was applied.
	FixIndex int
	// FixTitle is the title of the quick fix that was applied.
	FixTitle string
	// Changes is the list of file changes from applying the quick fix.
	Changes []fourslash.FileChange
	// Skipped indicates the fix was intentionally not applied (e.g., disable-style fixes).
	Skipped bool
}

// diagnosticCodeString formats the diagnostic code for display.
func diagnosticCodeString(d *lsproto.Diagnostic) string {
	if d.Code == nil {
		return "?"
	}
	if d.Code.Integer != nil {
		return strconv.Itoa(int(*d.Code.Integer))
	}
	if d.Code.String != nil {
		return *d.Code.String
	}
	return "?"
}

// generateQuickFixBaseline creates the content for a *.quickfixes.txt baseline file.
func generateQuickFixBaseline(inventory []QuickFixInventoryEntry, results []QuickFixApplicationResult) string {
	var sb strings.Builder

	// === Inventory Section ===
	sb.WriteString("=== Quick Fix Inventory ===\n")

	if len(inventory) == 0 {
		sb.WriteString("(no diagnostics)\n")
	} else {
		for _, entry := range inventory {
			fmt.Fprintf(&sb, "\n[%s] (%d:%d-%d:%d) TS%s: %s\n",
				entry.ID,
				entry.Diagnostic.Range.Start.Line+1,
				entry.Diagnostic.Range.Start.Character+1,
				entry.Diagnostic.Range.End.Line+1,
				entry.Diagnostic.Range.End.Character+1,
				diagnosticCodeString(entry.Diagnostic),
				entry.Diagnostic.Message,
			)
			if len(entry.FixTitles) == 0 {
				sb.WriteString("  (no quick fixes)\n")
			} else {
				for i, title := range entry.FixTitles {
					fmt.Fprintf(&sb, "  Fix %d: %q\n", i, title)
				}
			}
		}
	}

	// === Application Results Section ===
	sb.WriteString("\n=== Quick Fix Application Results ===\n")

	if len(results) == 0 {
		sb.WriteString("(no quick fixes to apply)\n")
	} else {
		for _, result := range results {
			fmt.Fprintf(&sb, "\n=== [%s] Fix %d: %q ===\n",
				result.DiagnosticID,
				result.FixIndex,
				result.FixTitle,
			)

			switch {
			case result.Skipped:
				sb.WriteString("skipped by default\n")
			case len(result.Changes) == 0:
				sb.WriteString("(no changes)\n")
			default:
				// Sort changes by URI for deterministic output
				sortedChanges := make([]fourslash.FileChange, len(result.Changes))
				copy(sortedChanges, result.Changes)
				sort.Slice(sortedChanges, func(i, j int) bool {
					return string(sortedChanges[i].URI) < string(sortedChanges[j].URI)
				})

				for _, change := range sortedChanges {
					fmt.Fprintf(&sb, "\n--- %s ---\n", string(change.URI))

					switch {
					case change.Created:
						sb.WriteString("(file created)\n")
						sb.WriteString(change.After)
						if !strings.HasSuffix(change.After, "\n") {
							sb.WriteString("\n")
						}
					case change.Deleted:
						sb.WriteString("(file deleted)\n")
						sb.WriteString("Before:\n")
						sb.WriteString(change.Before)
						if !strings.HasSuffix(change.Before, "\n") {
							sb.WriteString("\n")
						}
					default:
						sb.WriteString(change.After)
						if !strings.HasSuffix(change.After, "\n") {
							sb.WriteString("\n")
						}
					}
				}
			}
		}
	}

	return sb.String()
}

// DoQuickFixBaseline generates a .quickfixes.txt baseline for Effect tests.
func DoQuickFixBaseline(
	t *testing.T,
	baselineName string,
	subfolder string,
	inventory []QuickFixInventoryEntry,
	results []QuickFixApplicationResult,
) {
	content := generateQuickFixBaseline(inventory, results)
	runEffectBaseline(t, baselineName+".quickfixes.txt", content, subfolder)
}
