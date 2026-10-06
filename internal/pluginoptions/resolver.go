package pluginoptions

import (
	"maps"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfsmatch"
)

// ResolveEffectPluginOptionsForSourceFile resolves ordered path-scoped diagnostic overrides for a file.
func ResolveEffectPluginOptionsForSourceFile(config *etscore.EffectPluginOptions, fileName tspath.RootedFilePath, configFilePath tspath.RootedFilePath, useCaseSensitiveFileNames bool) *etscore.ResolvedEffectPluginOptions {
	if config == nil {
		return nil
	}
	if len(config.Overrides) == 0 {
		return cloneOptions(config)
	}

	basePath := tspath.GetDirectoryPath(string(configFilePath))
	var effective *etscore.ResolvedEffectPluginOptions

	for _, override := range config.Overrides {
		if !matchesOverride(override, string(fileName), basePath, useCaseSensitiveFileNames) {
			continue
		}
		if effective == nil {
			effective = cloneOptions(config)
		}
		applyOverride(effective, override)
	}

	if effective == nil {
		return cloneOptions(config)
	}
	return effective
}

// ResolveDiagnosticSeverityForFile resolves ordered diagnostic severity overrides for a file.
func ResolveDiagnosticSeverityForFile(config *etscore.EffectPluginOptions, fileName tspath.RootedFilePath, configFilePath tspath.RootedFilePath, useCaseSensitiveFileNames bool) map[string]etscore.Severity {
	if config == nil {
		return nil
	}
	if len(config.Overrides) == 0 {
		return config.DiagnosticSeverity
	}

	basePath := tspath.GetDirectoryPath(string(configFilePath))
	resolved := cloneDiagnosticSeverity(config.DiagnosticSeverity)
	if resolved == nil {
		resolved = map[string]etscore.Severity{}
	}

	for _, override := range config.Overrides {
		if !matchesOverride(override, string(fileName), basePath, useCaseSensitiveFileNames) {
			continue
		}
		if len(override.Options.DiagnosticSeverity) == 0 {
			continue
		}
		maps.Copy(resolved, override.Options.DiagnosticSeverity)
	}

	return resolved
}

func matchesOverride(override etscore.Override, fileName string, basePath string, useCaseSensitiveFileNames bool) bool {
	sensitivity := tspath.CaseInsensitive
	if useCaseSensitiveFileNames {
		sensitivity = tspath.CaseSensitive
	}
	rootedBasePath := tspath.RootedDirectoryPathFromAbsolute(basePath)
	if len(override.Include) > 0 {
		matcher := vfsmatch.NewSpecMatcher(override.Include, rootedBasePath, vfsmatch.UsageFiles, sensitivity)
		if matcher == nil || !matcher.MatchString(fileName) {
			return false
		}
	}

	if len(override.Exclude) > 0 {
		matcher := vfsmatch.NewSpecMatcher(override.Exclude, rootedBasePath, vfsmatch.UsageExclude, sensitivity)
		if matcher != nil && matcher.MatchString(fileName) {
			return false
		}
	}

	return true
}

func applyOverride(target *etscore.ResolvedEffectPluginOptions, override etscore.Override) {
	if target == nil {
		return
	}

	if override.Options.PipeableMinArgCount != nil {
		target.PipeableMinArgCount = *override.Options.PipeableMinArgCount
	}
	if override.Options.KeyPatterns != nil {
		target.KeyPatterns = cloneKeyPatterns(*override.Options.KeyPatterns)
	}
	if override.Options.ExtendedKeyDetection != nil {
		target.ExtendedKeyDetection = *override.Options.ExtendedKeyDetection
	}
	if override.Options.AllowedUnstableApis != nil {
		target.AllowedUnstableApis = append([]string(nil), (*override.Options.AllowedUnstableApis)...)
	}
	if override.Options.AllowedExperimentalApis != nil {
		target.AllowedExperimentalApis = append([]string(nil), (*override.Options.AllowedExperimentalApis)...)
	}
	if override.Options.AllowedDuplicatedPackages != nil {
		target.AllowedDuplicatedPackages = append([]string(nil), (*override.Options.AllowedDuplicatedPackages)...)
	}
	if override.Options.EffectFn != nil {
		target.EffectFn = append([]string(nil), (*override.Options.EffectFn)...)
	}
}

func cloneOptions(config *etscore.EffectPluginOptions) *etscore.ResolvedEffectPluginOptions {
	if config == nil {
		return nil
	}

	cloned := etscore.ResolvedEffectPluginOptions{
		NamespaceImportPackages:   append([]string(nil), config.NamespaceImportPackages...),
		BarrelImportPackages:      append([]string(nil), config.BarrelImportPackages...),
		ImportAliases:             cloneStringMap(config.ImportAliases),
		TopLevelNamedReexports:    config.TopLevelNamedReexports,
		KeyPatterns:               cloneKeyPatterns(config.KeyPatterns),
		ExtendedKeyDetection:      config.ExtendedKeyDetection,
		PipeableMinArgCount:       config.PipeableMinArgCount,
		AllowedUnstableApis:       append([]string(nil), config.AllowedUnstableApis...),
		AllowedExperimentalApis:   append([]string(nil), config.AllowedExperimentalApis...),
		AllowedDuplicatedPackages: append([]string(nil), config.AllowedDuplicatedPackages...),
		EffectFn:                  append([]string(nil), config.EffectFn...),
	}

	return &cloned
}

func cloneDiagnosticSeverity(input map[string]etscore.Severity) map[string]etscore.Severity {
	if input == nil {
		return nil
	}
	cloned := make(map[string]etscore.Severity, len(input))
	maps.Copy(cloned, input)
	return cloned
}

func cloneStringMap(input map[string]string) map[string]string {
	if input == nil {
		return nil
	}
	cloned := make(map[string]string, len(input))
	maps.Copy(cloned, input)
	return cloned
}

func cloneKeyPatterns(patterns []etscore.KeyPattern) []etscore.KeyPattern {
	if len(patterns) == 0 {
		return nil
	}
	cloned := make([]etscore.KeyPattern, len(patterns))
	for i, pattern := range patterns {
		cloned[i] = etscore.KeyPattern{
			Target:          pattern.Target,
			Pattern:         pattern.Pattern,
			SkipLeadingPath: append([]string(nil), pattern.SkipLeadingPath...),
		}
	}
	return cloned
}
