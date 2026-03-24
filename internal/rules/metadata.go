package rules

import (
	"slices"

	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
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
	return []rule.MetadataPreset{{
		Name:               "effect-native",
		Description:        "Enable all Effect-native diagnostics at warning level.",
		DiagnosticSeverity: buildGroupPreset("effectNative", etscore.SeverityWarning),
	}}
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
