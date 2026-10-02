package rules

import (
	"slices"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
)

var metadataGroups = []rule.MetadataGroup{
	{ID: "correctness", Name: "Correctness", Description: "Wrong, unsafe, or structurally invalid code patterns."},
	{ID: "antipattern", Name: "Anti-pattern", Description: "Discouraged patterns that often lead to bugs or confusing behavior."},
	{ID: "effectNative", Name: "Effect-native", Description: "Prefer Effect-native APIs and abstractions when available."},
	{ID: "style", Name: "Style", Description: "Cleanup, consistency, and idiomatic Effect code."},
}

func MetadataGroups() []rule.MetadataGroup {
	return slices.Clone(metadataGroups)
}

func MetadataPresets() []rule.MetadataPreset {
	return []rule.MetadataPreset{
		{
			Name:               "recommended",
			Description:        "Enable diagnostics at their default severities.",
			DiagnosticSeverity: buildDefaultPreset(),
		},
		{
			Name:               "strict",
			Description:        "Treat every diagnostic enabled by default as an error.",
			DiagnosticSeverity: buildStrictPreset(),
		},
		{
			Name:               "correctness",
			Description:        "Enable all correctness diagnostics at warning level.",
			DiagnosticSeverity: buildGroupPreset("correctness", etscore.SeverityWarning),
		},
		{
			Name:               "antipattern",
			Description:        "Enable all anti-pattern diagnostics at warning level.",
			DiagnosticSeverity: buildGroupPreset("antipattern", etscore.SeverityWarning),
		},
		{
			Name:               "effect-native",
			Description:        "Enable all Effect-native diagnostics at warning level.",
			DiagnosticSeverity: buildGroupPreset("effectNative", etscore.SeverityWarning),
		},
		{
			Name:               "style",
			Description:        "Enable all style diagnostics at warning level.",
			DiagnosticSeverity: buildGroupPreset("style", etscore.SeverityWarning),
		},
	}
}

func buildGroupPreset(group string, severity etscore.Severity) map[string]etscore.Severity {
	preset := make(map[string]etscore.Severity)
	for _, current := range All {
		if current.Group == group {
			preset[current.Name] = severity
		}
	}
	return preset
}

func buildStrictPreset() map[string]etscore.Severity {
	preset := buildDefaultPreset()
	for name := range preset {
		preset[name] = etscore.SeverityError
	}
	return preset
}

func buildDefaultPreset() map[string]etscore.Severity {
	preset := make(map[string]etscore.Severity)
	for _, current := range All {
		if !current.DefaultSeverity.IsOff() {
			preset[current.Name] = current.DefaultSeverity
		}
	}
	return preset
}
