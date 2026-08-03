package etsrulerunner

import (
	"context"
	"testing"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/diagnostics"
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

func TestReportDiagnostics(t *testing.T) {
	t.Parallel()

	diagnostic := ast.NewDiagnosticFromSerialized(
		nil,
		core.NewTextRange(3, 7),
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

	var reported []ReportedDiagnostic
	reportDiagnostics([]*ast.Diagnostic{diagnostic}, "floatingEffect", DiagnosticAdapter{
		Message: func(*ast.Diagnostic) string {
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
		reportDiagnostics([]*ast.Diagnostic{diagnostic}, "floatingEffect", DiagnosticAdapter{
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
