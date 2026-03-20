package effecttest

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"testing"

	"github.com/microsoft/typescript-go/shim/fourslash"
	"github.com/microsoft/typescript-go/shim/ls/lsconv"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/tspath"
)

// RefactorTestCasesDir returns the path to the Effect refactor test cases directory.
func RefactorTestCasesDir(version EffectVersion) string {
	return filepath.Join(EffectTsGoRootPath(), "testdata", "tests", string(version)+"-refactors")
}

// DiscoverRefactorTestCases finds all .ts test files in the refactor test cases directory.
func DiscoverRefactorTestCases(version EffectVersion) ([]string, error) {
	dir := RefactorTestCasesDir(version)
	entries, err := os.ReadDir(dir)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, err
	}

	var cases []string
	for _, entry := range entries {
		if !entry.IsDir() && filepath.Ext(entry.Name()) == ".ts" {
			cases = append(cases, filepath.Join(dir, entry.Name()))
		}
	}
	return cases, nil
}

// refactorCommentPrefix matches lines starting with "// refactor:"
var refactorCommentPrefix = regexp.MustCompile(`^//\s*refactor:\s*`)

// rangeRegex matches a single "L:C-L:C" range token (dash-separated start and end).
var rangeRegex = regexp.MustCompile(`^(\d+):(\d+)-(\d+):(\d+)$`)

// pointRegex matches a single "L:C" point selection token (no dash).
var pointRegex = regexp.MustCompile(`^(\d+):(\d+)$`)

// refactorCommentResult holds the parsed refactor comment data.
type refactorCommentResult struct {
	found  bool
	ranges []lsproto.Range
}

// parseRefactorComment scans the test file content for a first-line
// "// refactor: L:C-L:C,L:C-L:C,..." comment (1-based lines and columns).
// Multiple ranges are separated by commas. A single "L:C" denotes a point
// selection (zero-width range where start == end). The comment is NOT stripped
// from the source content.
func parseRefactorComment(content string) refactorCommentResult {
	lines := strings.SplitN(content, "\n", 2)
	firstLine := strings.TrimSpace(lines[0])

	if !refactorCommentPrefix.MatchString(firstLine) {
		return refactorCommentResult{found: false}
	}

	// Strip the "// refactor:" prefix to get the ranges portion
	payload := refactorCommentPrefix.ReplaceAllString(firstLine, "")

	// Split on "," to get individual range/point tokens
	parts := strings.Split(payload, ",")
	var ranges []lsproto.Range
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part == "" {
			continue
		}
		if m := rangeRegex.FindStringSubmatch(part); m != nil {
			startLine, _ := strconv.Atoi(m[1])
			startCol, _ := strconv.Atoi(m[2])
			endLine, _ := strconv.Atoi(m[3])
			endCol, _ := strconv.Atoi(m[4])

			ranges = append(ranges, lsproto.Range{
				Start: lsproto.Position{Line: uint32(startLine - 1), Character: uint32(startCol - 1)},
				End:   lsproto.Position{Line: uint32(endLine - 1), Character: uint32(endCol - 1)},
			})
		} else if m := pointRegex.FindStringSubmatch(part); m != nil {
			line, _ := strconv.Atoi(m[1])
			col, _ := strconv.Atoi(m[2])

			pos := lsproto.Position{Line: uint32(line - 1), Character: uint32(col - 1)}
			ranges = append(ranges, lsproto.Range{
				Start: pos,
				End:   pos,
			})
		}
	}

	if len(ranges) == 0 {
		return refactorCommentResult{found: false}
	}

	return refactorCommentResult{found: true, ranges: ranges}
}

// RunEffectRefactorTest executes a single Effect refactor baseline test case.
// It creates a fourslash test instance, collects refactor inventory and application
// results for the selection specified by a "// refactor:" comment, and generates a *.refactors.txt baseline.
func RunEffectRefactorTest(t *testing.T, version EffectVersion, testFile string) {
	AcquireProgram()
	defer ReleaseProgram()

	// Read the test file
	content, err := os.ReadFile(testFile)
	if err != nil {
		t.Fatal("Failed to read test file:", err)
	}

	// Parse the "// refactor:" comment marker (comment is preserved in source)
	rc := parseRefactorComment(string(content))
	sourceContent := string(content)

	// Parse test file into units (handles @filename directives)
	defaultFileName := tspath.GetBaseFileName(testFile)
	units := parseTestUnits(sourceContent, defaultFileName)

	// Build fourslash content with @filename directives
	var sb strings.Builder
	currentDirectory := "/.src"

	// Check if a tsconfig is provided
	hasTsConfig := false
	for _, unit := range units {
		unitName := tspath.GetNormalizedAbsolutePath(unit.name, currentDirectory)
		if strings.HasSuffix(strings.ToLower(unitName), "tsconfig.json") || strings.HasSuffix(strings.ToLower(unitName), "jsconfig.json") {
			hasTsConfig = true
			break
		}
	}

	// Inject default tsconfig if none was provided
	if !hasTsConfig {
		sb.WriteString("// @filename: ")
		sb.WriteString(tspath.GetNormalizedAbsolutePath("tsconfig.json", currentDirectory))
		sb.WriteString("\n")
		sb.WriteString(DefaultTsConfig)
		sb.WriteString("\n")
	}

	// Collect test file paths for later use
	var testFileNames []string

	// Add all test units as fourslash files
	for _, unit := range units {
		unitName := tspath.GetNormalizedAbsolutePath(unit.name, currentDirectory)
		sb.WriteString("// @filename: ")
		sb.WriteString(unitName)
		sb.WriteString("\n")
		sb.WriteString(unit.content)
		sb.WriteString("\n")

		// Track non-config files
		if !strings.HasSuffix(strings.ToLower(unitName), "tsconfig.json") && !strings.HasSuffix(strings.ToLower(unitName), "jsconfig.json") {
			testFileNames = append(testFileNames, unitName)
		}
	}

	// Create fourslash test instance
	f, done := fourslash.NewFourslash(t, nil, sb.String())
	defer done()

	// Collect inventory and application results
	var inventory []RefactorInventoryEntry
	var results []RefactorApplicationResult
	rangeCounter := 0

	// If a "// refactor:" comment was found, test with each parsed selection range
	if rc.found && len(testFileNames) > 0 {
		fileURI := lsconv.FileNameToDocumentURI(testFileNames[0])

		for _, selRange := range rc.ranges {
			rangeCounter++
			rangeID := fmt.Sprintf("R%d", rangeCounter)

			actions := f.GetRefactorActionsForRange(t, fileURI, selRange)

			var titles []string
			var kinds []string
			for _, action := range actions {
				titles = append(titles, action.Title)
				kinds = append(kinds, action.Kind)
			}

			rangeDesc := fmt.Sprintf("%d:%d-%d:%d",
				selRange.Start.Line+1,
				selRange.Start.Character+1,
				selRange.End.Line+1,
				selRange.End.Character+1,
			)

			inventory = append(inventory, RefactorInventoryEntry{
				ID:           rangeID,
				RangeText:    rangeDesc,
				ActionTitles: titles,
				ActionKinds:  kinds,
			})

			// Apply each refactor action and collect results
			for actionIdx := range actions {
				// Reset caret to beginning of file before each action to avoid
				// stale caret positions from a previous action's apply+undo cycle.
				f.GoToBOF(t)
				result := f.ApplyRefactorAction(t, fileURI, selRange, actionIdx)
				results = append(results, RefactorApplicationResult{
					RangeID:     rangeID,
					ActionIndex: result.ActionIndex,
					ActionTitle: result.ActionTitle,
					Changes:     result.Changes,
				})
			}
		}
	}

	// Also test that an empty selection (zero-width range) produces no refactors
	if len(testFileNames) > 0 {
		fileURI := lsconv.FileNameToDocumentURI(testFileNames[0])
		emptyRange := fourslash.RangeMarker{}
		emptyActions := f.GetRefactorActionsForRange(t, fileURI, emptyRange.LSRange)

		rangeCounter++
		rangeID := fmt.Sprintf("R%d", rangeCounter)
		inventory = append(inventory, RefactorInventoryEntry{
			ID:           rangeID,
			RangeText:    "empty (0:0-0:0)",
			ActionTitles: nil,
			ActionKinds:  nil,
		})
		_ = emptyActions
	}

	// Generate baseline
	baselineName := strings.TrimSuffix(tspath.GetBaseFileName(testFile), ".ts")
	baselineSubfolder := string(version)
	DoRefactorBaseline(t, baselineName, baselineSubfolder, inventory, results)
}
