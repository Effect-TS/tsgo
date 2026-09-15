// Package effectconfigcheck validates the @effect/language-service plugin block of
// a tsconfig file and reports configuration diagnostics anchored on the offending
// node inside that file.
//
// Validation runs in two phases because the two things it needs become available at
// different points. The offending node exists only while the config file that
// declares it is being parsed, before any extends hop is merged; whether the
// diagnostic is enabled, and at which severity, is a property of the fully merged
// configuration. ValidatePluginsNode therefore reports every unresolved name it
// finds, and FinalizeDiagnostics drops or recategorizes them once the merge is done.
package effectconfigcheck

import (
	"slices"
	"sync"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/directives"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
)

var (
	unknownRuleNameMessage = tsdiag.Unknown_Effect_diagnostic_rule_0_in_diagnosticSeverity_This_version_of_effect_Slashtsgo_does_not_provide_it_so_this_entry_has_no_effect_effect_unknownRuleName
	didYouMeanMessage      = tsdiag.Unknown_Effect_diagnostic_rule_0_in_diagnosticSeverity_Did_you_mean_1_effect_unknownRuleName
)

// Register wires tsconfig plugin-option validation into TypeScript-Go.
func Register() {
	tsoptions.RegisterValidateEffectPluginOptionsCallback(ValidatePluginsNode)
	tsoptions.RegisterFinalizeEffectPluginDiagnosticsCallback(FinalizeDiagnostics)
}

// ConfigurableNames returns every name diagnosticSeverity accepts: the rules in the
// registry plus the diagnostics that are configurable without being rules.
var ConfigurableNames = sync.OnceValue(func() []string {
	names := make([]string, 0, len(rules.All)+len(rule.NonRuleConfigurableNames))
	for i := range rules.All {
		names = append(names, rules.All[i].Name)
	}
	names = append(names, rule.NonRuleConfigurableNames...)
	slices.Sort(names)
	return names
})

// ValidatePluginsNode reports a diagnostic for every diagnosticSeverity key in the
// @effect/language-service plugin entry that this build does not provide, covering
// both the top-level map and the map inside each overrides entry. It reports
// unconditionally: the config file being parsed does not yet know what it inherits,
// so FinalizeDiagnostics owns the decision to keep, drop or recategorize.
func ValidatePluginsNode(sourceFile *ast.SourceFile, pluginsNode *ast.Node) []*ast.Diagnostic {
	if sourceFile == nil || pluginsNode == nil {
		return nil
	}

	pluginEntry := findEffectPluginEntry(pluginsNode)
	if pluginEntry == nil {
		return nil
	}

	known := ConfigurableNames()
	var diags []*ast.Diagnostic
	diags = appendUnknownNames(diags, sourceFile, known, propertyValue(pluginEntry, "diagnosticSeverity"))
	for _, override := range arrayElements(propertyValue(pluginEntry, "overrides")) {
		overrideOptions := propertyValue(override, "options")
		diags = appendUnknownNames(diags, sourceFile, known, propertyValue(overrideOptions, "diagnosticSeverity"))
	}
	return diags
}

// FinalizeDiagnostics applies the merged configuration to the diagnostics
// ValidatePluginsNode contributed. Diagnostics from any other source pass through
// untouched. This is what makes `diagnostics: false` and
// `diagnosticSeverity.unknownRuleName` work when they are inherited through
// extends rather than declared in the file that carries the offending name.
func FinalizeDiagnostics(diags []*ast.Diagnostic, options *core.CompilerOptions) []*ast.Diagnostic {
	if !slices.ContainsFunc(diags, isUnknownRuleNameDiagnostic) {
		return diags
	}

	severity, enabled := resolveSeverity(options)
	category := directives.ToCategory(severity)

	result := make([]*ast.Diagnostic, 0, len(diags))
	for _, diag := range diags {
		if !isUnknownRuleNameDiagnostic(diag) {
			result = append(result, diag)
			continue
		}
		if !enabled {
			continue
		}
		result = append(result, withCategory(diag, category))
	}
	return result
}

// resolveSeverity reads the severity of the unknown-rule-name diagnostic from the
// merged configuration, and reports whether it should be surfaced at all.
func resolveSeverity(options *core.CompilerOptions) (etscore.Severity, bool) {
	if options == nil || options.Effect == nil || !options.Effect.Diagnostics {
		return etscore.SeverityOff, false
	}
	severity, configured := options.Effect.DiagnosticSeverity[rule.UnknownRuleNameName]
	if !configured {
		severity = etscore.SeverityWarning
	}
	return severity, !severity.IsOff()
}

func isUnknownRuleNameDiagnostic(diag *ast.Diagnostic) bool {
	if diag == nil {
		return false
	}
	code := diag.Code()
	return code == unknownRuleNameMessage.Code() || code == didYouMeanMessage.Code()
}

func withCategory(diag *ast.Diagnostic, category tsdiag.Category) *ast.Diagnostic {
	if diag.Category() == category {
		return diag
	}
	return ast.NewDiagnosticFromSerialized(
		diag.File(),
		core.NewTextRange(diag.Pos(), diag.End()),
		diag.Code(),
		category,
		diag.MessageKey(),
		diag.MessageArgs(),
		diag.MessageChain(),
		diag.RelatedInformation(),
		diag.ReportsUnnecessary(),
		diag.ReportsDeprecated(),
		diag.SkippedOnNoEmit(),
	)
}

func appendUnknownNames(
	diags []*ast.Diagnostic,
	sourceFile *ast.SourceFile,
	known []string,
	diagnosticSeverity *ast.Node,
) []*ast.Diagnostic {
	if diagnosticSeverity == nil || !ast.IsObjectLiteralExpression(diagnosticSeverity) {
		return diags
	}
	for _, property := range diagnosticSeverity.Properties() {
		if !ast.IsPropertyAssignment(property) {
			continue
		}
		name := property.Name()
		if name == nil || !ast.IsStringLiteralLike(name) {
			continue
		}
		text := ast.GetTextOfPropertyName(name)
		if text == "" || slices.Contains(known, text) {
			continue
		}
		diags = append(diags, unknownNameDiagnostic(sourceFile, name, text, known))
	}
	return diags
}

func unknownNameDiagnostic(sourceFile *ast.SourceFile, name *ast.Node, text string, known []string) *ast.Diagnostic {
	if suggestion := core.GetSpellingSuggestionForStrings(text, slices.Values(known)); suggestion != "" {
		return tsoptions.CreateDiagnosticForNodeInSourceFile(sourceFile, name, didYouMeanMessage, text, suggestion)
	}
	return tsoptions.CreateDiagnosticForNodeInSourceFile(sourceFile, name, unknownRuleNameMessage, text)
}

// findEffectPluginEntry returns the object literal in the plugins array whose name
// is the Effect language service plugin, or nil when the block is absent or is not
// written as a literal.
func findEffectPluginEntry(pluginsNode *ast.Node) *ast.Node {
	for _, entry := range arrayElements(pluginsNode) {
		name := propertyValue(entry, "name")
		if name == nil || !ast.IsStringLiteralLike(name) {
			continue
		}
		if name.Text() == etscore.EffectPluginName {
			return entry
		}
	}
	return nil
}

func arrayElements(node *ast.Node) []*ast.Node {
	if node == nil || !ast.IsArrayLiteralExpression(node) {
		return nil
	}
	return node.Elements()
}

func propertyValue(objectLiteral *ast.Node, key string) *ast.Node {
	if objectLiteral == nil || !ast.IsObjectLiteralExpression(objectLiteral) {
		return nil
	}
	for _, property := range objectLiteral.Properties() {
		if !ast.IsPropertyAssignment(property) {
			continue
		}
		name := property.Name()
		if name == nil || !ast.IsStringLiteralLike(name) {
			continue
		}
		if ast.GetTextOfPropertyName(name) == key {
			return property.Initializer()
		}
	}
	return nil
}
