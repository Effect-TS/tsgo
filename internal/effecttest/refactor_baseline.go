package effecttest

import (
	"fmt"
	"sort"
	"strings"
	"testing"

	"github.com/microsoft/typescript-go/shim/fourslash"
)

// RefactorInventoryEntry represents one selection range with its available refactor actions.
type RefactorInventoryEntry struct {
	// ID is the stable range identifier (e.g., "R1", "R2").
	ID string
	// RangeText is a short excerpt of the selected text.
	RangeText string
	// ActionTitles is the list of available refactor action titles.
	ActionTitles []string
	// ActionKinds is the list of code action kinds parallel to ActionTitles.
	ActionKinds []string
}

// RefactorApplicationResult represents the result of applying one refactor action.
type RefactorApplicationResult struct {
	// RangeID is the stable range identifier (e.g., "R1").
	RangeID string
	// ActionIndex is the index of the refactor action that was applied.
	ActionIndex int
	// ActionTitle is the title of the refactor action that was applied.
	ActionTitle string
	// Changes is the list of file changes from applying the refactor.
	Changes []fourslash.FileChange
}

// generateRefactorBaseline creates the content for a *.refactors.txt baseline file.
func generateRefactorBaseline(inventory []RefactorInventoryEntry, results []RefactorApplicationResult) string {
	var sb strings.Builder

	// === Inventory Section ===
	sb.WriteString("=== Refactor Inventory ===\n")

	if len(inventory) == 0 {
		sb.WriteString("(no selections)\n")
	} else {
		for _, entry := range inventory {
			fmt.Fprintf(&sb, "\n[%s] selection: %q\n", entry.ID, entry.RangeText)
			if len(entry.ActionTitles) == 0 {
				sb.WriteString("  (no refactors)\n")
			} else {
				for i, title := range entry.ActionTitles {
					fmt.Fprintf(&sb, "  Refactor %d: %q (%s)\n", i, title, entry.ActionKinds[i])
				}
			}
		}
	}

	// === Application Results Section ===
	sb.WriteString("\n=== Refactor Application Results ===\n")

	if len(results) == 0 {
		sb.WriteString("(no refactors to apply)\n")
	} else {
		for _, result := range results {
			fmt.Fprintf(&sb, "\n=== [%s] Refactor %d: %q ===\n",
				result.RangeID,
				result.ActionIndex,
				result.ActionTitle,
			)

			if len(result.Changes) == 0 {
				sb.WriteString("(no changes)\n")
			} else {
				// Sort changes by URI for deterministic output
				sortedChanges := make([]fourslash.FileChange, len(result.Changes))
				copy(sortedChanges, result.Changes)
				sort.Slice(sortedChanges, func(i, j int) bool {
					return string(sortedChanges[i].URI) < string(sortedChanges[j].URI)
				})

				for _, change := range sortedChanges {
					fmt.Fprintf(&sb, "\n--- %s ---\n", string(change.URI))

					if change.Created {
						sb.WriteString("(file created)\n")
						sb.WriteString(change.After)
						if !strings.HasSuffix(change.After, "\n") {
							sb.WriteString("\n")
						}
					} else if change.Deleted {
						sb.WriteString("(file deleted)\n")
						sb.WriteString("Before:\n")
						sb.WriteString(change.Before)
						if !strings.HasSuffix(change.Before, "\n") {
							sb.WriteString("\n")
						}
					} else {
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

// DoRefactorBaseline generates a .refactors.txt baseline for Effect refactor tests.
func DoRefactorBaseline(
	t *testing.T,
	baselineName string,
	subfolder string,
	inventory []RefactorInventoryEntry,
	results []RefactorApplicationResult,
) {
	content := generateRefactorBaseline(inventory, results)
	runEffectBaseline(t, baselineName+".refactors.txt", content, subfolder)
}
