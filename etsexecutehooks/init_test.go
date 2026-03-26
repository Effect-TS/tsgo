package etsexecutehooks

import (
	"testing"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/diagnostics"
)

// makeDiag creates a minimal diagnostic with the given code and category.
func makeDiag(code int32, category diagnostics.Category) *ast.Diagnostic {
	return ast.NewDiagnosticFromSerialized(
		nil,                     // file
		core.NewTextRange(0, 0), // loc
		code,                    // code
		category,                // category
		"",                      // messageKey
		nil,                     // messageArgs
		nil,                     // messageChain
		nil,                     // relatedInformation
		false,                   // reportsUnnecessary
		false,                   // reportsDeprecated
		false,                   // skippedOnNoEmit
	)
}

func TestFilterDiagnosticsForExitCode_NilOptions(t *testing.T) {
	t.Parallel()
	diags := []*ast.Diagnostic{
		makeDiag(377009, diagnostics.CategorySuggestion),
		makeDiag(377001, diagnostics.CategoryError),
		makeDiag(1002, diagnostics.CategoryError),
	}
	result := FilterDiagnosticsForExitCode(nil, diags)
	if len(result) != len(diags) {
		t.Errorf("expected %d diagnostics, got %d", len(diags), len(result))
	}
	// Verify it's the same slice (no filtering)
	for i := range diags {
		if result[i] != diags[i] {
			t.Errorf("expected same diagnostic at index %d", i)
		}
	}
}

func TestFilterDiagnosticsForExitCode_DefaultConfig(t *testing.T) {
	t.Parallel()
	// Default: suggestions ignored, warnings not ignored
	opts := &etscore.EffectPluginOptions{
		IgnoreEffectSuggestionsInTscExitCode: true,
		IgnoreEffectWarningsInTscExitCode:    false,
	}

	diags := []*ast.Diagnostic{
		makeDiag(377009, diagnostics.CategorySuggestion), // Effect suggestion — should be filtered
		makeDiag(377011, diagnostics.CategoryWarning),    // Effect warning — should be kept
		makeDiag(377001, diagnostics.CategoryError),      // Effect error — should be kept
		makeDiag(1002, diagnostics.CategoryError),        // Non-Effect error — should be kept
		makeDiag(2304, diagnostics.CategorySuggestion),   // Non-Effect suggestion — should be kept
	}

	result := FilterDiagnosticsForExitCode(opts, diags)

	if len(result) != 4 {
		t.Fatalf("expected 4 diagnostics, got %d", len(result))
	}
	// Verify the correct diagnostics were kept
	if result[0].Code() != 377011 {
		t.Errorf("expected code 377011, got %d", result[0].Code())
	}
	if result[1].Code() != 377001 {
		t.Errorf("expected code 377001, got %d", result[1].Code())
	}
	if result[2].Code() != 1002 {
		t.Errorf("expected code 1002, got %d", result[2].Code())
	}
	if result[3].Code() != 2304 {
		t.Errorf("expected code 2304, got %d", result[3].Code())
	}
}

func TestFilterDiagnosticsForExitCode_IgnoreWarnings(t *testing.T) {
	t.Parallel()
	opts := &etscore.EffectPluginOptions{
		IgnoreEffectSuggestionsInTscExitCode: true,
		IgnoreEffectWarningsInTscExitCode:    true,
	}

	diags := []*ast.Diagnostic{
		makeDiag(377009, diagnostics.CategorySuggestion), // Effect suggestion — filtered
		makeDiag(377011, diagnostics.CategoryWarning),    // Effect warning — filtered
		makeDiag(377001, diagnostics.CategoryError),      // Effect error — kept
		makeDiag(1002, diagnostics.CategoryError),        // Non-Effect error — kept
		makeDiag(2304, diagnostics.CategoryWarning),      // Non-Effect warning — kept
	}

	result := FilterDiagnosticsForExitCode(opts, diags)

	if len(result) != 3 {
		t.Fatalf("expected 3 diagnostics, got %d", len(result))
	}
	if result[0].Code() != 377001 {
		t.Errorf("expected code 377001, got %d", result[0].Code())
	}
	if result[1].Code() != 1002 {
		t.Errorf("expected code 1002, got %d", result[1].Code())
	}
	if result[2].Code() != 2304 {
		t.Errorf("expected code 2304, got %d", result[2].Code())
	}
}

func TestFilterDiagnosticsForExitCode_ErrorsNotFilteredByDefault(t *testing.T) {
	t.Parallel()
	// With suggestion+warning ignore set but error ignore NOT set, errors are kept
	opts := &etscore.EffectPluginOptions{
		IgnoreEffectSuggestionsInTscExitCode: true,
		IgnoreEffectWarningsInTscExitCode:    true,
	}

	diags := []*ast.Diagnostic{
		makeDiag(377001, diagnostics.CategoryError), // Effect error — kept
		makeDiag(377003, diagnostics.CategoryError), // Effect error — kept
		makeDiag(377004, diagnostics.CategoryError), // Effect error — kept
	}

	result := FilterDiagnosticsForExitCode(opts, diags)

	if len(result) != 3 {
		t.Fatalf("expected 3 diagnostics, got %d", len(result))
	}
	for i, diag := range result {
		if diag != diags[i] {
			t.Errorf("expected same diagnostic at index %d", i)
		}
	}
}

func TestFilterDiagnosticsForExitCode_IgnoreErrors(t *testing.T) {
	t.Parallel()
	// When IgnoreEffectErrorsInTscExitCode is true, Effect errors are filtered
	opts := &etscore.EffectPluginOptions{
		IgnoreEffectErrorsInTscExitCode: true,
	}

	diags := []*ast.Diagnostic{
		makeDiag(377001, diagnostics.CategoryError), // Effect error — filtered
		makeDiag(377003, diagnostics.CategoryError), // Effect error — filtered
		makeDiag(1002, diagnostics.CategoryError),   // Non-Effect error — kept
	}

	result := FilterDiagnosticsForExitCode(opts, diags)

	if len(result) != 1 {
		t.Fatalf("expected 1 diagnostic, got %d", len(result))
	}
	if result[0].Code() != 1002 {
		t.Errorf("expected code 1002, got %d", result[0].Code())
	}
}

func TestFilterDiagnosticsForExitCode_IgnoreErrorsFalse(t *testing.T) {
	t.Parallel()
	// When IgnoreEffectErrorsInTscExitCode is false (default), Effect errors are kept
	opts := &etscore.EffectPluginOptions{
		IgnoreEffectErrorsInTscExitCode: false,
	}

	diags := []*ast.Diagnostic{
		makeDiag(377001, diagnostics.CategoryError), // Effect error — kept
		makeDiag(377003, diagnostics.CategoryError), // Effect error — kept
		makeDiag(1002, diagnostics.CategoryError),   // Non-Effect error — kept
	}

	result := FilterDiagnosticsForExitCode(opts, diags)

	if len(result) != 3 {
		t.Fatalf("expected 3 diagnostics, got %d", len(result))
	}
}

func TestFilterDiagnosticsForExitCode_NonEffectErrorsNeverFiltered(t *testing.T) {
	t.Parallel()
	// Non-Effect errors are never filtered regardless of option
	opts := &etscore.EffectPluginOptions{
		IgnoreEffectErrorsInTscExitCode:      true,
		IgnoreEffectSuggestionsInTscExitCode: true,
		IgnoreEffectWarningsInTscExitCode:    true,
	}

	diags := []*ast.Diagnostic{
		makeDiag(1002, diagnostics.CategoryError),      // Non-Effect error — kept
		makeDiag(2304, diagnostics.CategoryWarning),    // Non-Effect warning — kept
		makeDiag(6133, diagnostics.CategorySuggestion), // Non-Effect suggestion — kept
	}

	result := FilterDiagnosticsForExitCode(opts, diags)

	if len(result) != 3 {
		t.Fatalf("expected 3 diagnostics, got %d", len(result))
	}
	for i, diag := range result {
		if diag != diags[i] {
			t.Errorf("expected same diagnostic at index %d", i)
		}
	}
}

func TestFilterDiagnosticsForExitCode_NonEffectNeverFiltered(t *testing.T) {
	t.Parallel()
	// Non-Effect diagnostics of any category are never filtered
	opts := &etscore.EffectPluginOptions{
		IgnoreEffectSuggestionsInTscExitCode: true,
		IgnoreEffectWarningsInTscExitCode:    true,
	}

	diags := []*ast.Diagnostic{
		makeDiag(1002, diagnostics.CategoryError),
		makeDiag(2304, diagnostics.CategoryWarning),
		makeDiag(6133, diagnostics.CategorySuggestion),
		makeDiag(9999, diagnostics.CategoryMessage),
	}

	result := FilterDiagnosticsForExitCode(opts, diags)

	if len(result) != 4 {
		t.Fatalf("expected 4 diagnostics, got %d", len(result))
	}
	for i, diag := range result {
		if diag != diags[i] {
			t.Errorf("expected same diagnostic at index %d", i)
		}
	}
}

func TestFilterDiagnosticsForExitCode_BothFalse(t *testing.T) {
	t.Parallel()
	// When both ignore options are false, nothing is filtered
	opts := &etscore.EffectPluginOptions{
		IgnoreEffectSuggestionsInTscExitCode: false,
		IgnoreEffectWarningsInTscExitCode:    false,
	}

	diags := []*ast.Diagnostic{
		makeDiag(377009, diagnostics.CategorySuggestion),
		makeDiag(377011, diagnostics.CategoryWarning),
		makeDiag(377001, diagnostics.CategoryError),
	}

	result := FilterDiagnosticsForExitCode(opts, diags)

	if len(result) != 3 {
		t.Fatalf("expected 3 diagnostics, got %d", len(result))
	}
}

func TestFilterDiagnosticsForExitCode_EmptySlice(t *testing.T) {
	t.Parallel()
	opts := &etscore.EffectPluginOptions{
		IgnoreEffectSuggestionsInTscExitCode: true,
	}

	result := FilterDiagnosticsForExitCode(opts, nil)
	if result != nil {
		t.Errorf("expected nil for nil input, got %v", result)
	}

	result = FilterDiagnosticsForExitCode(opts, []*ast.Diagnostic{})
	if len(result) != 0 {
		t.Errorf("expected empty slice, got %d diagnostics", len(result))
	}
}

func TestFilterDiagnosticsForExitCode_MessageCategory(t *testing.T) {
	t.Parallel()
	// Message category follows the same config as Suggestion
	opts := &etscore.EffectPluginOptions{
		IgnoreEffectSuggestionsInTscExitCode: true,
	}

	diags := []*ast.Diagnostic{
		makeDiag(377009, diagnostics.CategoryMessage), // Effect message — filtered (same config as suggestion)
	}

	result := FilterDiagnosticsForExitCode(opts, diags)

	if len(result) != 0 {
		t.Fatalf("expected 0 diagnostics, got %d", len(result))
	}
}
