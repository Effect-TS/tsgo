package effectconfigraw

import (
	"maps"
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/microsoft/typescript-go/shim/collections"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/tsoptions"
	"github.com/microsoft/typescript-go/shim/tspath"
)

// Register wires Effect-specific tsconfig merge hooks into TypeScript-Go.
func Register() {
	tsoptions.RegisterMergeCompilerOptionsCallback(MergeEffectCompilerOptions)
}

// MergeEffectCompilerOptions customizes merging for Effect plugin config.
func MergeEffectCompilerOptions(targetOptions, sourceOptions *core.CompilerOptions, rawSource any, sourceConfigPath string, basePath string) {
	if targetOptions == nil || sourceOptions == nil {
		return
	}
	sourcePluginRaw := getEffectPluginRaw(rawSource)
	if sourcePluginRaw == nil {
		return
	}
	sourceEffect := cloneEffectOptions(sourceOptions.Effect)
	if sourceEffect == nil {
		return
	}
	rewriteEffectOptionsOverrides(sourceEffect, sourceConfigPath, basePath)

	if targetOptions.Effect == nil {
		targetOptions.Effect = sourceEffect
		return
	}

	targetOptions.Effect = mergeEffectOptions(targetOptions.Effect, sourceEffect, sourcePluginRaw)
}

func mergeEffectOptions(target, source *etscore.EffectPluginOptions, sourcePluginRaw *collections.OrderedMap[string, any]) *etscore.EffectPluginOptions {
	merged := cloneEffectOptions(target)
	if merged == nil {
		return cloneEffectOptions(source)
	}
	if source == nil || sourcePluginRaw == nil {
		return merged
	}

	if sourcePluginRaw.Has("refactors") {
		merged.Refactors = source.Refactors
	}
	if sourcePluginRaw.Has("diagnostics") {
		merged.Diagnostics = source.Diagnostics
	}
	if sourcePluginRaw.Has("includeSuggestionsInTsc") {
		merged.IncludeSuggestionsInTsc = source.IncludeSuggestionsInTsc
	}
	if sourcePluginRaw.Has("quickinfo") {
		merged.Quickinfo = source.Quickinfo
	}
	if sourcePluginRaw.Has("completions") {
		merged.Completions = source.Completions
	}
	if sourcePluginRaw.Has("goto") {
		merged.Goto = source.Goto
	}
	if sourcePluginRaw.Has("renames") {
		merged.Renames = source.Renames
	}
	if sourcePluginRaw.Has("ignoreEffectSuggestionsInTscExitCode") {
		merged.IgnoreEffectSuggestionsInTscExitCode = source.IgnoreEffectSuggestionsInTscExitCode
	}
	if sourcePluginRaw.Has("ignoreEffectWarningsInTscExitCode") {
		merged.IgnoreEffectWarningsInTscExitCode = source.IgnoreEffectWarningsInTscExitCode
	}
	if sourcePluginRaw.Has("ignoreEffectErrorsInTscExitCode") {
		merged.IgnoreEffectErrorsInTscExitCode = source.IgnoreEffectErrorsInTscExitCode
	}
	if sourcePluginRaw.Has("skipDisabledOptimization") {
		merged.SkipDisabledOptimization = source.SkipDisabledOptimization
	}
	if sourcePluginRaw.Has("mermaidProvider") {
		merged.MermaidProvider = source.MermaidProvider
	}
	if sourcePluginRaw.Has("noExternal") {
		merged.NoExternal = source.NoExternal
	}
	if sourcePluginRaw.Has("layerGraphFollowDepth") {
		merged.LayerGraphFollowDepth = source.LayerGraphFollowDepth
	}
	if sourcePluginRaw.Has("inlays") {
		merged.Inlays = source.Inlays
	}
	if sourcePluginRaw.Has("namespaceImportPackages") {
		merged.NamespaceImportPackages = append([]string(nil), source.NamespaceImportPackages...)
	}
	if sourcePluginRaw.Has("barrelImportPackages") {
		merged.BarrelImportPackages = append([]string(nil), source.BarrelImportPackages...)
	}
	if sourcePluginRaw.Has("importAliases") {
		merged.ImportAliases = cloneStringMap(source.ImportAliases)
	}
	if sourcePluginRaw.Has("topLevelNamedReexports") {
		merged.TopLevelNamedReexports = source.TopLevelNamedReexports
	}
	if sourcePluginRaw.Has("keyPatterns") {
		merged.KeyPatterns = cloneKeyPatterns(source.KeyPatterns)
	}
	if sourcePluginRaw.Has("extendedKeyDetection") {
		merged.ExtendedKeyDetection = source.ExtendedKeyDetection
	}
	if sourcePluginRaw.Has("pipeableMinArgCount") {
		merged.PipeableMinArgCount = source.PipeableMinArgCount
	}
	if sourcePluginRaw.Has("allowedDuplicatedPackages") {
		merged.AllowedDuplicatedPackages = append([]string(nil), source.AllowedDuplicatedPackages...)
	}
	if sourcePluginRaw.Has("effectFn") {
		merged.EffectFn = append([]string(nil), source.EffectFn...)
	}
	if sourcePluginRaw.Has("diagnosticSeverity") {
		merged.DiagnosticSeverity = mergeSeverityMaps(merged.DiagnosticSeverity, source.DiagnosticSeverity)
	}
	if sourcePluginRaw.Has("overrides") {
		merged.Overrides = append(append([]etscore.Override(nil), merged.Overrides...), cloneOverrides(source.Overrides)...)
	}

	return merged
}

func rewriteEffectOptionsOverrides(effect *etscore.EffectPluginOptions, sourceConfigPath string, basePath string) {
	if effect == nil || len(effect.Overrides) == 0 || sourceConfigPath == "" {
		return
	}
	t := tspath.ComparePathsOptions{
		UseCaseSensitiveFileNames: true,
		CurrentDirectory:          basePath,
	}
	relativeDifference := tspath.ConvertToRelativePath(tspath.GetDirectoryPath(sourceConfigPath), t)
	if relativeDifference == "" {
		return
	}
	for i := range effect.Overrides {
		effect.Overrides[i].Include = rewriteSpecs(effect.Overrides[i].Include, relativeDifference)
		effect.Overrides[i].Exclude = rewriteSpecs(effect.Overrides[i].Exclude, relativeDifference)
	}
}

func rewriteSpecs(specs []string, relativeDifference string) []string {
	if len(specs) == 0 || relativeDifference == "" {
		return specs
	}
	rewritten := make([]string, 0, len(specs))
	for _, spec := range specs {
		if startsWithConfigDirTemplate(spec) || tspath.IsRootedDiskPath(spec) {
			rewritten = append(rewritten, spec)
			continue
		}
		rewritten = append(rewritten, tspath.CombinePaths(relativeDifference, spec))
	}
	return rewritten
}

func getEffectPluginRaw(raw any) *collections.OrderedMap[string, any] {
	compilerOptionsRaw := getCompilerOptionsRaw(raw)
	if compilerOptionsRaw == nil {
		return nil
	}
	plugins, _ := compilerOptionsRaw.GetOrZero("plugins").([]any)
	plugin, _ := findEffectPluginRaw(plugins)
	return plugin
}

func getCompilerOptionsRaw(raw any) *collections.OrderedMap[string, any] {
	if rawMap, ok := raw.(*collections.OrderedMap[string, any]); ok && rawMap != nil {
		if compilerOptionsRaw, ok := rawMap.GetOrZero("compilerOptions").(*collections.OrderedMap[string, any]); ok {
			return compilerOptionsRaw
		}
	}
	return nil
}

func findEffectPluginRaw(plugins []any) (*collections.OrderedMap[string, any], int) {
	for i, pluginValue := range plugins {
		plugin, ok := pluginValue.(*collections.OrderedMap[string, any])
		if !ok || plugin == nil {
			continue
		}
		if name, ok := plugin.GetOrZero("name").(string); ok && name == etscore.EffectPluginName {
			return plugin, i
		}
	}
	return nil, -1
}

func cloneRawJSON(value any) any {
	switch value := value.(type) {
	case *collections.OrderedMap[string, any]:
		cloned := new(collections.OrderedMap[string, any])
		for key, entryValue := range value.Entries() {
			cloned.Set(key, cloneRawJSON(entryValue))
		}
		return cloned
	case []any:
		cloned := make([]any, len(value))
		for i, entryValue := range value {
			cloned[i] = cloneRawJSON(entryValue)
		}
		return cloned
	default:
		return value
	}
}

func cloneEffectOptions(source *etscore.EffectPluginOptions) *etscore.EffectPluginOptions {
	if source == nil {
		return nil
	}
	cloned := *source
	cloned.KeyPatterns = cloneKeyPatterns(source.KeyPatterns)
	cloned.AllowedDuplicatedPackages = append([]string(nil), source.AllowedDuplicatedPackages...)
	cloned.EffectFn = append([]string(nil), source.EffectFn...)
	cloned.DiagnosticSeverity = mergeSeverityMaps(nil, source.DiagnosticSeverity)
	cloned.Overrides = cloneOverrides(source.Overrides)
	cloned.NamespaceImportPackages = append([]string(nil), source.NamespaceImportPackages...)
	cloned.BarrelImportPackages = append([]string(nil), source.BarrelImportPackages...)
	cloned.ImportAliases = cloneStringMap(source.ImportAliases)
	return &cloned
}

func cloneOverrides(source []etscore.Override) []etscore.Override {
	if len(source) == 0 {
		return nil
	}
	cloned := make([]etscore.Override, len(source))
	for i, override := range source {
		cloned[i] = etscore.Override{
			Include: append([]string(nil), override.Include...),
			Exclude: append([]string(nil), override.Exclude...),
			Options: etscore.OverrideOptions{
				DiagnosticSeverity:        mergeSeverityMaps(nil, override.Options.DiagnosticSeverity),
				PipeableMinArgCount:       cloneIntPtr(override.Options.PipeableMinArgCount),
				KeyPatterns:               cloneKeyPatternsPtr(override.Options.KeyPatterns),
				ExtendedKeyDetection:      cloneBoolPtr(override.Options.ExtendedKeyDetection),
				AllowedDuplicatedPackages: cloneStringSlicePtr(override.Options.AllowedDuplicatedPackages),
				EffectFn:                  cloneStringSlicePtr(override.Options.EffectFn),
			},
		}
	}
	return cloned
}

func cloneKeyPatterns(source []etscore.KeyPattern) []etscore.KeyPattern {
	if len(source) == 0 {
		return nil
	}
	cloned := make([]etscore.KeyPattern, len(source))
	for i, pattern := range source {
		cloned[i] = etscore.KeyPattern{
			Target:          pattern.Target,
			Pattern:         pattern.Pattern,
			SkipLeadingPath: append([]string(nil), pattern.SkipLeadingPath...),
		}
	}
	return cloned
}

func cloneKeyPatternsPtr(source *[]etscore.KeyPattern) *[]etscore.KeyPattern {
	if source == nil {
		return nil
	}
	cloned := cloneKeyPatterns(*source)
	return &cloned
}

func cloneStringMap(source map[string]string) map[string]string {
	if len(source) == 0 {
		return nil
	}
	cloned := make(map[string]string, len(source))
	maps.Copy(cloned, source)
	return cloned
}

func mergeSeverityMaps(target, source map[string]etscore.Severity) map[string]etscore.Severity {
	if source == nil {
		if target == nil {
			return nil
		}
		cloned := make(map[string]etscore.Severity, len(target))
		maps.Copy(cloned, target)
		return cloned
	}
	merged := make(map[string]etscore.Severity, len(target)+len(source))
	maps.Copy(merged, target)
	maps.Copy(merged, source)
	return merged
}

func cloneIntPtr(source *int) *int {
	if source == nil {
		return nil
	}
	v := *source
	return &v
}

func cloneBoolPtr(source *bool) *bool {
	if source == nil {
		return nil
	}
	v := *source
	return &v
}

func cloneStringSlicePtr(source *[]string) *[]string {
	if source == nil {
		return nil
	}
	v := append([]string(nil), (*source)...)
	return &v
}

const configDirTemplate = "${configDir}"

func startsWithConfigDirTemplate(value any) bool {
	str, ok := value.(string)
	if !ok {
		return false
	}
	return strings.HasPrefix(strings.ToLower(str), strings.ToLower(configDirTemplate))
}
