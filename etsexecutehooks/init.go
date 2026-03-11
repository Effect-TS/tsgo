package etsexecutehooks

import (
	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/execute/tsc"
)

func init() {
	tsc.RegisterFilterDiagnosticsForExitCodeCallback(filterDiagnosticsForExitCode)
}

// filterDiagnosticsForExitCode is the callback registered with the tsc package.
// It extracts the Effect plugin options from compiler options and delegates to
// FilterDiagnosticsForExitCode.
func filterDiagnosticsForExitCode(opts *core.CompilerOptions, diagnostics []*ast.Diagnostic) []*ast.Diagnostic {
	return FilterDiagnosticsForExitCode(opts.Effect, diagnostics)
}

// FilterDiagnosticsForExitCode filters out Effect diagnostics whose category should
// be ignored for exit-code determination, based on the exit-code config options.
// Non-Effect diagnostics are always retained. If opts is nil, the original slice is
// returned unchanged.
func FilterDiagnosticsForExitCode(opts *etscore.EffectPluginOptions, diags []*ast.Diagnostic) []*ast.Diagnostic {
	if opts == nil {
		return diags
	}

	var filtered []*ast.Diagnostic
	for _, diag := range diags {
		if rule.IsEffectCode(diag.Code()) && shouldIgnoreForExitCode(opts, diag.Category()) {
			continue
		}
		filtered = append(filtered, diag)
	}
	return filtered
}

// shouldIgnoreForExitCode returns true if the given diagnostic category should be
// excluded from exit-code determination based on the Effect plugin options.
func shouldIgnoreForExitCode(opts *etscore.EffectPluginOptions, category diagnostics.Category) bool {
	switch category {
	case diagnostics.CategorySuggestion, diagnostics.CategoryMessage:
		return opts.IgnoreEffectSuggestionsInTscExitCode
	case diagnostics.CategoryWarning:
		return opts.IgnoreEffectWarningsInTscExitCode
	case diagnostics.CategoryError:
		return opts.IgnoreEffectErrorsInTscExitCode
	default:
		return false
	}
}
