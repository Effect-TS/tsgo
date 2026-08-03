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
	Range              core.TextRange
	MessageID          string
	Description        string
	RelatedInformation []ReportedRelatedInformation
}

// ReportedRelatedInformation is a location and message related to a reported diagnostic.
type ReportedRelatedInformation struct {
	FileName    string
	Range       core.TextRange
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
	normalized := normalizeOptions(options, ruleName)

	return rulerunner.Run(ctx, program, c, sf, &normalized, []string{ruleName})
}

func normalizeOptions(options *etscore.EffectPluginOptions, ruleName string) etscore.EffectPluginOptions {
	if options == nil {
		options = etscore.ParseFromPlugins([]any{map[string]any{"name": etscore.EffectPluginName}})
	}

	normalized := *options
	normalized.Diagnostics = true
	normalized.DiagnosticSeverity = map[string]etscore.Severity{
		ruleName:          etscore.SeverityError,
		"unusedDirective": etscore.SeverityOff,
	}
	normalized.Overrides = append([]etscore.Override(nil), options.Overrides...)
	for i := range normalized.Overrides {
		normalized.Overrides[i].Options.DiagnosticSeverity = nil
	}
	return normalized
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
			Range:              diagnostic.Loc(),
			MessageID:          fmt.Sprintf("TS%d", diagnostic.Code()),
			Description:        diagnosticDescription(diagnostic, ruleName, adapter),
			RelatedInformation: reportedRelatedInformation(diagnostic, ruleName, adapter),
		})
	}
}

func diagnosticDescription(diagnostic *ast.Diagnostic, ruleName string, adapter DiagnosticAdapter) string {
	return strings.TrimSuffix(adapter.Message(diagnostic), " effect("+ruleName+")")
}

func reportedRelatedInformation(diagnostic *ast.Diagnostic, ruleName string, adapter DiagnosticAdapter) []ReportedRelatedInformation {
	relatedDiagnostics := diagnostic.RelatedInformation()
	if len(relatedDiagnostics) == 0 {
		return nil
	}

	relatedInformation := make([]ReportedRelatedInformation, 0, len(relatedDiagnostics))
	for _, related := range relatedDiagnostics {
		if related == nil || related.File() == nil {
			continue
		}
		relatedInformation = append(relatedInformation, ReportedRelatedInformation{
			FileName:    related.File().FileName(),
			Range:       related.Loc(),
			Description: diagnosticDescription(related, ruleName, adapter),
		})
	}
	return relatedInformation
}
