// Package etsoxlintrunner exposes Effect diagnostics and suggestions to Oxlint.
package etsoxlintrunner

import (
	"context"
	"errors"
	"fmt"
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/fixables"
	"github.com/effect-ts/tsgo/internal/pluginoptions"
	"github.com/effect-ts/tsgo/internal/rulerunner"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/compiler"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
)

// ReportedDiagnostic is a runner-neutral diagnostic ready for an external integration.
type ReportedDiagnostic struct {
	Range              core.TextRange
	MessageID          string
	Description        string
	RelatedInformation []ReportedRelatedInformation
	Suggestions        func() []ReportedSuggestion
}

// ReportedSuggestion is an independently selectable correction for a diagnostic.
type ReportedSuggestion struct {
	Description string
	Edits       []ReportedEdit
}

// ReportedEdit replaces Range with Text.
type ReportedEdit struct {
	Range core.TextRange
	Text  string
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
	program *compiler.Program,
	c *checker.Checker,
	sf *ast.SourceFile,
	options *etscore.EffectPluginOptions,
	ruleName string,
) ([]*ast.Diagnostic, error) {
	normalized := normalizeOptions(options, ruleName)

	return rulerunner.Run(ctx, program, c, sf, &normalized, []string{ruleName}, rulerunner.MinVisibleSeverity(&normalized))
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
	program *compiler.Program,
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
	normalized := normalizeOptions(options, ruleName)
	diagnostics, err := rulerunner.Run(ctx, program, c, sf, &normalized, []string{ruleName}, rulerunner.MinVisibleSeverity(&normalized))
	if err != nil {
		return err
	}
	resolvedOptions := pluginoptions.ResolveEffectPluginOptionsForSourceFile(
		&normalized,
		sf.FileName(),
		program.Options().ConfigFilePath,
		program.UseCaseSensitiveFileNames(),
	)
	reportDiagnostics(ctx, diagnostics, ruleName, program, c, sf, resolvedOptions, adapter)
	return nil
}

func reportDiagnostics(
	ctx context.Context,
	diagnostics []*ast.Diagnostic,
	ruleName string,
	program *compiler.Program,
	c *checker.Checker,
	sf *ast.SourceFile,
	options *etscore.ResolvedEffectPluginOptions,
	adapter DiagnosticAdapter,
) {
	for _, diagnostic := range diagnostics {
		adapter.Report(ReportedDiagnostic{
			Range:              diagnostic.Loc(),
			MessageID:          fmt.Sprintf("TS%d", diagnostic.Code()),
			Description:        diagnosticDescription(diagnostic, ruleName, adapter),
			RelatedInformation: reportedRelatedInformation(diagnostic, ruleName, adapter),
			Suggestions: func() []ReportedSuggestion {
				return reportedSuggestions(ctx, program, c, sf, options, diagnostic)
			},
		})
	}
}

func reportedSuggestions(
	ctx context.Context,
	program *compiler.Program,
	c *checker.Checker,
	sf *ast.SourceFile,
	options *etscore.ResolvedEffectPluginOptions,
	diagnostic *ast.Diagnostic,
) []ReportedSuggestion {
	tp := typeparser.NewTypeParser(program, c)
	fixCtx := fixable.NewStandaloneContext(ctx, sf, diagnostic.Loc(), diagnostic.Code(), options, program, c, tp)
	var suggestions []ReportedSuggestion
	for _, provider := range fixables.ByErrorCode(diagnostic.Code()) {
		if provider.Name == "effectDisable" {
			continue
		}
		for _, action := range provider.Run(fixCtx) {
			edits := reportedEdits(fixCtx, action.Changes)
			if len(edits) == 0 {
				continue
			}
			suggestions = append(suggestions, ReportedSuggestion{
				Description: action.Description,
				Edits:       edits,
			})
		}
	}
	return suggestions
}

func reportedEdits(ctx *fixable.Context, edits []*lsproto.TextEdit) []ReportedEdit {
	reported := make([]ReportedEdit, 0, len(edits))
	for _, edit := range edits {
		if edit == nil {
			continue
		}
		reported = append(reported, ReportedEdit{
			Range: ctx.LSPRangeToTextRange(edit.Range),
			Text:  edit.NewText,
		})
	}
	return reported
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
