package etsexecutehooks

import (
	"slices"
	"testing"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

func makeDiag(code int32, category diagnostics.Category) *ast.Diagnostic {
	return ast.NewDiagnosticFromSerialized(
		nil,
		core.NewTextRange(0, 0),
		code,
		category,
		"",
		nil,
		nil,
		nil,
		false,
		false,
		false,
	)
}

func TestFilterDiagnosticsForExitCode(t *testing.T) {
	t.Parallel()

	mixed := []*ast.Diagnostic{
		makeDiag(377009, diagnostics.CategorySuggestion),
		makeDiag(377010, diagnostics.CategoryMessage),
		makeDiag(377011, diagnostics.CategoryWarning),
		makeDiag(377001, diagnostics.CategoryError),
		makeDiag(1002, diagnostics.CategoryError),
		makeDiag(2304, diagnostics.CategoryWarning),
		makeDiag(6133, diagnostics.CategorySuggestion),
		makeDiag(9999, diagnostics.CategoryMessage),
	}
	nonEffect := []int32{1002, 2304, 6133, 9999}

	for _, tt := range []struct {
		name string
		opts *etscore.EffectPluginOptions
		want []int32
	}{
		{"nil options", nil, []int32{377009, 377010, 377011, 377001, 1002, 2304, 6133, 9999}},
		{"nothing ignored", &etscore.EffectPluginOptions{}, []int32{377009, 377010, 377011, 377001, 1002, 2304, 6133, 9999}},
		{"suggestions and messages ignored", &etscore.EffectPluginOptions{IgnoreEffectSuggestionsInTscExitCode: true}, []int32{377011, 377001, 1002, 2304, 6133, 9999}},
		{"warnings ignored", &etscore.EffectPluginOptions{IgnoreEffectWarningsInTscExitCode: true}, []int32{377009, 377010, 377001, 1002, 2304, 6133, 9999}},
		{"errors ignored", &etscore.EffectPluginOptions{IgnoreEffectErrorsInTscExitCode: true}, []int32{377009, 377010, 377011, 1002, 2304, 6133, 9999}},
		{"everything ignored", &etscore.EffectPluginOptions{
			IgnoreEffectSuggestionsInTscExitCode: true,
			IgnoreEffectWarningsInTscExitCode:    true,
			IgnoreEffectErrorsInTscExitCode:      true,
		}, nonEffect},
	} {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			var got []int32
			for _, diag := range FilterDiagnosticsForExitCode(tt.opts, mixed) {
				got = append(got, diag.Code())
			}
			if !slices.Equal(got, tt.want) {
				t.Fatalf("kept codes = %v, want %v", got, tt.want)
			}
		})
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

func TestNoEmitOnErrorHookFiltersCompilerOptions(t *testing.T) {
	t.Parallel()
	opts := &core.CompilerOptions{
		NoEmitOnError: core.BoolToTristate(true),
		Effect: &etscore.EffectPluginOptions{
			IgnoreEffectWarningsInTscExitCode: true,
		},
	}
	if result := filterDiagnosticsForExitCode(opts, []*ast.Diagnostic{makeDiag(377011, diagnostics.CategoryWarning)}); len(result) != 0 {
		t.Fatal("expected ignored Effect warning to be filtered")
	}

	if result := filterDiagnosticsForExitCode(opts, []*ast.Diagnostic{makeDiag(1002, diagnostics.CategoryError)}); len(result) != 1 {
		t.Fatal("expected TypeScript error to be retained")
	}
}
