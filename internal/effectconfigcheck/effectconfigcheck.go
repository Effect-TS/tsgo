// Package effectconfigcheck validates the @effect/language-service plugin block of
// a tsconfig file and reports configuration diagnostics anchored on the offending
// node inside that file.
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
// both the top-level map and the map inside each overrides entry.
func ValidatePluginsNode(sourceFile *ast.SourceFile, pluginsNode *ast.Node, options *core.CompilerOptions) []*ast.Diagnostic {
	if sourceFile == nil || pluginsNode == nil || options == nil || options.Effect == nil {
		return nil
	}
	if !options.Effect.Diagnostics {
		return nil
	}

	severity, configured := options.Effect.DiagnosticSeverity[rule.UnknownRuleNameName]
	if !configured {
		severity = etscore.SeverityWarning
	}
	if severity.IsOff() {
		return nil
	}

	pluginEntry := findEffectPluginEntry(pluginsNode)
	if pluginEntry == nil {
		return nil
	}

	known := ConfigurableNames()
	var diags []*ast.Diagnostic
	diags = appendUnknownNames(diags, sourceFile, severity, known, propertyValue(pluginEntry, "diagnosticSeverity"))
	for _, override := range arrayElements(propertyValue(pluginEntry, "overrides")) {
		overrideOptions := propertyValue(override, "options")
		diags = appendUnknownNames(diags, sourceFile, severity, known, propertyValue(overrideOptions, "diagnosticSeverity"))
	}
	return diags
}

func appendUnknownNames(
	diags []*ast.Diagnostic,
	sourceFile *ast.SourceFile,
	severity etscore.Severity,
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
		diags = append(diags, unknownNameDiagnostic(sourceFile, name, text, severity, known))
	}
	return diags
}

func unknownNameDiagnostic(
	sourceFile *ast.SourceFile,
	name *ast.Node,
	text string,
	severity etscore.Severity,
	known []string,
) *ast.Diagnostic {
	var diagnostic *ast.Diagnostic
	if suggestion := core.GetSpellingSuggestionForStrings(text, slices.Values(known)); suggestion != "" {
		diagnostic = tsoptions.CreateDiagnosticForNodeInSourceFile(sourceFile, name, didYouMeanMessage, text, suggestion)
	} else {
		diagnostic = tsoptions.CreateDiagnosticForNodeInSourceFile(sourceFile, name, unknownRuleNameMessage, text)
	}

	category := directives.ToCategory(severity)
	if diagnostic.Category() == category {
		return diagnostic
	}
	return ast.NewDiagnosticFromSerialized(
		diagnostic.File(),
		core.NewTextRange(diagnostic.Pos(), diagnostic.End()),
		diagnostic.Code(),
		category,
		diagnostic.MessageKey(),
		diagnostic.MessageArgs(),
		diagnostic.MessageChain(),
		diagnostic.RelatedInformation(),
		diagnostic.ReportsUnnecessary(),
		diagnostic.ReportsDeprecated(),
		diagnostic.SkippedOnNoEmit(),
	)
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
