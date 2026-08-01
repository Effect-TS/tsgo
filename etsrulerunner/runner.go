// Package etsrulerunner exposes Effect diagnostic execution to external runners.
package etsrulerunner

import (
	"context"
	"errors"
	"fmt"
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rulerunner"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
)

// ReportedDiagnostic is a runner-neutral diagnostic ready for an external integration.
type ReportedDiagnostic struct {
	Range       core.TextRange
	MessageID   string
	Description string
}

// DiagnosticAdapter provides integration-specific message localization and reporting.
type DiagnosticAdapter struct {
	Message func(*ast.Diagnostic) string
	Report  func(ReportedDiagnostic)
}

// RunRule executes one Effect rule with the supplied program checker.
func RunRule(
	ctx context.Context,
	program checker.Program,
	c *checker.Checker,
	sf *ast.SourceFile,
	options *etscore.EffectPluginOptions,
	ruleName string,
) ([]*ast.Diagnostic, error) {
	if options == nil {
		return nil, nil
	}

	normalized := *options
	normalized.Diagnostics = true
	normalized.DiagnosticSeverity = map[string]etscore.Severity{
		ruleName: etscore.SeverityError,
	}
	normalized.Overrides = append([]etscore.Override(nil), options.Overrides...)
	for i := range normalized.Overrides {
		normalized.Overrides[i].Options.DiagnosticSeverity = nil
	}

	return rulerunner.Run(ctx, program, c, sf, &normalized, []string{ruleName})
}

// RunRuleAndReport executes one Effect rule and reports converted diagnostics through adapter.
func RunRuleAndReport(
	ctx context.Context,
	program checker.Program,
	c *checker.Checker,
	sf *ast.SourceFile,
	options *etscore.EffectPluginOptions,
	ruleName string,
	adapter DiagnosticAdapter,
) error {
	if adapter.Message == nil {
		return errors.New("diagnostic adapter Message callback is required")
	}
	if adapter.Report == nil {
		return errors.New("diagnostic adapter Report callback is required")
	}
	diagnostics, err := RunRule(ctx, program, c, sf, options, ruleName)
	if err != nil {
		return err
	}
	reportDiagnostics(diagnostics, ruleName, adapter)
	return nil
}

func reportDiagnostics(diagnostics []*ast.Diagnostic, ruleName string, adapter DiagnosticAdapter) {
	for _, diagnostic := range diagnostics {
		adapter.Report(ReportedDiagnostic{
			Range:       diagnostic.Loc(),
			MessageID:   fmt.Sprintf("TS%d", diagnostic.Code()),
			Description: strings.TrimSuffix(adapter.Message(diagnostic), " effect("+ruleName+")"),
		})
	}
}
