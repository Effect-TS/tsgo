package etscore

import (
	"reflect"
	"testing"

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
	plugins := makePlugins(makePluginMap("name", EffectPluginName))
	opts := ParseFromPlugins(plugins)
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

func TestParseFromPlugins_ExitCodeExplicitTrue(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", EffectPluginName,
		"ignoreEffectSuggestionsInTscExitCode", true,
		"ignoreEffectWarningsInTscExitCode", true,
	))
	opts := ParseFromPlugins(plugins)
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
		"name", EffectPluginName,
		"ignoreEffectSuggestionsInTscExitCode", false,
		"ignoreEffectWarningsInTscExitCode", false,
	))
	opts := ParseFromPlugins(plugins)
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
		"name", EffectPluginName,
		"ignoreEffectSuggestionsInTscExitCode", "yes",
		"ignoreEffectWarningsInTscExitCode", 42,
	))
	opts := ParseFromPlugins(plugins)
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
			opts := &EffectPluginOptions{MermaidProvider: tt.provider}
			if got := opts.GetMermaidBaseURL(); got != tt.want {
				t.Errorf("GetMermaidBaseURL() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestGetMermaidBaseURL_Nil(t *testing.T) {
	var opts *EffectPluginOptions
	if got := opts.GetMermaidBaseURL(); got != "https://mermaid.live/edit#" {
		t.Errorf("GetMermaidBaseURL() on nil = %q, want default", got)
	}
}

func TestGetLayerGraphFollowDepth(t *testing.T) {
	// Default (zero value)
	opts := &EffectPluginOptions{}
	if got := opts.GetLayerGraphFollowDepth(); got != 0 {
		t.Errorf("GetLayerGraphFollowDepth() = %d, want 0", got)
	}

	// Non-zero
	opts.LayerGraphFollowDepth = 3
	if got := opts.GetLayerGraphFollowDepth(); got != 3 {
		t.Errorf("GetLayerGraphFollowDepth() = %d, want 3", got)
	}

	// Nil receiver
	var nilOpts *EffectPluginOptions
	if got := nilOpts.GetLayerGraphFollowDepth(); got != 0 {
		t.Errorf("GetLayerGraphFollowDepth() on nil = %d, want 0", got)
	}
}

func TestParseFromPlugins_MermaidProvider(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", EffectPluginName,
		"mermaidProvider", "mermaid.com",
	))
	opts := ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if opts.MermaidProvider != "mermaid.com" {
		t.Errorf("MermaidProvider = %q, want %q", opts.MermaidProvider, "mermaid.com")
	}
}

func TestParseFromPlugins_NoExternal(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", EffectPluginName,
		"noExternal", true,
	))
	opts := ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if !opts.NoExternal {
		t.Error("expected NoExternal to be true")
	}
}

func TestParseFromPlugins_LayerGraphFollowDepth(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", EffectPluginName,
		"layerGraphFollowDepth", float64(2),
	))
	opts := ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if opts.LayerGraphFollowDepth != 2 {
		t.Errorf("LayerGraphFollowDepth = %d, want 2", opts.LayerGraphFollowDepth)
	}
}

func TestParseFromPlugins_NewOptionsDefaults(t *testing.T) {
	// Plugin with just the name — new options should have zero-value defaults
	plugins := makePlugins(makePluginMap("name", EffectPluginName))
	opts := ParseFromPlugins(plugins)
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
		"name", EffectPluginName,
		"allowedDuplicatedPackages", []any{"effect", "@effect/platform"},
	))
	opts := ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	got := opts.GetAllowedDuplicatedPackages()
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

func TestGetAllowedDuplicatedPackages_Nil(t *testing.T) {
	var opts *EffectPluginOptions
	if got := opts.GetAllowedDuplicatedPackages(); got != nil {
		t.Errorf("GetAllowedDuplicatedPackages() on nil = %v, want nil", got)
	}
}

func TestAutoImportStyleDefaults(t *testing.T) {
	opts := &EffectPluginOptions{}

	if got := opts.GetNamespaceImportPackages(); len(got) != 0 {
		t.Errorf("GetNamespaceImportPackages() = %v, want empty list", got)
	}
	if got := opts.GetBarrelImportPackages(); len(got) != 0 {
		t.Errorf("GetBarrelImportPackages() = %v, want empty list", got)
	}
	if got := opts.GetImportAliases(); len(got) != 0 {
		t.Errorf("GetImportAliases() = %v, want empty map", got)
	}
	if got := opts.GetTopLevelNamedReexports(); got != TopLevelNamedReexportsIgnore {
		t.Errorf("GetTopLevelNamedReexports() = %q, want %q", got, TopLevelNamedReexportsIgnore)
	}
}

func TestAutoImportStyleNormalization(t *testing.T) {
	opts := &EffectPluginOptions{
		NamespaceImportPackages: []string{"Effect", " @Effect/Platform ", ""},
		BarrelImportPackages:    []string{"@Effect/Schema", " EFFECT/SQL "},
		ImportAliases: map[string]string{
			"Effect":             "E",
			" @Effect/Platform ": "Platform",
			"":                   "Invalid",
		},
		TopLevelNamedReexports: TopLevelNamedReexportsMode("FOLLOW"),
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
	if got := opts.GetTopLevelNamedReexports(); got != TopLevelNamedReexportsFollow {
		t.Errorf("GetTopLevelNamedReexports() = %q, want %q", got, TopLevelNamedReexportsFollow)
	}
}

func TestTopLevelNamedReexportsInvalidFallback(t *testing.T) {
	opts := &EffectPluginOptions{TopLevelNamedReexports: TopLevelNamedReexportsMode("invalid")}
	if got := opts.GetTopLevelNamedReexports(); got != TopLevelNamedReexportsIgnore {
		t.Errorf("GetTopLevelNamedReexports() = %q, want %q", got, TopLevelNamedReexportsIgnore)
	}
}

func TestParseFromPlugins_AutoImportStyleDefaults(t *testing.T) {
	plugins := makePlugins(makePluginMap("name", EffectPluginName))
	opts := ParseFromPlugins(plugins)
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
	if got := opts.GetTopLevelNamedReexports(); got != TopLevelNamedReexportsIgnore {
		t.Errorf("GetTopLevelNamedReexports() = %q, want %q", got, TopLevelNamedReexportsIgnore)
	}
}

func TestParseFromPlugins_AutoImportStyleValidValues(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", EffectPluginName,
		"namespaceImportPackages", []any{"Effect", " @Effect/Platform "},
		"barrelImportPackages", []any{"@Effect/Schema", " EFFECT/SQL "},
		"importAliases", makePluginMap(
			"Effect", "E",
			" @Effect/Platform ", "Platform",
		),
		"topLevelNamedReexports", "FOLLOW",
	))
	opts := ParseFromPlugins(plugins)
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
	if got := opts.GetTopLevelNamedReexports(); got != TopLevelNamedReexportsFollow {
		t.Errorf("GetTopLevelNamedReexports() = %q, want %q", got, TopLevelNamedReexportsFollow)
	}
}

func TestParseFromPlugins_AutoImportStyleInvalidFallback(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", EffectPluginName,
		"namespaceImportPackages", []any{"effect", 1},
		"barrelImportPackages", "not-an-array",
		"importAliases", makePluginMap(
			"effect", "E",
			"@effect/platform", 1,
		),
		"topLevelNamedReexports", "invalid",
	))
	opts := ParseFromPlugins(plugins)
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
	if got := opts.GetTopLevelNamedReexports(); got != TopLevelNamedReexportsIgnore {
		t.Errorf("GetTopLevelNamedReexports() = %q, want %q for invalid enum fallback", got, TopLevelNamedReexportsIgnore)
	}
}

func TestParseFromPlugins_AutoImportStyleWrongTypes(t *testing.T) {
	// All fields have completely wrong types (not arrays/maps/strings)
	plugins := makePlugins(makePluginMap(
		"name", EffectPluginName,
		"namespaceImportPackages", 42,
		"barrelImportPackages", 42,
		"importAliases", "not-a-map",
		"topLevelNamedReexports", 42,
	))
	opts := ParseFromPlugins(plugins)
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
		"name", EffectPluginName,
		"topLevelNamedReexports", "IGNORE",
	))
	opts := ParseFromPlugins(plugins)
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	if got := opts.TopLevelNamedReexports; got != "ignore" {
		t.Errorf("TopLevelNamedReexports = %q, want %q (case-insensitive)", got, "ignore")
	}
}

func TestAutoImportStyleGetters_NilReceiver(t *testing.T) {
	var opts *EffectPluginOptions

	if got := opts.GetNamespaceImportPackages(); len(got) != 0 {
		t.Errorf("GetNamespaceImportPackages() on nil = %v, want empty list", got)
	}
	if got := opts.GetBarrelImportPackages(); len(got) != 0 {
		t.Errorf("GetBarrelImportPackages() on nil = %v, want empty list", got)
	}
	if got := opts.GetImportAliases(); len(got) != 0 {
		t.Errorf("GetImportAliases() on nil = %v, want empty map", got)
	}
	if got := opts.GetTopLevelNamedReexports(); got != TopLevelNamedReexportsIgnore {
		t.Errorf("GetTopLevelNamedReexports() on nil = %q, want %q", got, TopLevelNamedReexportsIgnore)
	}
}

func TestParseFromPlugins_AutoImportStyleNormalizationDropsEmptyEntries(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", EffectPluginName,
		"namespaceImportPackages", []any{"  ", "Effect", "\t"},
		"barrelImportPackages", []any{"", " @Effect/Schema "},
		"importAliases", makePluginMap(
			"", "Ignored",
			" ", "AlsoIgnored",
			" Effect ", "E",
		),
	))
	opts := ParseFromPlugins(plugins)
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
