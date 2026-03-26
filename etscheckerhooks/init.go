// Package etscheckerhooks provides Effect diagnostics integration with TypeScript-Go.
// This package registers hooks into the checker to run Effect-specific diagnostics
// after each source file is type checked.
package etscheckerhooks

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/directives"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// init registers the Effect diagnostics callbacks with TypeScript-Go.
func init() {
	// Set the version suffix so that core.Version() includes the Effect version
	core.SetVersionSuffix("+effect-tsgo." + etscore.EffectVersion)
	// Register the after check source file callback
	checker.RegisterAfterCheckSourceFileCallback(afterCheckSourceFile)
}

// getEffectConfig retrieves the Effect plugin configuration from the program's compiler options.
// Returns nil if no Effect config is present.
func getEffectConfig(p checker.Program) *etscore.EffectPluginOptions {
	return p.Options().Effect
}

// RuleDiagnostic pairs a diagnostic with its rule for directive processing.
type RuleDiagnostic struct {
	RuleName   string
	Rule       *rule.Rule
	Diagnostic *ast.Diagnostic
}

// afterCheckSourceFile is called after type checking each source file.
// It runs Effect diagnostics if the plugin is enabled.
func afterCheckSourceFile(c *checker.Checker, sf *ast.SourceFile) {
	// Get Effect config from program options (parsed during config loading)
	effectConfig := getEffectConfig(c.Program())
	if effectConfig == nil {
		return
	}

	// Check if diagnostics are enabled (nil DiagnosticSeverity map means explicitly disabled)
	if !effectConfig.IsEnabled() {
		return
	}

	// Skip declaration files
	if sf.IsDeclarationFile {
		return
	}

	// Collect directives from source file for suppression support
	sourceText := sf.Text()
	effectDirectives := directives.CollectEffectDirectives(sourceText)
	directiveSet := directives.BuildDirectiveSet(effectDirectives)

	// Check for file-level skip-file directive for all rules
	if directiveSet.IsSkipFile("*") {
		return
	}

	// Collect all diagnostics from enabled rules
	allDiagnostics := collectDiagnostics(c, sf, effectConfig, directiveSet)

	// Transform and filter diagnostics based on directives
	finalDiagnostics := transformDiagnostics(allDiagnostics, sf, directiveSet, effectConfig)

	// Emit final diagnostics
	for _, diag := range finalDiagnostics {
		c.AddDiagnostic(diag)
	}

	// Report unused next-line directives
	reportUnusedDirectives(c, sf, effectDirectives, directiveSet, effectConfig)
}

// collectDiagnostics runs all enabled rules and collects their diagnostics.
func collectDiagnostics(
	c *checker.Checker,
	sf *ast.SourceFile,
	config *etscore.EffectPluginOptions,
	directiveSet *directives.DirectiveSet,
) []*RuleDiagnostic {
	var results []*RuleDiagnostic

	for i := range rules.All {
		r := &rules.All[i]
		// Determine effective severity: use explicit config if set, otherwise rule's default
		configSeverity, configuredExplicitly := config.GetSeverityOk(r.Name)
		if !configuredExplicitly {
			configSeverity = r.DefaultSeverity
		}
		// Skip rules that are off, unless a directive in the source file enables them
		// or skipDisabledOptimization is set (which bypasses this optimization entirely)
		if !config.SkipDisabledOptimization && configSeverity.IsOff() && !directiveSet.HasEnablingDirective(r.Name) {
			continue
		}

		// Skip rules with file-level skip-file directive
		if directiveSet.IsSkipFile(r.Name) {
			continue
		}

		// Run the rule
		ctx := rule.NewContext(c, sf, r.DefaultSeverity)
		diags := r.Run(ctx)

		// Tag each diagnostic with its rule for directive lookup
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

// transformDiagnostics applies directive transformations to diagnostics.
// Returns a new slice of diagnostics with potentially modified severities.
// Diagnostics may be filtered out if their effective severity is "off".
func transformDiagnostics(
	diags []*RuleDiagnostic,
	sf *ast.SourceFile,
	directiveSet *directives.DirectiveSet,
	config *etscore.EffectPluginOptions,
) []*ast.Diagnostic {
	var results []*ast.Diagnostic
	lineMap := sf.ECMALineMap()

	for _, rd := range diags {
		// Get diagnostic line number
		line := scanner.ComputeLineOfPosition(lineMap, rd.Diagnostic.Pos())

		// Get default severity: use explicit config if set, otherwise rule's default
		defaultSeverity, configuredExplicitly := config.GetSeverityOk(rd.RuleName)
		if !configuredExplicitly {
			defaultSeverity = rd.Rule.DefaultSeverity
		}

		// Get effective severity considering directives
		effectiveSeverity := directiveSet.GetEffectiveSeverityAndMarkUsed(
			rd.RuleName,
			line,
			defaultSeverity,
		)

		// Skip if severity is off
		if effectiveSeverity.IsOff() {
			continue
		}

		// Transform diagnostic if severity changed
		originalCategory := rd.Diagnostic.Category()
		newCategory := directives.ToCategory(effectiveSeverity)

		// In CLI mode, filter or convert suggestion/message diagnostics
		if etscore.IsCommandLineMode() {
			if !config.GetIncludeSuggestionsInTsc() {
				// Drop suggestion and message diagnostics entirely
				if newCategory == tsdiag.CategorySuggestion || newCategory == tsdiag.CategoryMessage {
					continue
				}
			}
		}

		if originalCategory != newCategory {
			transformed := createTransformedDiagnostic(rd.Diagnostic, newCategory)
			results = append(results, transformed)
		} else {
			// No change needed
			results = append(results, rd.Diagnostic)
		}
	}

	return results
}

// createTransformedDiagnostic creates a new diagnostic with a different category.
// This is the immutable approach - we don't mutate the original.
func createTransformedDiagnostic(
	original *ast.Diagnostic,
	newCategory tsdiag.Category,
) *ast.Diagnostic {
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

// reportUnusedDirectives emits warnings for next-line directives that didn't suppress any diagnostic.
func reportUnusedDirectives(
	c *checker.Checker,
	sf *ast.SourceFile,
	allDirectives []directives.Directive,
	directiveSet *directives.DirectiveSet,
	config *etscore.EffectPluginOptions,
) {
	// Get configured severity, defaulting to warning (per spec)
	severity, ok := config.GetSeverityOk("unusedDirective")
	if !ok {
		severity = etscore.SeverityWarning // default for unusedDirective
	}
	if severity.IsOff() {
		return
	}

	unused := directiveSet.GetUnusedNextLineDirectives(allDirectives)
	if len(unused) == 0 {
		return
	}

	for _, d := range unused {
		diag := ast.NewDiagnosticFromSerialized(
			sf,
			core.NewTextRange(d.Pos, d.End),
			tsdiag.X_effect_diagnostics_directive_has_no_effect.Code(),
			directives.ToCategory(severity),
			tsdiag.X_effect_diagnostics_directive_has_no_effect.Key(),
			nil,   // messageArgs
			nil,   // messageChain
			nil,   // relatedInformation
			false, // reportsUnnecessary
			false, // reportsDeprecated
			false, // skippedOnNoEmit
		)
		c.AddDiagnostic(diag)
	}
}
