package etscore_test

import (
	"reflect"
	"testing"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/pluginoptions"
	"github.com/microsoft/TypeScript/tsc/shim/collections"
)

func makePluginMap(pairs ...any) *collections.OrderedMap[string, any] {
	m := new(collections.OrderedMap[string, any])
	for i := 0; i < len(pairs); i += 2 {
		m.Set(pairs[i].(string), pairs[i+1])
	}
	return m
}

func makePlugins(maps ...*collections.OrderedMap[string, any]) []any {
	result := make([]any, len(maps))
	for i, m := range maps {
		result[i] = m
	}
	return result
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
					"allowedUnstableApis", []any{"effect/http"},
					"allowedExperimentalApis", []any{"effect/schema/SchemaJITCompiler"},
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
	if override.Options.AllowedUnstableApis == nil || !reflect.DeepEqual(*override.Options.AllowedUnstableApis, []string{"effect/http"}) {
		t.Fatalf("AllowedUnstableApis = %v, want [effect/http]", override.Options.AllowedUnstableApis)
	}
	if override.Options.AllowedExperimentalApis == nil || !reflect.DeepEqual(*override.Options.AllowedExperimentalApis, []string{"effect/schema/SchemaJITCompiler"}) {
		t.Fatalf("AllowedExperimentalApis = %v, want [effect/schema/SchemaJITCompiler]", override.Options.AllowedExperimentalApis)
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

func TestParseFromPlugins_OverrideImportStyle(t *testing.T) {
	plugins := makePlugins(makePluginMap(
		"name", etscore.EffectPluginName,
		"overrides", []any{
			makePluginMap(
				"include", []any{"src/legacy/**/*.ts"},
				"options", makePluginMap(
					"namespaceImportPackages", []any{},
					"barrelImportPackages", []any{" Effect "},
					"importAliases", makePluginMap("@Effect/Platform", "Platform"),
					"topLevelNamedReexports", "FOLLOW",
				),
			),
		},
	))

	opts := etscore.ParseFromPlugins(plugins)
	if opts == nil || len(opts.Overrides) != 1 {
		t.Fatalf("unexpected options: %+v", opts)
	}
	options := opts.Overrides[0].Options
	if options.NamespaceImportPackages == nil || len(*options.NamespaceImportPackages) != 0 {
		t.Errorf("NamespaceImportPackages = %v, want empty non-nil", options.NamespaceImportPackages)
	}
	if options.BarrelImportPackages == nil || !reflect.DeepEqual(*options.BarrelImportPackages, []string{"effect"}) {
		t.Errorf("BarrelImportPackages = %v, want [effect]", options.BarrelImportPackages)
	}
	if options.ImportAliases == nil || !reflect.DeepEqual(*options.ImportAliases, map[string]string{"@effect/platform": "Platform"}) {
		t.Errorf("ImportAliases = %v, want map[@effect/platform:Platform]", options.ImportAliases)
	}
	if options.TopLevelNamedReexports == nil || *options.TopLevelNamedReexports != etscore.TopLevelNamedReexportsFollow {
		t.Errorf("TopLevelNamedReexports = %v, want follow", options.TopLevelNamedReexports)
	}
}

func TestResolveForFile_OverridesImportStyle(t *testing.T) {
	follow := etscore.TopLevelNamedReexportsFollow
	opts := &etscore.EffectPluginOptions{
		Diagnostics:             true,
		NamespaceImportPackages: []string{"effect"},
		ImportAliases:           map[string]string{"effect": "E"},
		Overrides: []etscore.Override{
			{
				Include: []string{"src/legacy/**/*.ts"},
				Options: etscore.OverrideOptions{
					NamespaceImportPackages: &[]string{},
					BarrelImportPackages:    &[]string{"effect"},
					ImportAliases:           &map[string]string{},
					TopLevelNamedReexports:  &follow,
				},
			},
		},
	}

	legacy := pluginoptions.ResolveEffectPluginOptionsForSourceFile(opts, "/repo/src/legacy/example.ts", "/repo/tsconfig.json", true)
	if len(legacy.NamespaceImportPackages) != 0 {
		t.Errorf("legacy NamespaceImportPackages = %v, want []", legacy.NamespaceImportPackages)
	}
	if !reflect.DeepEqual(legacy.BarrelImportPackages, []string{"effect"}) {
		t.Errorf("legacy BarrelImportPackages = %v, want [effect]", legacy.BarrelImportPackages)
	}
	if len(legacy.ImportAliases) != 0 {
		t.Errorf("legacy ImportAliases = %v, want empty", legacy.ImportAliases)
	}
	if legacy.TopLevelNamedReexports != etscore.TopLevelNamedReexportsFollow {
		t.Errorf("legacy TopLevelNamedReexports = %q, want follow", legacy.TopLevelNamedReexports)
	}

	other := pluginoptions.ResolveEffectPluginOptionsForSourceFile(opts, "/repo/src/app/example.ts", "/repo/tsconfig.json", true)
	if !reflect.DeepEqual(other.NamespaceImportPackages, []string{"effect"}) {
		t.Errorf("other NamespaceImportPackages = %v, want [effect]", other.NamespaceImportPackages)
	}
	if len(other.BarrelImportPackages) != 0 {
		t.Errorf("other BarrelImportPackages = %v, want []", other.BarrelImportPackages)
	}
	if !reflect.DeepEqual(other.ImportAliases, map[string]string{"effect": "E"}) {
		t.Errorf("other ImportAliases = %v, want map[effect:E]", other.ImportAliases)
	}
	if other.TopLevelNamedReexports == etscore.TopLevelNamedReexportsFollow {
		t.Errorf("other TopLevelNamedReexports = follow, want unset")
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

func parseNameOnlyPlugin(t *testing.T, pairs ...any) *etscore.EffectPluginOptions {
	t.Helper()
	opts := etscore.ParseFromPlugins(makePlugins(makePluginMap(append([]any{"name", etscore.EffectPluginName}, pairs...)...)))
	if opts == nil {
		t.Fatal("expected non-nil options")
	}
	return opts
}

func TestParseFromPlugins_BooleanOptions(t *testing.T) {
	for _, tt := range []struct {
		key        string
		field      func(*etscore.EffectPluginOptions) bool
		defaultVal bool
	}{
		{"refactors", func(o *etscore.EffectPluginOptions) bool { return o.Refactors }, true},
		{"diagnostics", func(o *etscore.EffectPluginOptions) bool { return o.Diagnostics }, true},
		{"includeSuggestionsInTsc", func(o *etscore.EffectPluginOptions) bool { return o.IncludeSuggestionsInTsc }, true},
		{"quickinfo", func(o *etscore.EffectPluginOptions) bool { return o.Quickinfo }, true},
		{"completions", func(o *etscore.EffectPluginOptions) bool { return o.Completions }, true},
		{"goto", func(o *etscore.EffectPluginOptions) bool { return o.Goto }, true},
		{"renames", func(o *etscore.EffectPluginOptions) bool { return o.Renames }, true},
		{"ignoreEffectSuggestionsInTscExitCode", func(o *etscore.EffectPluginOptions) bool { return o.IgnoreEffectSuggestionsInTscExitCode }, true},
		{"ignoreEffectWarningsInTscExitCode", func(o *etscore.EffectPluginOptions) bool { return o.IgnoreEffectWarningsInTscExitCode }, false},
		{"noExternal", func(o *etscore.EffectPluginOptions) bool { return o.NoExternal }, false},
	} {
		t.Run(tt.key, func(t *testing.T) {
			for _, c := range []struct {
				name  string
				pairs []any
				want  bool
			}{
				{"omitted", nil, tt.defaultVal},
				{"true", []any{tt.key, true}, true},
				{"false", []any{tt.key, false}, false},
				{"non-boolean", []any{tt.key, "yes"}, tt.defaultVal},
			} {
				if got := tt.field(parseNameOnlyPlugin(t, c.pairs...)); got != c.want {
					t.Errorf("%s %s = %v, want %v", c.name, tt.key, got, c.want)
				}
			}
		})
	}
}

func TestParseFromPlugins_ValueOptions(t *testing.T) {
	defaults := parseNameOnlyPlugin(t)
	if defaults.MermaidProvider != "" || defaults.LayerGraphFollowDepth != 0 || defaults.AllowedDuplicatedPackages != nil {
		t.Errorf("unexpected defaults: MermaidProvider=%q LayerGraphFollowDepth=%d AllowedDuplicatedPackages=%v",
			defaults.MermaidProvider, defaults.LayerGraphFollowDepth, defaults.AllowedDuplicatedPackages)
	}

	opts := parseNameOnlyPlugin(t,
		"mermaidProvider", "mermaid.com",
		"layerGraphFollowDepth", float64(2),
		"allowedDuplicatedPackages", []any{"effect", "@effect/platform"},
		"allowedUnstableApis", []any{"effect/http", "effect/rpc/Client#make"},
		"allowedExperimentalApis", []any{"effect/http", "effect/rpc/Client#make"},
	)
	if opts.MermaidProvider != "mermaid.com" {
		t.Errorf("MermaidProvider = %q, want %q", opts.MermaidProvider, "mermaid.com")
	}
	if opts.LayerGraphFollowDepth != 2 {
		t.Errorf("LayerGraphFollowDepth = %d, want 2", opts.LayerGraphFollowDepth)
	}
	if want := []string{"effect", "@effect/platform"}; !reflect.DeepEqual(opts.AllowedDuplicatedPackages, want) {
		t.Errorf("AllowedDuplicatedPackages = %v, want %v", opts.AllowedDuplicatedPackages, want)
	}
	if want := []string{"effect/http", "effect/rpc/Client#make"}; !reflect.DeepEqual(opts.AllowedUnstableApis, want) {
		t.Errorf("AllowedUnstableApis = %v, want %v", opts.AllowedUnstableApis, want)
	}
	if want := []string{"effect/http", "effect/rpc/Client#make"}; !reflect.DeepEqual(opts.AllowedExperimentalApis, want) {
		t.Errorf("AllowedExperimentalApis = %v, want %v", opts.AllowedExperimentalApis, want)
	}
}

func TestNilOptionsGetters(t *testing.T) {
	var opts *etscore.EffectPluginOptions
	if !opts.GetIncludeSuggestionsInTsc() {
		t.Error("expected GetIncludeSuggestionsInTsc() on nil to return true")
	}
	if got := opts.GetMermaidBaseURL(); got != "https://mermaid.live/edit#" {
		t.Errorf("GetMermaidBaseURL() on nil = %q, want default", got)
	}
	if got := opts.GetLayerGraphFollowDepth(); got != 0 {
		t.Errorf("GetLayerGraphFollowDepth() on nil = %d, want 0", got)
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

func assertImportStyle(t *testing.T, opts *etscore.EffectPluginOptions, namespace, barrel []string, aliases map[string]string, mode etscore.TopLevelNamedReexportsMode) {
	t.Helper()
	if got := opts.GetNamespaceImportPackages(); !reflect.DeepEqual(got, namespace) && (len(got) != 0 || len(namespace) != 0) {
		t.Errorf("GetNamespaceImportPackages() = %v, want %v", got, namespace)
	}
	if got := opts.GetBarrelImportPackages(); !reflect.DeepEqual(got, barrel) && (len(got) != 0 || len(barrel) != 0) {
		t.Errorf("GetBarrelImportPackages() = %v, want %v", got, barrel)
	}
	if got := opts.GetImportAliases(); !reflect.DeepEqual(got, aliases) && (len(got) != 0 || len(aliases) != 0) {
		t.Errorf("GetImportAliases() = %v, want %v", got, aliases)
	}
	if got := opts.GetTopLevelNamedReexports(); got != mode {
		t.Errorf("GetTopLevelNamedReexports() = %q, want %q", got, mode)
	}
}

func TestAutoImportStyleFallsBackToDefaults(t *testing.T) {
	for _, tt := range []struct {
		name string
		opts func(t *testing.T) *etscore.EffectPluginOptions
	}{
		{"nil receiver", func(*testing.T) *etscore.EffectPluginOptions { return nil }},
		{"zero options", func(*testing.T) *etscore.EffectPluginOptions { return &etscore.EffectPluginOptions{} }},
		{"invalid mode", func(*testing.T) *etscore.EffectPluginOptions {
			return &etscore.EffectPluginOptions{TopLevelNamedReexports: etscore.TopLevelNamedReexportsMode("invalid")}
		}},
		{"name-only plugin", func(t *testing.T) *etscore.EffectPluginOptions { return parseNameOnlyPlugin(t) }},
		{"mixed or invalid values", func(t *testing.T) *etscore.EffectPluginOptions {
			return parseNameOnlyPlugin(t,
				"namespaceImportPackages", []any{"effect", 1},
				"barrelImportPackages", "not-an-array",
				"importAliases", makePluginMap("effect", "E", "@effect/platform", 1),
				"topLevelNamedReexports", "invalid",
			)
		}},
		{"wrong types", func(t *testing.T) *etscore.EffectPluginOptions {
			return parseNameOnlyPlugin(t,
				"namespaceImportPackages", 42,
				"barrelImportPackages", 42,
				"importAliases", "not-a-map",
				"topLevelNamedReexports", 42,
			)
		}},
	} {
		t.Run(tt.name, func(t *testing.T) {
			assertImportStyle(t, tt.opts(t), nil, nil, nil, etscore.TopLevelNamedReexportsIgnore)
		})
	}
}

func TestAutoImportStyleNormalization(t *testing.T) {
	namespace := []string{"effect", "@effect/platform"}
	barrel := []string{"@effect/schema", "effect/sql"}
	aliases := map[string]string{"effect": "E", "@effect/platform": "Platform"}

	assertImportStyle(t, &etscore.EffectPluginOptions{
		NamespaceImportPackages: []string{"Effect", " @Effect/Platform ", ""},
		BarrelImportPackages:    []string{"@Effect/Schema", " EFFECT/SQL "},
		ImportAliases: map[string]string{
			"Effect":             "E",
			" @Effect/Platform ": "Platform",
			"":                   "Invalid",
		},
		TopLevelNamedReexports: etscore.TopLevelNamedReexportsMode("FOLLOW"),
	}, namespace, barrel, aliases, etscore.TopLevelNamedReexportsFollow)

	assertImportStyle(t, parseNameOnlyPlugin(t,
		"namespaceImportPackages", []any{"  ", "Effect", "\t", " @Effect/Platform "},
		"barrelImportPackages", []any{"", "@Effect/Schema", " EFFECT/SQL "},
		"importAliases", makePluginMap(
			"", "Ignored",
			" ", "AlsoIgnored",
			"Effect", "E",
			" @Effect/Platform ", "Platform",
		),
		"topLevelNamedReexports", "FOLLOW",
	), namespace, barrel, aliases, etscore.TopLevelNamedReexportsFollow)

	if got := parseNameOnlyPlugin(t, "topLevelNamedReexports", "IGNORE").TopLevelNamedReexports; got != etscore.TopLevelNamedReexportsIgnore {
		t.Errorf("TopLevelNamedReexports = %q, want %q (case-insensitive)", got, etscore.TopLevelNamedReexportsIgnore)
	}
}

func TestParseFromPlugins_BrandedSchemaClasses(t *testing.T) {
	resolve := func(pairs ...any) *etscore.ResolvedEffectPluginOptions {
		t.Helper()
		return pluginoptions.ResolveEffectPluginOptionsForSourceFile(parseNameOnlyPlugin(t, pairs...), "/repo/src/example.ts", "/repo/tsconfig.json", true)
	}

	if got := resolve().GetBrandedSchemaClasses(); !reflect.DeepEqual(got, etscore.DefaultBrandedSchemaClasses) {
		t.Errorf("default GetBrandedSchemaClasses() = %v, want %v", got, etscore.DefaultBrandedSchemaClasses)
	}

	reduced := resolve("brandedSchemaClasses", []any{"Class", "Opaque"})
	if !reduced.BrandedSchemaClassesIncludes("Class") || !reduced.BrandedSchemaClassesIncludes("Opaque") || reduced.BrandedSchemaClassesIncludes("TaggedError") {
		t.Errorf("reduced GetBrandedSchemaClasses() = %v, want [Class Opaque]", reduced.GetBrandedSchemaClasses())
	}

	empty := resolve("brandedSchemaClasses", []any{})
	if got := empty.GetBrandedSchemaClasses(); got == nil || len(got) != 0 {
		t.Errorf("empty GetBrandedSchemaClasses() = %#v, want empty non-default list", got)
	}

	overridden := resolve(
		"brandedSchemaClasses", []any{},
		"overrides", []any{makePluginMap(
			"include", []any{"src/**/*.ts"},
			"options", makePluginMap("brandedSchemaClasses", []any{"TaggedClass"}),
		)},
	)
	if got := overridden.GetBrandedSchemaClasses(); !reflect.DeepEqual(got, []string{"TaggedClass"}) {
		t.Errorf("overridden GetBrandedSchemaClasses() = %v, want [TaggedClass]", got)
	}
}
