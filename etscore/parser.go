package etscore

import (
	"strings"

	"github.com/microsoft/typescript-go/shim/collections"
)

// ParseFromPlugins parses the @effect/language-service plugin config from the plugins array.
// The plugins parameter is the value of compilerOptions.plugins from tsconfig.json.
// Returns nil if the plugin is not configured or if diagnosticSeverity is explicitly set to null.
func ParseFromPlugins(value any) *EffectPluginOptions {
	plugins, ok := value.([]any)
	if !ok {
		return nil
	}

	for _, p := range plugins {
		var getPluginValue func(string) (any, bool)
		switch pluginMap := p.(type) {
		case *collections.OrderedMap[string, any]:
			getPluginValue = pluginMap.Get
		case map[string]any:
			getPluginValue = func(key string) (any, bool) {
				val, exists := pluginMap[key]
				return val, exists
			}
		default:
			continue
		}

		name, found := getPluginValue("name")
		if !found || name != EffectPluginName {
			continue
		}

		// Found our plugin, parse the config
		result := &EffectPluginOptions{
			DiagnosticSeverity:                   make(map[string]Severity), // Start with empty map (enabled)
			IncludeSuggestionsInTsc:              true,                      // default: true
			IgnoreEffectSuggestionsInTscExitCode: true,                      // default: true
			IgnoreEffectWarningsInTscExitCode:    false,                     // default: false
		}

		// Parse diagnosticSeverity
		if diag, exists := getPluginValue("diagnosticSeverity"); exists {
			if diag == nil {
				// diagnosticSeverity: null means explicitly disabled
				return nil
			}
			result.DiagnosticSeverity = parseDiagnosticSeverityMap(diag)
		}

		// Parse includeSuggestionsInTsc (default: true)
		if val, exists := getPluginValue("includeSuggestionsInTsc"); exists {
			if b, ok := val.(bool); ok {
				result.IncludeSuggestionsInTsc = b
			}
		}

		// Parse ignoreEffectSuggestionsInTscExitCode (default: true)
		if val, exists := getPluginValue("ignoreEffectSuggestionsInTscExitCode"); exists {
			if b, ok := val.(bool); ok {
				result.IgnoreEffectSuggestionsInTscExitCode = b
			}
		}

		// Parse ignoreEffectWarningsInTscExitCode (default: false)
		if val, exists := getPluginValue("ignoreEffectWarningsInTscExitCode"); exists {
			if b, ok := val.(bool); ok {
				result.IgnoreEffectWarningsInTscExitCode = b
			}
		}

		// Parse ignoreEffectErrorsInTscExitCode (default: false)
		if val, exists := getPluginValue("ignoreEffectErrorsInTscExitCode"); exists {
			if b, ok := val.(bool); ok {
				result.IgnoreEffectErrorsInTscExitCode = b
			}
		}

		// Parse skipDisabledOptimization (default: false)
		if val, exists := getPluginValue("skipDisabledOptimization"); exists {
			if b, ok := val.(bool); ok {
				result.SkipDisabledOptimization = b
			}
		}

		// Parse keyPatterns (default: nil, use GetKeyPatterns() for defaults)
		if val, exists := getPluginValue("keyPatterns"); exists {
			if arr, ok := val.([]any); ok {
				result.KeyPatterns = parseKeyPatterns(arr)
			}
		}

		// Parse extendedKeyDetection (default: false)
		if val, exists := getPluginValue("extendedKeyDetection"); exists {
			if b, ok := val.(bool); ok {
				result.ExtendedKeyDetection = b
			}
		}

		// Parse pipeableMinArgCount (default: 2, handled by GetPipeableMinArgCount)
		if val, exists := getPluginValue("pipeableMinArgCount"); exists {
			if f, ok := val.(float64); ok {
				result.PipeableMinArgCount = int(f)
			}
		}

		// Parse mermaidProvider (default: "", resolved by GetMermaidBaseURL)
		if val, exists := getPluginValue("mermaidProvider"); exists {
			if s, ok := val.(string); ok {
				result.MermaidProvider = s
			}
		}

		// Parse noExternal (default: false)
		if val, exists := getPluginValue("noExternal"); exists {
			if b, ok := val.(bool); ok {
				result.NoExternal = b
			}
		}

		// Parse layerGraphFollowDepth (default: 0, handled by GetLayerGraphFollowDepth)
		if val, exists := getPluginValue("layerGraphFollowDepth"); exists {
			if f, ok := val.(float64); ok {
				result.LayerGraphFollowDepth = int(f)
			}
		}

		// Parse inlays (default: false)
		if val, exists := getPluginValue("inlays"); exists {
			if b, ok := val.(bool); ok {
				result.Inlays = b
			}
		}

		// Parse effectFn (default: nil, GetEffectFn() returns ["span"])
		if val, exists := getPluginValue("effectFn"); exists {
			if arr, ok := val.([]any); ok {
				variants := make([]string, 0, len(arr))
				for _, item := range arr {
					if s, ok := item.(string); ok {
						variants = append(variants, s)
					}
				}
				result.EffectFn = variants
			}
		}

		// Parse allowedDuplicatedPackages (default: nil)
		if val, exists := getPluginValue("allowedDuplicatedPackages"); exists {
			if arr, ok := val.([]any); ok {
				pkgs := make([]string, 0, len(arr))
				for _, item := range arr {
					if s, ok := item.(string); ok {
						pkgs = append(pkgs, s)
					}
				}
				result.AllowedDuplicatedPackages = pkgs
			}
		}

		// Parse namespaceImportPackages (default: nil, getter resolves to empty list)
		if val, exists := getPluginValue("namespaceImportPackages"); exists {
			if pkgs, ok := parseNormalizedStringArrayStrict(val); ok {
				result.NamespaceImportPackages = pkgs
			}
		}

		// Parse barrelImportPackages (default: nil, getter resolves to empty list)
		if val, exists := getPluginValue("barrelImportPackages"); exists {
			if pkgs, ok := parseNormalizedStringArrayStrict(val); ok {
				result.BarrelImportPackages = pkgs
			}
		}

		// Parse importAliases (default: nil, getter resolves to empty map)
		if val, exists := getPluginValue("importAliases"); exists {
			if aliases, ok := parseNormalizedStringMapStrict(val); ok {
				result.ImportAliases = aliases
			}
		}

		// Parse topLevelNamedReexports (default: "", getter resolves to "ignore")
		if val, exists := getPluginValue("topLevelNamedReexports"); exists {
			if s, ok := val.(string); ok {
				normalized := strings.ToLower(s)
				if normalized == "ignore" || normalized == "follow" {
					result.TopLevelNamedReexports = TopLevelNamedReexportsMode(normalized)
				}
			}
		}

		return result
	}

	return nil
}

func parseNormalizedStringArrayStrict(value any) ([]string, bool) {
	arr, ok := value.([]any)
	if !ok {
		return nil, false
	}
	result := make([]string, 0, len(arr))
	for _, item := range arr {
		s, ok := item.(string)
		if !ok {
			return nil, false
		}
		if normalized := normalizePackageName(s); normalized != "" {
			result = append(result, normalized)
		}
	}
	return result, true
}

func parseNormalizedStringMapStrict(value any) (map[string]string, bool) {
	result := map[string]string{}
	switch aliasMap := value.(type) {
	case *collections.OrderedMap[string, any]:
		for k, v := range aliasMap.Entries() {
			s, ok := v.(string)
			if !ok {
				return nil, false
			}
			if normalized := normalizePackageName(k); normalized != "" {
				result[normalized] = s
			}
		}
	case map[string]any:
		for k, v := range aliasMap {
			s, ok := v.(string)
			if !ok {
				return nil, false
			}
			if normalized := normalizePackageName(k); normalized != "" {
				result[normalized] = s
			}
		}
	default:
		return nil, false
	}
	return result, true
}

// parseDiagnosticSeverityMap converts a plugin rule configuration map to map[string]Severity.
func parseDiagnosticSeverityMap(value any) map[string]Severity {
	result := make(map[string]Severity)
	switch m := value.(type) {
	case *collections.OrderedMap[string, any]:
		for k, v := range m.Entries() {
			if s, ok := v.(string); ok {
				result[k] = ParseSeverity(s)
			}
		}
	case map[string]any:
		for k, v := range m {
			if s, ok := v.(string); ok {
				result[k] = ParseSeverity(s)
			}
		}
	}
	return result
}

// parseKeyPatterns converts a JSON array of key pattern objects to []KeyPattern.
func parseKeyPatterns(arr []any) []KeyPattern {
	var patterns []KeyPattern
	for _, item := range arr {
		m, ok := item.(*collections.OrderedMap[string, any])
		if !ok {
			continue
		}

		kp := KeyPattern{
			Target:          "service",
			Pattern:         "default",
			SkipLeadingPath: []string{"src/"},
		}

		if v, exists := m.Get("target"); exists {
			if s, ok := v.(string); ok {
				kp.Target = s
			}
		}

		if v, exists := m.Get("pattern"); exists {
			if s, ok := v.(string); ok {
				kp.Pattern = s
			}
		}

		if v, exists := m.Get("skipLeadingPath"); exists {
			if arr, ok := v.([]any); ok {
				paths := make([]string, 0, len(arr))
				for _, p := range arr {
					if s, ok := p.(string); ok {
						paths = append(paths, s)
					}
				}
				kp.SkipLeadingPath = paths
			}
		}

		patterns = append(patterns, kp)
	}
	return patterns
}
