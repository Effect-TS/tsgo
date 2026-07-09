package effecttest

import (
	"fmt"
	"os"
	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/microsoft/typescript-go/shim/fourslash"
	"github.com/microsoft/typescript-go/shim/ls/lsconv"
	"github.com/microsoft/typescript-go/shim/tspath"
)

// isDisableStyleFix returns true if the fix title matches the disable-style pattern:
// "Disable ... for this line" or "Disable ... for entire file".
func isDisableStyleFix(title string) bool {
	return strings.HasPrefix(title, "Disable ") &&
		(strings.HasSuffix(title, " for this line") || strings.HasSuffix(title, " for entire file"))
}

// RunEffectQuickFixTest executes a single Effect quick-fix baseline test case.
// It creates a fourslash test instance, collects quick-fix inventory and application
// results, and generates a *.quickfixes.txt baseline.
func RunEffectQuickFixTest(t *testing.T, version bundledeffect.EffectVersion, testFile string) {
	AcquireProgram()
	defer ReleaseProgram()

	// Read the test file
	content, err := os.ReadFile(testFile)
	if err != nil {
		t.Fatal("Failed to read test file:", err)
	}

	// Parse test file into units (handles @filename directives)
	defaultFileName := tspath.GetBaseFileName(testFile)
	units := parseTestUnits(string(content), defaultFileName)

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

	// Collect inventory and application results across all test files
	var inventory []QuickFixInventoryEntry
	var results []QuickFixApplicationResult
	diagCounter := 0

	for _, fileName := range testFileNames {
		fileURI := lsconv.FileNameToDocumentURI(fileName)

		// Get quick-fix inventory for this file
		fixes := f.GetQuickFixesForDiagnostics(t, fileURI)

		for _, fix := range fixes {
			diagCounter++
			diagID := fmt.Sprintf("D%d", diagCounter)

			inventory = append(inventory, QuickFixInventoryEntry{
				ID:         diagID,
				FileURI:    fileURI,
				Diagnostic: fix.Diagnostic,
				FixTitles:  fix.FixTitles,
			})

			// Apply each quick fix and collect results
			for fixIdx, fixTitle := range fix.FixTitles {
				if isDisableStyleFix(fixTitle) {
					results = append(results, QuickFixApplicationResult{
						DiagnosticID: diagID,
						FixIndex:     fixIdx,
						FixTitle:     fixTitle,
						Skipped:      true,
					})
				} else {
					result := f.ApplyQuickFix(t, fileURI, fix.Diagnostic, fixIdx)
					results = append(results, QuickFixApplicationResult{
						DiagnosticID: diagID,
						FixIndex:     result.FixIndex,
						FixTitle:     result.FixTitle,
						Changes:      result.Changes,
					})
				}
			}
		}
	}

	// Generate baseline
	baselineName := strings.TrimSuffix(tspath.GetBaseFileName(testFile), ".ts")
	baselineSubfolder := string(version)
	DoQuickFixBaseline(t, baselineName, baselineSubfolder, inventory, results)
}
