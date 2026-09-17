// Package effectconfigcheck validates resolved Effect compiler options.
package effectconfigcheck

import (
	"slices"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/directives"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
)

func Register() {
	tsoptions.RegisterValidateCompilerOptionsCallback(validate)
}

func validate(options *core.CompilerOptions, sourceFile *ast.SourceFile) []*ast.Diagnostic {
	if options == nil || !etscore.DiagnosticsEnabled(options.Effect) {
		return nil
	}
	config := options.Effect
	severity, configured := config.DiagnosticSeverity[rule.UnknownRuleNameName]
	if !configured {
		severity = etscore.SeverityWarning
	}
	if severity.IsOff() {
		return nil
	}

	// Syntax is used only to locate diagnostics; validation uses the merged options.
	plugin := effectPluginSyntax(sourceFile)
	var result []*ast.Diagnostic
	check := func(severities map[string]etscore.Severity, syntax *ast.Node) {
		var unknown []string
		for name := range severities {
			if name != rule.UnusedDirectiveName && name != rule.UnknownRuleNameName && rule.ByName(rules.All, name) == nil {
				unknown = append(unknown, name)
			}
		}
		// Maps have no iteration order; keep CLI output and baselines deterministic.
		slices.Sort(unknown)
		for _, name := range unknown {
			var node *ast.Node
			if property := findProperty(syntax, name); property != nil {
				node = property.Name()
			}
			diagnostic := tsoptions.CreateDiagnosticForNodeInSourceFileOrCompilerDiagnostic(
				sourceFile, node,
				diagnostics.Unknown_Effect_diagnostic_rule_0_in_diagnosticSeverity_effect_unknownRuleName,
				name,
			)
			diagnostic.SetCategory(directives.ToCategory(severity))
			result = append(result, diagnostic)
		}
	}
	check(config.DiagnosticSeverity, propertyValue(plugin, "diagnosticSeverity"))

	// Inherited overrides precede local overrides. Only local object entries have
	// syntax in this file, and the parser skips non-object entries.
	var localOverrides []*ast.Node
	for _, node := range arrayElements(propertyValue(plugin, "overrides")) {
		if ast.IsObjectLiteralExpression(node) {
			localOverrides = append(localOverrides, node)
		}
	}
	localStart := len(config.Overrides) - len(localOverrides)
	for i, override := range config.Overrides {
		var syntax *ast.Node
		if localStart >= 0 && i >= localStart {
			syntax = propertyValue(propertyValue(localOverrides[i-localStart], "options"), "diagnosticSeverity")
		}
		check(override.Options.DiagnosticSeverity, syntax)
	}
	return result
}

func effectPluginSyntax(sourceFile *ast.SourceFile) *ast.Node {
	compilerOptions := tsoptions.ForEachTsConfigPropArray(sourceFile, "compilerOptions", func(property *ast.PropertyAssignment) *ast.PropertyAssignment {
		return property
	})
	if compilerOptions == nil {
		return nil
	}
	for _, plugin := range arrayElements(propertyValue(compilerOptions.Initializer, "plugins")) {
		name := propertyValue(plugin, "name")
		if name != nil && ast.IsStringLiteralLike(name) && name.Text() == etscore.EffectPluginName {
			return plugin
		}
	}
	return nil
}

func findProperty(node *ast.Node, name string) *ast.PropertyAssignment {
	if node == nil || !ast.IsObjectLiteralExpression(node) {
		return nil
	}
	var result *ast.PropertyAssignment
	for _, property := range node.Properties() {
		if ast.IsPropertyAssignment(property) && ast.GetTextOfPropertyName(property.Name()) == name {
			// JSON parsing retains the last value for duplicate keys.
			result = property.AsPropertyAssignment()
		}
	}
	return result
}

func propertyValue(node *ast.Node, name string) *ast.Node {
	if property := findProperty(node, name); property != nil {
		return property.Initializer
	}
	return nil
}

func arrayElements(node *ast.Node) []*ast.Node {
	if node != nil && ast.IsArrayLiteralExpression(node) {
		return node.Elements()
	}
	return nil
}
