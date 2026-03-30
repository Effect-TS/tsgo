package etscore_test

import (
	"reflect"
	"testing"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/pluginoptions"
	"github.com/microsoft/typescript-go/shim/collections"
)

// makePluginMap creates an *OrderedMap for testing with the given key-value pairs.
func makePluginMap(pairs ...any) *collections.OrderedMap[string, any] {
	m := new(collections.OrderedMap[string, any])
	for i := 0; i < len(pairs); i += 2 {
		m.Set(pairs[i].(string), pairs[i+1])
	}
	return m
}

// makePlugins wraps plugin maps into the []any structure expected by ParseFromPlugins.
func makePlugins(maps ...*collections.OrderedMap[string, any]) []any {
	result := make([]any, len(maps))
	for i, m := range maps {
		result[i] = m
	}
	return result
}

func TestParseFromPlugins_ExitCodeDefaults(t *testing.T) {
	// Plugin with just the name — no exit-code keys present
	plugins := makePlugins(makePluginMap("name", etscore.EffectPluginName))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if !opts.IgnoreEffectSuggestionsInTscExitCode {
		t.Error("expected IgnoreEffectSuggestionsInTscExitCode to default to true")
	}
	if opts.IgnoreEffectWarningsInTscExitCode {
		t.Error("expected IgnoreEffectWarningsInTscExitCode to default to false")
	}
}

func TestParseFromPlugins_Overrides(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", etscore.EffectPluginName,
		"overrides", []any{
			makePluginMap(
				"include", []any{"src/**/*.ts"},
				"exclude", []any{"src/**/*.test.ts"},
				"options", makePluginMap(
					"diagnosticSeverity", makePluginMap(
						"deterministicKeys", "warning",
					),
					"pipeableMinArgCount", float64(3),
					"extendedKeyDetection", true,
					"allowedDuplicatedPackages", []any{"effect"},
				),
			),
		},
	))

	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if len(opts.Overrides) != 1 {
		t.Fatalf("Overrides length = %d, want 1", len(opts.Overrides))
	}
	override := opts.Overrides[0]
	if got, want := override.Include, []string{"src/**/*.ts"}; !reflect.DeepEqual(got, want) {
		t.Errorf("Include = %v, want %v", got, want)
	}
	if got, want := override.Exclude, []string{"src/**/*.test.ts"}; !reflect.DeepEqual(got, want) {
		t.Errorf("Exclude = %v, want %v", got, want)
	}
	if got := override.Options.DiagnosticSeverity["deterministicKeys"]; got != etscore.SeverityWarning {
		t.Errorf("Diagnostics[deterministicKeys] = %v, want %v", got, etscore.SeverityWarning)
	}
	if override.Options.PipeableMinArgCount == nil || *override.Options.PipeableMinArgCount != 3 {
		t.Fatalf("PipeableMinArgCount = %v, want 3", override.Options.PipeableMinArgCount)
	}
	if override.Options.ExtendedKeyDetection == nil || !*override.Options.ExtendedKeyDetection {
		t.Fatalf("ExtendedKeyDetection = %v, want true", override.Options.ExtendedKeyDetection)
	}
	if override.Options.AllowedDuplicatedPackages == nil || !reflect.DeepEqual(*override.Options.AllowedDuplicatedPackages, []string{"effect"}) {
		t.Fatalf("AllowedDuplicatedPackages = %v, want [effect]", override.Options.AllowedDuplicatedPackages)
	}
}

func TestResolveForFile_AppliesMatchingScopesInOrder(t *testing.T) {
	opts := &etscore.EffectPluginOptions{
		Diagnostics:         true,
		DiagnosticSeverity:  map[string]etscore.Severity{"deterministicKeys": etscore.SeverityError},
		PipeableMinArgCount: 2,
		Overrides: []etscore.Override{
			{
				Include: []string{"src/**/*.ts"},
				Options: etscore.OverrideOptions{
					DiagnosticSeverity:  map[string]etscore.Severity{"deterministicKeys": etscore.SeverityWarning},
					PipeableMinArgCount: intPtr(3),
				},
			},
			{
				Include: []string{"src/legacy/**/*.ts"},
				Options: etscore.OverrideOptions{
					DiagnosticSeverity: map[string]etscore.Severity{"deterministicKeys": etscore.SeverityOff},
				},
			},
		},
	}

	resolvedSeverity := pluginoptions.ResolveDiagnosticSeverityForFile(opts, "/repo/src/legacy/example.ts", "/repo/tsconfig.json", true)
	effective := pluginoptions.ResolveEffectPluginOptionsForSourceFile(opts, "/repo/src/legacy/example.ts", "/repo/tsconfig.json", true)
	if got := resolvedSeverity["deterministicKeys"]; got != etscore.SeverityOff {
		t.Errorf("deterministicKeys severity = %v, want %v", got, etscore.SeverityOff)
	}
	if got := effective.GetPipeableMinArgCount(); got != 3 {
		t.Errorf("PipeableMinArgCount = %d, want 3", got)
	}
}

func TestResolveForFile_ExcludeOnlyAffectsCurrentScope(t *testing.T) {
	opts := &etscore.EffectPluginOptions{
		Diagnostics:        true,
		DiagnosticSeverity: map[string]etscore.Severity{"deterministicKeys": etscore.SeverityError},
		Overrides: []etscore.Override{
			{
				Exclude: []string{"generated/**/*.ts"},
				Options: etscore.OverrideOptions{
					DiagnosticSeverity: map[string]etscore.Severity{"deterministicKeys": etscore.SeverityWarning},
				},
			},
			{
				Include: []string{"generated/**/*.ts"},
				Options: etscore.OverrideOptions{
					DiagnosticSeverity: map[string]etscore.Severity{"deterministicKeys": etscore.SeverityOff},
				},
			},
		},
	}

	resolvedSeverity := pluginoptions.ResolveDiagnosticSeverityForFile(opts, "/repo/generated/example.ts", "/repo/tsconfig.json", true)
	if got := resolvedSeverity["deterministicKeys"]; got != etscore.SeverityOff {
		t.Errorf("deterministicKeys severity = %v, want %v", got, etscore.SeverityOff)
	}
}

func intPtr(value int) *int {
	return &value
}

func TestParseFromPlugins_ExitCodeExplicitTrue(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", etscore.EffectPluginName,
		"ignoreEffectSuggestionsInTscExitCode", true,
		"ignoreEffectWarningsInTscExitCode", true,
	))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if !opts.IgnoreEffectSuggestionsInTscExitCode {
		t.Error("expected IgnoreEffectSuggestionsInTscExitCode to be true")
	}
	if !opts.IgnoreEffectWarningsInTscExitCode {
		t.Error("expected IgnoreEffectWarningsInTscExitCode to be true")
	}
}

func TestParseFromPlugins_ExitCodeExplicitFalse(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", etscore.EffectPluginName,
		"ignoreEffectSuggestionsInTscExitCode", false,
		"ignoreEffectWarningsInTscExitCode", false,
	))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if opts.IgnoreEffectSuggestionsInTscExitCode {
		t.Error("expected IgnoreEffectSuggestionsInTscExitCode to be false")
	}
	if opts.IgnoreEffectWarningsInTscExitCode {
		t.Error("expected IgnoreEffectWarningsInTscExitCode to be false")
	}
}

func TestParseFromPlugins_ExitCodeNonBooleanFallsBackToDefault(t *testing.T) {
	// Non-boolean values should fall back to defaults
	plugins := makePlugins(makePluginMap(
		"name", etscore.EffectPluginName,
		"ignoreEffectSuggestionsInTscExitCode", "yes",
		"ignoreEffectWarningsInTscExitCode", 42,
	))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if !opts.IgnoreEffectSuggestionsInTscExitCode {
		t.Error("expected IgnoreEffectSuggestionsInTscExitCode to default to true for non-boolean value")
	}
	if opts.IgnoreEffectWarningsInTscExitCode {
		t.Error("expected IgnoreEffectWarningsInTscExitCode to default to false for non-boolean value")
	}
}

func TestParseFromPlugins_IncludeSuggestionsInTscDefault(t *testing.T) {
	plugins := makePlugins(makePluginMap("name", etscore.EffectPluginName))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if !opts.IncludeSuggestionsInTsc {
		t.Error("expected IncludeSuggestionsInTsc to default to true")
	}
	if !opts.Refactors {
		t.Error("expected Refactors to default to true")
	}
	if !opts.Diagnostics {
		t.Error("expected Diagnostics to default to true")
	}
	if !opts.Quickinfo {
		t.Error("expected Quickinfo to default to true")
	}
	if !opts.Completions {
		t.Error("expected Completions to default to true")
	}
	if !opts.Goto {
		t.Error("expected Goto to default to true")
	}
	if !opts.Renames {
		t.Error("expected Renames to default to true")
	}
}

func TestParseFromPlugins_FeatureFlagsExplicitFalse(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", etscore.EffectPluginName,
		"refactors", false,
		"diagnostics", false,
		"quickinfo", false,
		"completions", false,
		"goto", false,
		"renames", false,
	))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if opts.Refactors {
		t.Error("expected Refactors to be false")
	}
	if opts.Diagnostics {
		t.Error("expected Diagnostics to be false")
	}
	if opts.Quickinfo {
		t.Error("expected Quickinfo to be false")
	}
	if opts.Completions {
		t.Error("expected Completions to be false")
	}
	if opts.Goto {
		t.Error("expected Goto to be false")
	}
	if opts.Renames {
		t.Error("expected Renames to be false")
	}
}

func TestParseFromPlugins_IncludeSuggestionsInTscExplicitTrue(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", etscore.EffectPluginName,
		"includeSuggestionsInTsc", true,
	))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if !opts.IncludeSuggestionsInTsc {
		t.Error("expected IncludeSuggestionsInTsc to be true")
	}
}

func TestParseFromPlugins_IncludeSuggestionsInTscExplicitFalse(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", etscore.EffectPluginName,
		"includeSuggestionsInTsc", false,
	))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if opts.IncludeSuggestionsInTsc {
		t.Error("expected IncludeSuggestionsInTsc to be false")
	}
}

func TestParseFromPlugins_IncludeSuggestionsInTscNonBooleanFallback(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", etscore.EffectPluginName,
		"includeSuggestionsInTsc", "yes",
	))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if !opts.IncludeSuggestionsInTsc {
		t.Error("expected IncludeSuggestionsInTsc to default to true for non-boolean value")
	}
}

func TestGetIncludeSuggestionsInTsc_NilReceiver(t *testing.T) {
	var opts *etscore.EffectPluginOptions
	if !opts.GetIncludeSuggestionsInTsc() {
		t.Error("expected GetIncludeSuggestionsInTsc() on nil to return true")
	}
}

func TestGetMermaidBaseURL(t *testing.T) {
	tests := []struct {
		name     string
		provider string
		want     string
	}{
		{"default empty", "", "https://mermaid.live/edit#"},
		{"mermaid.live", "mermaid.live", "https://mermaid.live/edit#"},
		{"mermaid.com", "mermaid.com", "https://www.mermaidchart.com/play#"},
		{"custom URL", "https://my-mermaid.example.com", "https://my-mermaid.example.com/edit#"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			opts := &etscore.EffectPluginOptions{MermaidProvider: tt.provider}
			if got := opts.GetMermaidBaseURL(); got != tt.want {
				t.Errorf("GetMermaidBaseURL() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestGetMermaidBaseURL_Nil(t *testing.T) {
	var opts *etscore.EffectPluginOptions
	if got := opts.GetMermaidBaseURL(); got != "https://mermaid.live/edit#" {
		t.Errorf("GetMermaidBaseURL() on nil = %q, want default", got)
	}
}

func TestGetLayerGraphFollowDepth(t *testing.T) {
	// Default (zero value)
	opts := &etscore.EffectPluginOptions{}
	if got := opts.GetLayerGraphFollowDepth(); got != 0 {
		t.Errorf("GetLayerGraphFollowDepth() = %d, want 0", got)
	}

	// Non-zero
	opts.LayerGraphFollowDepth = 3
	if got := opts.GetLayerGraphFollowDepth(); got != 3 {
		t.Errorf("GetLayerGraphFollowDepth() = %d, want 3", got)
	}

	// Nil receiver
	var nilOpts *etscore.EffectPluginOptions
	if got := nilOpts.GetLayerGraphFollowDepth(); got != 0 {
		t.Errorf("GetLayerGraphFollowDepth() on nil = %d, want 0", got)
	}
}

func TestParseFromPlugins_MermaidProvider(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", etscore.EffectPluginName,
		"mermaidProvider", "mermaid.com",
	))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if opts.MermaidProvider != "mermaid.com" {
		t.Errorf("MermaidProvider = %q, want %q", opts.MermaidProvider, "mermaid.com")
	}
}

func TestParseFromPlugins_NoExternal(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", etscore.EffectPluginName,
		"noExternal", true,
	))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if !opts.NoExternal {
		t.Error("expected NoExternal to be true")
	}
}

func TestParseFromPlugins_LayerGraphFollowDepth(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", etscore.EffectPluginName,
		"layerGraphFollowDepth", float64(2),
	))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if opts.LayerGraphFollowDepth != 2 {
		t.Errorf("LayerGraphFollowDepth = %d, want 2", opts.LayerGraphFollowDepth)
	}
}

func TestParseFromPlugins_NewOptionsDefaults(t *testing.T) {
	// Plugin with just the name — new options should have zero-value defaults
	plugins := makePlugins(makePluginMap("name", etscore.EffectPluginName))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if opts.MermaidProvider != "" {
		t.Errorf("MermaidProvider = %q, want empty", opts.MermaidProvider)
	}
	if opts.NoExternal {
		t.Error("expected NoExternal to default to false")
	}
	if opts.LayerGraphFollowDepth != 0 {
		t.Errorf("LayerGraphFollowDepth = %d, want 0", opts.LayerGraphFollowDepth)
	}
	if opts.AllowedDuplicatedPackages != nil {
		t.Errorf("AllowedDuplicatedPackages = %v, want nil", opts.AllowedDuplicatedPackages)
	}
}

func TestParseFromPlugins_AllowedDuplicatedPackages(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", etscore.EffectPluginName,
		"allowedDuplicatedPackages", []any{"effect", "@effect/platform"},
	))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	got := opts.AllowedDuplicatedPackages
	if len(got) != 2 {
		t.Fatalf("AllowedDuplicatedPackages length = %d, want 2", len(got))
	}
	if got[0] != "effect" {
		t.Errorf("AllowedDuplicatedPackages[0] = %q, want %q", got[0], "effect")
	}
	if got[1] != "@effect/platform" {
		t.Errorf("AllowedDuplicatedPackages[1] = %q, want %q", got[1], "@effect/platform")
	}
}

func TestAutoImportStyleDefaults(t *testing.T) {
	opts := &etscore.EffectPluginOptions{}

	if got := opts.GetNamespaceImportPackages(); len(got) != 0 {
		t.Errorf("GetNamespaceImportPackages() = %v, want empty list", got)
	}
	if got := opts.GetBarrelImportPackages(); len(got) != 0 {
		t.Errorf("GetBarrelImportPackages() = %v, want empty list", got)
	}
	if got := opts.GetImportAliases(); len(got) != 0 {
		t.Errorf("GetImportAliases() = %v, want empty map", got)
	}
	if got := opts.GetTopLevelNamedReexports(); got != etscore.TopLevelNamedReexportsIgnore {
		t.Errorf("GetTopLevelNamedReexports() = %q, want %q", got, etscore.TopLevelNamedReexportsIgnore)
	}
}

func TestAutoImportStyleNormalization(t *testing.T) {
	opts := &etscore.EffectPluginOptions{
		NamespaceImportPackages: []string{"Effect", " @Effect/Platform ", ""},
		BarrelImportPackages:    []string{"@Effect/Schema", " EFFECT/SQL "},
		ImportAliases: map[string]string{
			"Effect":             "E",
			" @Effect/Platform ": "Platform",
			"":                   "Invalid",
		},
		TopLevelNamedReexports: etscore.TopLevelNamedReexportsMode("FOLLOW"),
	}

	if got, want := opts.GetNamespaceImportPackages(), []string{"effect", "@effect/platform"}; !reflect.DeepEqual(got, want) {
		t.Errorf("GetNamespaceImportPackages() = %v, want %v", got, want)
	}
	if got, want := opts.GetBarrelImportPackages(), []string{"@effect/schema", "effect/sql"}; !reflect.DeepEqual(got, want) {
		t.Errorf("GetBarrelImportPackages() = %v, want %v", got, want)
	}
	if got, want := opts.GetImportAliases(), map[string]string{"effect": "E", "@effect/platform": "Platform"}; !reflect.DeepEqual(got, want) {
		t.Errorf("GetImportAliases() = %v, want %v", got, want)
	}
	if got := opts.GetTopLevelNamedReexports(); got != etscore.TopLevelNamedReexportsFollow {
		t.Errorf("GetTopLevelNamedReexports() = %q, want %q", got, etscore.TopLevelNamedReexportsFollow)
	}
}

func TestTopLevelNamedReexportsInvalidFallback(t *testing.T) {
	opts := &etscore.EffectPluginOptions{TopLevelNamedReexports: etscore.TopLevelNamedReexportsMode("invalid")}
	if got := opts.GetTopLevelNamedReexports(); got != etscore.TopLevelNamedReexportsIgnore {
		t.Errorf("GetTopLevelNamedReexports() = %q, want %q", got, etscore.TopLevelNamedReexportsIgnore)
	}
}

func TestParseFromPlugins_AutoImportStyleDefaults(t *testing.T) {
	plugins := makePlugins(makePluginMap("name", etscore.EffectPluginName))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}

	if got := opts.GetNamespaceImportPackages(); len(got) != 0 {
		t.Errorf("GetNamespaceImportPackages() = %v, want empty list", got)
	}
	if got := opts.GetBarrelImportPackages(); len(got) != 0 {
		t.Errorf("GetBarrelImportPackages() = %v, want empty list", got)
	}
	if got := opts.GetImportAliases(); len(got) != 0 {
		t.Errorf("GetImportAliases() = %v, want empty map", got)
	}
	if got := opts.GetTopLevelNamedReexports(); got != etscore.TopLevelNamedReexportsIgnore {
		t.Errorf("GetTopLevelNamedReexports() = %q, want %q", got, etscore.TopLevelNamedReexportsIgnore)
	}
}

func TestParseFromPlugins_AutoImportStyleValidValues(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", etscore.EffectPluginName,
		"namespaceImportPackages", []any{"Effect", " @Effect/Platform "},
		"barrelImportPackages", []any{"@Effect/Schema", " EFFECT/SQL "},
		"importAliases", makePluginMap(
			"Effect", "E",
			" @Effect/Platform ", "Platform",
		),
		"topLevelNamedReexports", "FOLLOW",
	))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}

	if got, want := opts.GetNamespaceImportPackages(), []string{"effect", "@effect/platform"}; !reflect.DeepEqual(got, want) {
		t.Errorf("GetNamespaceImportPackages() = %v, want %v", got, want)
	}
	if got, want := opts.GetBarrelImportPackages(), []string{"@effect/schema", "effect/sql"}; !reflect.DeepEqual(got, want) {
		t.Errorf("GetBarrelImportPackages() = %v, want %v", got, want)
	}
	if got, want := opts.GetImportAliases(), map[string]string{"effect": "E", "@effect/platform": "Platform"}; !reflect.DeepEqual(got, want) {
		t.Errorf("GetImportAliases() = %v, want %v", got, want)
	}
	if got := opts.GetTopLevelNamedReexports(); got != etscore.TopLevelNamedReexportsFollow {
		t.Errorf("GetTopLevelNamedReexports() = %q, want %q", got, etscore.TopLevelNamedReexportsFollow)
	}
}

func TestParseFromPlugins_AutoImportStyleInvalidFallback(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", etscore.EffectPluginName,
		"namespaceImportPackages", []any{"effect", 1},
		"barrelImportPackages", "not-an-array",
		"importAliases", makePluginMap(
			"effect", "E",
			"@effect/platform", 1,
		),
		"topLevelNamedReexports", "invalid",
	))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}

	if got := opts.GetNamespaceImportPackages(); len(got) != 0 {
		t.Errorf("GetNamespaceImportPackages() = %v, want empty list for mixed array fallback", got)
	}
	if got := opts.GetBarrelImportPackages(); len(got) != 0 {
		t.Errorf("GetBarrelImportPackages() = %v, want empty list for wrong-type fallback", got)
	}
	if got := opts.GetImportAliases(); len(got) != 0 {
		t.Errorf("GetImportAliases() = %v, want empty map for mixed map fallback", got)
	}
	if got := opts.GetTopLevelNamedReexports(); got != etscore.TopLevelNamedReexportsIgnore {
		t.Errorf("GetTopLevelNamedReexports() = %q, want %q for invalid enum fallback", got, etscore.TopLevelNamedReexportsIgnore)
	}
}

func TestParseFromPlugins_AutoImportStyleWrongTypes(t *testing.T) {
	// All fields have completely wrong types (not arrays/maps/strings)
	plugins := makePlugins(makePluginMap(
		"name", etscore.EffectPluginName,
		"namespaceImportPackages", 42,
		"barrelImportPackages", 42,
		"importAliases", "not-a-map",
		"topLevelNamedReexports", 42,
	))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}

	if got := opts.NamespaceImportPackages; got != nil {
		t.Errorf("NamespaceImportPackages = %v, want nil for wrong-type fallback", got)
	}
	if got := opts.BarrelImportPackages; got != nil {
		t.Errorf("BarrelImportPackages = %v, want nil for wrong-type fallback", got)
	}
	if got := opts.ImportAliases; got != nil {
		t.Errorf("ImportAliases = %v, want nil for wrong-type fallback", got)
	}
	if got := opts.TopLevelNamedReexports; got != "" {
		t.Errorf("TopLevelNamedReexports = %q, want empty string for non-string fallback", got)
	}
}

func TestParseFromPlugins_TopLevelNamedReexportsIgnoreCase(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", etscore.EffectPluginName,
		"topLevelNamedReexports", "IGNORE",
	))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if got := opts.TopLevelNamedReexports; got != "ignore" {
		t.Errorf("TopLevelNamedReexports = %q, want %q (case-insensitive)", got, "ignore")
	}
}

func TestAutoImportStyleGetters_NilReceiver(t *testing.T) {
	var opts *etscore.EffectPluginOptions

	if got := opts.GetNamespaceImportPackages(); len(got) != 0 {
		t.Errorf("GetNamespaceImportPackages() on nil = %v, want empty list", got)
	}
	if got := opts.GetBarrelImportPackages(); len(got) != 0 {
		t.Errorf("GetBarrelImportPackages() on nil = %v, want empty list", got)
	}
	if got := opts.GetImportAliases(); len(got) != 0 {
		t.Errorf("GetImportAliases() on nil = %v, want empty map", got)
	}
	if got := opts.GetTopLevelNamedReexports(); got != etscore.TopLevelNamedReexportsIgnore {
		t.Errorf("GetTopLevelNamedReexports() on nil = %q, want %q", got, etscore.TopLevelNamedReexportsIgnore)
	}
}

func TestParseFromPlugins_AutoImportStyleNormalizationDropsEmptyEntries(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", etscore.EffectPluginName,
		"namespaceImportPackages", []any{"  ", "Effect", "\t"},
		"barrelImportPackages", []any{"", " @Effect/Schema "},
		"importAliases", makePluginMap(
			"", "Ignored",
			" ", "AlsoIgnored",
			" Effect ", "E",
		),
	))
	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}

	if got, want := opts.GetNamespaceImportPackages(), []string{"effect"}; !reflect.DeepEqual(got, want) {
		t.Errorf("GetNamespaceImportPackages() = %v, want %v", got, want)
	}
	if got, want := opts.GetBarrelImportPackages(), []string{"@effect/schema"}; !reflect.DeepEqual(got, want) {
		t.Errorf("GetBarrelImportPackages() = %v, want %v", got, want)
	}
	if got, want := opts.GetImportAliases(), map[string]string{"effect": "E"}; !reflect.DeepEqual(got, want) {
		t.Errorf("GetImportAliases() = %v, want %v", got, want)
	}
}
