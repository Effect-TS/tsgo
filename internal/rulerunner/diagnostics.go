package rulerunner

import (
	"context"
	"fmt"
	"slices"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/directives"
	"github.com/effect-ts/tsgo/internal/pluginoptions"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// RuleDiagnostic pairs a diagnostic with its rule for directive processing.
type RuleDiagnostic struct {
	RuleName   string
	Rule       *rule.Rule
	Diagnostic *ast.Diagnostic
}

// MinVisibleSeverity returns the least visible severity that can surface in
// the current execution mode: in CLI (tsc) mode without includeSuggestionsInTsc
// only warnings and errors are printed, so SeverityWarning; everywhere else
// (LSP, tests) every severity surfaces, so SeverityMessage.
func MinVisibleSeverity(effectConfig *etscore.EffectPluginOptions) etscore.Severity {
	if etscore.IsCommandLineMode() && !effectConfig.GetIncludeSuggestionsInTsc() {
		return etscore.SeverityWarning
	}
	return etscore.SeverityMessage
}

// Run executes Effect diagnostics for a source file and returns diagnostics without emitting them.
// minSeverity is the least visible severity the caller can surface (see
// MinVisibleSeverity); rules whose configured severity is below it are skipped
// pre-execution unless a directive in the file references them.
func Run(ctx context.Context, program checker.Program, c *checker.Checker, sf *ast.SourceFile, effectConfig *etscore.EffectPluginOptions, ruleNames []string, minSeverity etscore.Severity) ([]*ast.Diagnostic, error) {
	if sf.IsDeclarationFile || program.IsSourceFileFromExternalLibrary(sf) {
		return nil, nil
	}

	if effectConfig == nil {
		return nil, nil
	}

	effectiveConfig := pluginoptions.ResolveEffectPluginOptionsForSourceFile(
		effectConfig,
		sf.FileName(),
		program.Options().ConfigFilePath,
		program.UseCaseSensitiveFileNames(),
	)

	resolvedSeverity := pluginoptions.ResolveDiagnosticSeverityForFile(
		effectConfig,
		sf.FileName(),
		program.Options().ConfigFilePath,
		program.UseCaseSensitiveFileNames(),
	)

	if !etscore.DiagnosticsEnabled(effectConfig) || resolvedSeverity == nil {
		return nil, nil
	}

	selectedRules, err := selectRules(ruleNames)
	if err != nil {
		return nil, err
	}
	if len(selectedRules) == 0 {
		return nil, nil
	}

	tp := typeparser.NewTypeParser(program, c)
	sourceText := sf.Text()
	effectDirectives := directives.CollectEffectDirectives(sourceText)
	directiveSet := directives.BuildDirectiveSet(effectDirectives)

	if directiveSet.IsSkipFile("*") {
		return nil, nil
	}

	allDiagnostics := collectDiagnostics(ctx, program, c, tp, sf, effectConfig, effectiveConfig, resolvedSeverity, directiveSet, selectedRules, minSeverity)
	finalDiagnostics := transformDiagnostics(allDiagnostics, sf, directiveSet, resolvedSeverity, minSeverity)
	finalDiagnostics = append(finalDiagnostics, unusedDirectiveDiagnostics(sf, effectDirectives, directiveSet, resolvedSeverity)...)

	return finalDiagnostics, nil
}

func selectRules(ruleNames []string) ([]*rule.Rule, error) {
	if ruleNames != nil && len(ruleNames) == 0 {
		return nil, nil
	}

	if ruleNames == nil {
		selected := make([]*rule.Rule, 0, len(rules.All))
		for i := range rules.All {
			selected = append(selected, &rules.All[i])
		}
		return selected, nil
	}

	selected := make([]*rule.Rule, 0, len(ruleNames))
	for _, name := range ruleNames {
		idx := slices.IndexFunc(rules.All, func(r rule.Rule) bool { return r.Name == name })
		if idx == -1 {
			return nil, fmt.Errorf("unknown Effect diagnostic rule %q", name)
		}
		selected = append(selected, &rules.All[idx])
	}
	return selected, nil
}

// collectDiagnostics runs all enabled rules and collects their diagnostics.
func collectDiagnostics(
	ctx context.Context,
	program checker.Program,
	c *checker.Checker,
	tp *typeparser.TypeParser,
	sf *ast.SourceFile,
	globalConfig *etscore.EffectPluginOptions,
	options *etscore.ResolvedEffectPluginOptions,
	resolvedSeverity map[string]etscore.Severity,
	directiveSet *directives.DirectiveSet,
	selectedRules []*rule.Rule,
	minSeverity etscore.Severity,
) []*RuleDiagnostic {
	var results []*RuleDiagnostic

	for _, r := range selectedRules {
		configSeverity, configuredExplicitly := severityFromMap(resolvedSeverity, r.Name)
		if !configuredExplicitly {
			configSeverity = r.DefaultSeverity
		}
		if !globalConfig.SkipDisabledOptimization && configSeverity.IsOff() && !directiveSet.HasEnablingDirective(r.Name) {
			continue
		}
		// A rule below the minimum visible severity can only surface if a
		// directive raises it, and it must still run when any directive
		// references it so the directive is marked used for unusedDirective
		// tracking.
		if !globalConfig.SkipDisabledOptimization &&
			!configSeverity.AtLeastAsVisibleAs(minSeverity) &&
			!directiveSet.HasAnyDirectiveForRule(r.Name) {
			continue
		}

		if directiveSet.IsSkipFile(r.Name) {
			continue
		}

		ruleCtx := rule.NewContext(ctx, program, c, tp, sf, options, r.DefaultSeverity)
		diags := r.Run(ruleCtx)

		for _, diag := range diags {
			results = append(results, &RuleDiagnostic{
				RuleName:   r.Name,
				Rule:       r,
				Diagnostic: diag,
			})
		}
	}

	return results
}

func transformDiagnostics(
	diags []*RuleDiagnostic,
	sf *ast.SourceFile,
	directiveSet *directives.DirectiveSet,
	resolvedSeverity map[string]etscore.Severity,
	minSeverity etscore.Severity,
) []*ast.Diagnostic {
	var results []*ast.Diagnostic
	lineMap := sf.ECMALineMap()

	for _, rd := range diags {
		line := scanner.ComputeLineOfPosition(lineMap, rd.Diagnostic.Pos())

		defaultSeverity, configuredExplicitly := severityFromMap(resolvedSeverity, rd.RuleName)
		if !configuredExplicitly {
			defaultSeverity = rd.Rule.DefaultSeverity
		}

		effectiveSeverity := directiveSet.GetEffectiveSeverityAndMarkUsed(
			rd.RuleName,
			line,
			defaultSeverity,
		)

		if effectiveSeverity.IsOff() {
			continue
		}
		if !effectiveSeverity.AtLeastAsVisibleAs(minSeverity) {
			continue
		}

		originalCategory := rd.Diagnostic.Category()
		newCategory := directives.ToCategory(effectiveSeverity)

		if originalCategory != newCategory {
			results = append(results, createTransformedDiagnostic(rd.Diagnostic, newCategory))
		} else {
			results = append(results, rd.Diagnostic)
		}
	}

	return results
}

func createTransformedDiagnostic(original *ast.Diagnostic, newCategory tsdiag.Category) *ast.Diagnostic {
	return ast.NewDiagnosticFromSerialized(
		original.File(),
		core.NewTextRange(original.Pos(), original.End()),
		original.Code(),
		newCategory,
		original.MessageKey(),
		original.MessageArgs(),
		original.MessageChain(),
		original.RelatedInformation(),
		original.ReportsUnnecessary(),
		original.ReportsDeprecated(),
		original.SkippedOnNoEmit(),
	)
}

func unusedDirectiveDiagnostics(sf *ast.SourceFile, allDirectives []directives.Directive, directiveSet *directives.DirectiveSet, resolvedSeverity map[string]etscore.Severity) []*ast.Diagnostic {
	severity, ok := severityFromMap(resolvedSeverity, "unusedDirective")
	if !ok {
		severity = etscore.SeverityWarning
	}
	if severity.IsOff() {
		return nil
	}

	unused := directiveSet.GetUnusedNextLineDirectives(allDirectives)
	if len(unused) == 0 {
		return nil
	}

	diags := make([]*ast.Diagnostic, 0, len(unused))
	for _, d := range unused {
		diags = append(diags, ast.NewDiagnosticFromSerialized(
			sf,
			core.NewTextRange(d.Pos, d.End),
			tsdiag.X_effect_diagnostics_directive_has_no_effect.Code(),
			directives.ToCategory(severity),
			tsdiag.X_effect_diagnostics_directive_has_no_effect.Key(),
			nil,
			nil,
			nil,
			false,
			false,
			false,
		))
	}
	return diags
}

func severityFromMap(resolved map[string]etscore.Severity, ruleName string) (etscore.Severity, bool) {
	if resolved == nil {
		return etscore.SeverityError, false
	}
	severity, ok := resolved[ruleName]
	return severity, ok
}
