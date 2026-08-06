package etsoxlintrunner

import (
	"context"
	"testing"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/parser"
)

func TestRunRuleAndReportRequiresAdapterCallbacks(t *testing.T) {
	t.Parallel()

	err := RunRuleAndReport(context.Background(), nil, nil, nil, nil, "floatingEffect", DiagnosticAdapter{})
	if err == nil || err.Error() != "diagnostic adapter Message callback is required" {
		t.Fatalf("unexpected missing Message callback error: %v", err)
	}

	err = RunRuleAndReport(context.Background(), nil, nil, nil, nil, "floatingEffect", DiagnosticAdapter{
		Message: func(*ast.Diagnostic) string { return "" },
	})
	if err == nil || err.Error() != "diagnostic adapter Report callback is required" {
		t.Fatalf("unexpected missing Report callback error: %v", err)
	}
}

func TestNormalizeOptionsUsesPluginDefaults(t *testing.T) {
	t.Parallel()

	options := normalizeOptions(nil, "floatingEffect")
	if !options.Refactors || !options.Diagnostics || !options.IncludeSuggestionsInTsc ||
		!options.Quickinfo || !options.Completions || !options.Goto || !options.Renames ||
		!options.IgnoreEffectSuggestionsInTscExitCode {
		t.Fatal("expected nil options to use the name-only plugin defaults")
	}
	if options.DiagnosticSeverity["floatingEffect"] != etscore.SeverityError {
		t.Fatalf("unexpected rule severity: %s", options.DiagnosticSeverity["floatingEffect"])
	}
	if options.DiagnosticSeverity["unusedDirective"] != etscore.SeverityOff {
		t.Fatalf("unexpected unusedDirective severity: %s", options.DiagnosticSeverity["unusedDirective"])
	}
}

func TestNormalizeOptionsDoesNotMutateInput(t *testing.T) {
	t.Parallel()

	input := &etscore.EffectPluginOptions{
		PipeableMinArgCount: 4,
		DiagnosticSeverity: map[string]etscore.Severity{
			"floatingEffect":  etscore.SeverityWarning,
			"unusedDirective": etscore.SeverityWarning,
		},
	}
	options := normalizeOptions(input, "floatingEffect")

	if options.PipeableMinArgCount != 4 {
		t.Fatalf("unexpected pipeable minimum: %d", options.PipeableMinArgCount)
	}
	if options.DiagnosticSeverity["floatingEffect"] != etscore.SeverityError {
		t.Fatalf("unexpected normalized rule severity: %s", options.DiagnosticSeverity["floatingEffect"])
	}
	if options.DiagnosticSeverity["unusedDirective"] != etscore.SeverityOff {
		t.Fatalf("unexpected normalized unusedDirective severity: %s", options.DiagnosticSeverity["unusedDirective"])
	}
	if input.DiagnosticSeverity["floatingEffect"] != etscore.SeverityWarning ||
		input.DiagnosticSeverity["unusedDirective"] != etscore.SeverityWarning {
		t.Fatal("input diagnostic severities were mutated")
	}
}

func TestReportDiagnostics(t *testing.T) {
	t.Parallel()

	sourceFile := parser.ParseSourceFile(ast.SourceFileParseOptions{
		FileName: "/source.ts",
		Path:     "/source.ts",
	}, "const effect = 1", core.ScriptKindTS)
	related := ast.NewDiagnosticFromSerialized(
		sourceFile,
		core.NewTextRange(9, 15),
		377007,
		diagnostics.CategoryError,
		"",
		nil,
		nil,
		nil,
		false,
		false,
		false,
	)
	diagnostic := ast.NewDiagnosticFromSerialized(
		sourceFile,
		core.NewTextRange(3, 7),
		377001,
		diagnostics.CategoryError,
		"",
		nil,
		nil,
		[]*ast.Diagnostic{related},
		false,
		false,
		false,
	)

	var reported []ReportedDiagnostic
	reportDiagnostics(context.Background(), []*ast.Diagnostic{diagnostic}, "floatingEffect", nil, nil, sourceFile, nil, DiagnosticAdapter{
		Message: func(diagnostic *ast.Diagnostic) string {
			if diagnostic == related {
				return "Inside this Effect generator. effect(floatingEffect)"
			}
			return "Effect values must be yielded or assigned effect(floatingEffect)"
		},
		Report: func(diagnostic ReportedDiagnostic) {
			reported = append(reported, diagnostic)
		},
	})

	if len(reported) != 1 {
		t.Fatalf("expected one reported diagnostic, got %d", len(reported))
	}
	if reported[0].Range != core.NewTextRange(3, 7) {
		t.Fatalf("unexpected range: %v", reported[0].Range)
	}
	if reported[0].MessageID != "TS377001" {
		t.Fatalf("unexpected message ID: %q", reported[0].MessageID)
	}
	if reported[0].Description != "Effect values must be yielded or assigned" {
		t.Fatalf("unexpected description: %q", reported[0].Description)
	}
	if reported[0].Suggestions == nil {
		t.Fatal("expected lazy suggestions callback")
	}
	if len(reported[0].RelatedInformation) != 1 {
		t.Fatalf("expected one related diagnostic, got %d", len(reported[0].RelatedInformation))
	}
	reportedRelated := reported[0].RelatedInformation[0]
	if reportedRelated.FileName != "/source.ts" {
		t.Fatalf("unexpected related filename: %q", reportedRelated.FileName)
	}
	if reportedRelated.Range != core.NewTextRange(9, 15) {
		t.Fatalf("unexpected related range: %v", reportedRelated.Range)
	}
	if reportedRelated.Description != "Inside this Effect generator." {
		t.Fatalf("unexpected related description: %q", reportedRelated.Description)
	}
}

func TestReportDiagnosticsPreservesNonSuffixRuleText(t *testing.T) {
	t.Parallel()

	diagnostic := ast.NewDiagnosticFromSerialized(
		nil,
		core.NewTextRange(0, 0),
		377001,
		diagnostics.CategoryError,
		"",
		nil,
		nil,
		nil,
		false,
		false,
		false,
	)

	for _, message := range []string{
		"effect(floatingEffect) should remain when it is not a suffix",
		"message effect(otherRule)",
		"message effect(floatingEffect) ",
	} {
		var description string
		reportDiagnostics(context.Background(), []*ast.Diagnostic{diagnostic}, "floatingEffect", nil, nil, nil, nil, DiagnosticAdapter{
			Message: func(*ast.Diagnostic) string { return message },
			Report: func(diagnostic ReportedDiagnostic) {
				description = diagnostic.Description
			},
		})

		if description != message {
			t.Fatalf("unexpected description: %q", description)
		}
	}
}
