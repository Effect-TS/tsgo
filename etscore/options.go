package etscore

import "strings"

// KeyPattern defines a key pattern configuration for the deterministicKeys rule.
type KeyPattern struct {
	// Target is the category this pattern applies to: "service", "error", or "custom".
	Target string `json:"target" schema_description:"Target category this pattern applies to." schema_default:"\"service\"" schema_enum:"[\"service\",\"error\",\"custom\"]"`

	// Pattern is the formula to compute the key: "default", "package-identifier", or "default-hashed".
	Pattern string `json:"pattern" schema_description:"Formula used to compute the key." schema_default:"\"default\"" schema_enum:"[\"default\",\"package-identifier\",\"default-hashed\"]"`

	// SkipLeadingPath is a list of path prefixes to strip from the sub-directory segment.
	SkipLeadingPath []string `json:"skipLeadingPath" schema_description:"Path prefixes to strip from the sub-directory segment." schema_default:"[\"src/\"]" schema_items_type:"string"`
}

// EffectPluginOptions defines the configuration schema for
// `@effect/language-service` as parsed from tsconfig plugin JSON.
// This type is stored in CompilerOptions.Effect after parsing tsconfig.json.
type EffectPluginOptions struct {
	// Refactors enables Effect refactor actions in the language service.
	Refactors bool `json:"refactors,omitzero" schema_description:"Controls Effect refactors." schema_default:"true"`

	// Diagnostics enables Effect diagnostics in the checker.
	Diagnostics bool `json:"diagnostics,omitzero" schema_description:"Controls Effect diagnostics." schema_default:"true"`

	// IncludeSuggestionsInTsc controls whether suggestion-level Effect diagnostics appear
	// in tsc CLI output. Default: true (suggestions are included).
	IncludeSuggestionsInTsc bool `json:"includeSuggestionsInTsc,omitzero" schema_description:"When false, suggestion-level Effect diagnostics are omitted from tsc CLI output." schema_default:"true"`

	// Quickinfo enables Effect hover enrichment in the language service.
	Quickinfo bool `json:"quickinfo,omitzero" schema_description:"Controls Effect quickinfo." schema_default:"true"`

	// Completions enables Effect completions in the language service.
	Completions bool `json:"completions,omitzero" schema_description:"Controls Effect completions." schema_default:"true"`

	// Debug enables extra debugging-oriented language service output.
	Debug bool `json:"debug,omitzero" schema_description:"Enables additional debug-only Effect language service output." schema_default:"false"`

	// Goto enables Effect goto/definition helpers in the language service.
	Goto bool `json:"goto,omitzero" schema_description:"Controls Effect goto references support." schema_default:"true"`

	// Renames enables Effect rename helpers in the language service.
	Renames bool `json:"renames,omitzero" schema_description:"Controls Effect rename helpers." schema_default:"true"`

	// IgnoreEffectSuggestionsInTscExitCode controls whether Effect suggestion/message-category
	// diagnostics affect the tsc exit code. Default: true (suggestions do NOT affect exit code).
	IgnoreEffectSuggestionsInTscExitCode bool `json:"ignoreEffectSuggestionsInTscExitCode,omitzero" schema_description:"When true, suggestion diagnostics do not affect the tsc exit code." schema_default:"true"`

	// IgnoreEffectWarningsInTscExitCode controls whether Effect warning-category diagnostics
	// affect the tsc exit code. Default: false (warnings DO affect exit code).
	IgnoreEffectWarningsInTscExitCode bool `json:"ignoreEffectWarningsInTscExitCode,omitzero" schema_description:"When true, warning diagnostics do not affect the tsc exit code." schema_default:"false"`

	// IgnoreEffectErrorsInTscExitCode controls whether Effect error-category diagnostics
	// affect the tsc exit code. Default: false (errors DO affect exit code).
	IgnoreEffectErrorsInTscExitCode bool `json:"ignoreEffectErrorsInTscExitCode,omitzero" schema_description:"When true, error diagnostics do not affect the tsc exit code." schema_default:"false"`

	// SkipDisabledOptimization bypasses the optimization that skips off-severity rules entirely.
	// When true, disabled rules are still processed so per-line or per-section directive overrides
	// can enable them.
	SkipDisabledOptimization bool `json:"skipDisabledOptimization,omitzero" schema_description:"When true, disabled diagnostics are still processed so directives can re-enable them." schema_default:"false"`

	// MermaidProvider selects the Mermaid rendering service for hover links.
	// Accepted values: "" or "mermaid.live" (default), "mermaid.com", or a custom URL.
	MermaidProvider string `json:"mermaidProvider,omitzero" schema_description:"Mermaid rendering service for layer graph links. Accepts mermaid.live, mermaid.com, or a custom URL." schema_default:"\"mermaid.live\""`

	// NoExternal suppresses external links (Mermaid URLs) in hover output. Default: false.
	NoExternal bool `json:"noExternal,omitzero" schema_description:"When true, suppresses external Mermaid links in hover output." schema_default:"false"`

	// LayerGraphFollowDepth controls how many levels deep the graph extraction
	// follows symbol references when building the layer graph. Default: 0.
	LayerGraphFollowDepth int `json:"layerGraphFollowDepth,omitzero" schema_description:"How many levels deep the layer graph extraction follows symbol references." schema_default:"0" schema_minimum:"0"`

	// Inlays enables inlay hint middleware. When true, suppresses redundant
	// return-type inlay hints on Effect.gen, Effect.fn, and Effect.fnUntraced
	// generator functions. Default: false.
	Inlays bool `json:"inlays,omitzero" schema_description:"When true, suppresses redundant return-type inlay hints on supported Effect generator functions." schema_default:"false"`

	// NamespaceImportPackages configures package names that should prefer namespace imports.
	// Package matching is case-insensitive.
	NamespaceImportPackages []string `json:"namespaceImportPackages,omitzero" schema_description:"Package names that should prefer namespace imports." schema_default:"[]" schema_items_type:"string"`

	// BarrelImportPackages configures package names that should prefer barrel named imports.
	// Package matching is case-insensitive.
	BarrelImportPackages []string `json:"barrelImportPackages,omitzero" schema_description:"Package names that should prefer barrel named imports." schema_default:"[]" schema_items_type:"string"`

	// ImportAliases configures package-level import aliases keyed by package name.
	// Package matching for keys is case-insensitive.
	ImportAliases map[string]string `json:"importAliases,omitzero" schema_description:"Package-level import aliases keyed by package name." schema_default:"{}" schema_additional_properties_type:"string"`

	// TopLevelNamedReexports controls whether named reexports are followed at package top-level.
	// Accepted values are "ignore" (default) and "follow".
	TopLevelNamedReexports TopLevelNamedReexportsMode `json:"topLevelNamedReexports,omitzero" schema_description:"Controls whether named reexports are followed at package top-level." schema_default:"\"ignore\"" schema_enum:"[\"ignore\",\"follow\"]"`

	// KeyPatterns configures key pattern formulas for the deterministicKeys rule.
	// If nil, GetKeyPatterns() returns the defaults.
	KeyPatterns []KeyPattern `json:"keyPatterns,omitzero" schema_description:"Configures key pattern formulas for the deterministicKeys rule."`

	// ExtendedKeyDetection enables matching constructors with @effect-identifier annotations.
	ExtendedKeyDetection bool `json:"extendedKeyDetection,omitzero" schema_description:"Enables matching constructors with @effect-identifier annotations." schema_default:"false"`

	// PipeableMinArgCount is the minimum number of contiguous pipeable transformations
	// required to trigger the missedPipeableOpportunity diagnostic. Default: 2.
	PipeableMinArgCount int `json:"pipeableMinArgCount,omitzero" schema_description:"Minimum number of contiguous pipeable transformations to trigger missedPipeableOpportunity." schema_default:"2" schema_minimum:"1"`

	// AllowedDuplicatedPackages is a list of package names that are allowed to
	// have multiple versions without triggering the duplicatePackage diagnostic.
	AllowedDuplicatedPackages []string `json:"allowedDuplicatedPackages,omitzero" schema_description:"Package names allowed to have multiple versions without triggering duplicatePackage." schema_default:"[]" schema_items_type:"string"`

	// EffectFn controls which effectFnOpportunity quickfix variants are offered.
	// Valid values: "span", "untraced", "no-span", "inferred-span", "suggested-span".
	// Default (when empty/nil): ["span"].
	EffectFn []string `json:"effectFn,omitzero" schema_description:"Controls which effectFnOpportunity quickfix variants are offered." schema_default:"[\"span\"]" schema_items_type:"string" schema_items_enum:"[\"span\",\"untraced\",\"no-span\",\"inferred-span\",\"suggested-span\"]" schema_unique_items:"true"`

	// DiagnosticSeverity maps rule names to severity levels.
	// If nil, diagnostics are explicitly disabled.
	// If empty map {}, diagnostics are enabled with defaults.
	DiagnosticSeverity map[string]Severity `json:"diagnosticSeverity,omitzero" schema_description:"Maps rule names to severity levels. Use {} to enable diagnostics with rule defaults."`

	// Overrides applies ordered per-file diagnostic option overrides.
	Overrides []Override `json:"overrides,omitzero" schema_description:"Ordered per-file diagnostic option overrides."`
}

// ResolvedEffectPluginOptions contains Effect plugin options that may vary per
// source file after applying matching overrides.
type ResolvedEffectPluginOptions struct {
	NamespaceImportPackages   []string
	BarrelImportPackages      []string
	ImportAliases             map[string]string
	TopLevelNamedReexports    TopLevelNamedReexportsMode
	KeyPatterns               []KeyPattern
	ExtendedKeyDetection      bool
	PipeableMinArgCount       int
	AllowedDuplicatedPackages []string
	EffectFn                  []string
}

// EffectPluginDiagnosticsOptions is a compatibility alias for the resolved
// per-file diagnostic options.
type EffectPluginDiagnosticsOptions = ResolvedEffectPluginOptions

// Override is a path-scoped partial override for diagnostic configuration.
type Override struct {
	Include []string        `json:"include,omitzero" schema_description:"Files included by this override." schema_items_type:"string"`
	Exclude []string        `json:"exclude,omitzero" schema_description:"Files excluded from this override." schema_items_type:"string"`
	Options OverrideOptions `json:"options,omitzero" schema_description:"Diagnostic option overrides applied when this override matches."`
}

// OverrideOptions is the subset of plugin options that can vary per file.
type OverrideOptions struct {
	DiagnosticSeverity        map[string]Severity `json:"diagnosticSeverity,omitzero" schema_description:"Severity overrides applied when this override matches."`
	PipeableMinArgCount       *int                `json:"pipeableMinArgCount,omitzero" schema_description:"Minimum number of contiguous pipeable transformations to trigger missedPipeableOpportunity." schema_minimum:"1"`
	KeyPatterns               *[]KeyPattern       `json:"keyPatterns,omitzero" schema_description:"Scoped key pattern configuration for deterministicKeys."`
	ExtendedKeyDetection      *bool               `json:"extendedKeyDetection,omitzero" schema_description:"Scoped override for extended deterministic key detection."`
	AllowedDuplicatedPackages *[]string           `json:"allowedDuplicatedPackages,omitzero" schema_description:"Scoped allow-list for duplicatePackage." schema_items_type:"string"`
	EffectFn                  *[]string           `json:"effectFn,omitzero" schema_description:"Scoped override for effectFn quickfix variants." schema_items_type:"string" schema_items_enum:"[\"span\",\"untraced\",\"no-span\",\"inferred-span\",\"suggested-span\"]" schema_unique_items:"true"`
}

// TopLevelNamedReexportsMode configures top-level named reexport resolution behavior.
type TopLevelNamedReexportsMode string

const (
	TopLevelNamedReexportsIgnore TopLevelNamedReexportsMode = "ignore"
	TopLevelNamedReexportsFollow TopLevelNamedReexportsMode = "follow"
)

// EffectFn variant constants.
const (
	EffectFnSpan          = "span"
	EffectFnUntraced      = "untraced"
	EffectFnNoSpan        = "no-span"
	EffectFnInferredSpan  = "inferred-span"
	EffectFnSuggestedSpan = "suggested-span"
)

// DefaultEffectFn is the default effectFn configuration.
var DefaultEffectFn = []string{EffectFnSpan}

// DiagnosticsEnabled reports whether Effect diagnostics are enabled for the
// parsed config in the current program.
func DiagnosticsEnabled(config *EffectPluginOptions) bool {
	return config != nil && config.Diagnostics && config.DiagnosticSeverity != nil
}

// GetEffectFn returns the configured effectFn variants, or the default ["span"] when unset/empty.
func (e *ResolvedEffectPluginOptions) GetEffectFn() []string {
	if e == nil || len(e.EffectFn) == 0 {
		return DefaultEffectFn
	}
	return e.EffectFn
}

// EffectFnIncludes checks if a variant is in the configured (or default) effectFn list.
func (e *ResolvedEffectPluginOptions) EffectFnIncludes(variant string) bool {
	for _, v := range e.GetEffectFn() {
		if v == variant {
			return true
		}
	}
	return false
}

func (e *EffectPluginOptions) GetRefactorsEnabled() bool {
	if e == nil {
		return true
	}
	return e.Refactors
}

func (e *EffectPluginOptions) GetQuickinfoEnabled() bool {
	if e == nil {
		return true
	}
	return e.Quickinfo
}

func (e *EffectPluginOptions) GetCompletionsEnabled() bool {
	if e == nil {
		return true
	}
	return e.Completions
}

func (e *EffectPluginOptions) GetDebugEnabled() bool {
	if e == nil {
		return false
	}
	return e.Debug
}

// GetIncludeSuggestionsInTsc returns whether suggestion diagnostics should appear in tsc output.
// Returns true (include suggestions) when the receiver is nil.
func (e *EffectPluginOptions) GetIncludeSuggestionsInTsc() bool {
	if e == nil {
		return true
	}
	return e.IncludeSuggestionsInTsc
}

// GetPipeableMinArgCount returns the configured minimum pipeable arg count, or 2 if not set.
func (e *ResolvedEffectPluginOptions) GetPipeableMinArgCount() int {
	if e != nil && e.PipeableMinArgCount > 0 {
		return e.PipeableMinArgCount
	}
	return 2
}

// DefaultKeyPatterns is the default key patterns configuration.
var DefaultKeyPatterns = []KeyPattern{
	{Target: "service", Pattern: "default", SkipLeadingPath: []string{"src/"}},
	{Target: "custom", Pattern: "default", SkipLeadingPath: []string{"src/"}},
}

// GetKeyPatterns returns the configured key patterns or the defaults if none are configured.
func (e *ResolvedEffectPluginOptions) GetKeyPatterns() []KeyPattern {
	if e == nil || len(e.KeyPatterns) == 0 {
		return DefaultKeyPatterns
	}
	return e.KeyPatterns
}

// GetMermaidBaseURL resolves the MermaidProvider value to a full base URL.
//   - "" or "mermaid.live" → "https://mermaid.live/edit#"
//   - "mermaid.com" → "https://www.mermaidchart.com/play#"
//   - Any other string → "<value>/edit#"
func (e *EffectPluginOptions) GetMermaidBaseURL() string {
	provider := ""
	if e != nil {
		provider = e.MermaidProvider
	}
	switch provider {
	case "", "mermaid.live":
		return "https://mermaid.live/edit#"
	case "mermaid.com":
		return "https://www.mermaidchart.com/play#"
	default:
		return provider + "/edit#"
	}
}

// GetLayerGraphFollowDepth returns the configured layer graph follow depth, or 0 if not set.
func (e *EffectPluginOptions) GetLayerGraphFollowDepth() int {
	if e != nil && e.LayerGraphFollowDepth > 0 {
		return e.LayerGraphFollowDepth
	}
	return 0
}

// GetAllowedDuplicatedPackages returns the list of package names allowed to
// have multiple versions, or nil if none are configured.
func (e *ResolvedEffectPluginOptions) GetAllowedDuplicatedPackages() []string {
	if e == nil {
		return nil
	}
	return e.AllowedDuplicatedPackages
}

// GetNamespaceImportPackages returns normalized package names configured for namespace imports.
// Defaults to an empty list when unset.
func (e *EffectPluginOptions) GetNamespaceImportPackages() []string {
	if e == nil {
		return []string{}
	}
	return normalizePackageList(e.NamespaceImportPackages)
}

// GetBarrelImportPackages returns normalized package names configured for barrel imports.
// Defaults to an empty list when unset.
func (e *EffectPluginOptions) GetBarrelImportPackages() []string {
	if e == nil {
		return []string{}
	}
	return normalizePackageList(e.BarrelImportPackages)
}

// GetImportAliases returns normalized import aliases keyed by lower-cased package names.
// Defaults to an empty map when unset.
func (e *EffectPluginOptions) GetImportAliases() map[string]string {
	if e == nil {
		return map[string]string{}
	}
	return normalizeImportAliases(e.ImportAliases)
}

// GetTopLevelNamedReexports returns the normalized top-level named reexports mode.
// Defaults to "ignore" when unset or invalid.
func (e *EffectPluginOptions) GetTopLevelNamedReexports() TopLevelNamedReexportsMode {
	if e == nil {
		return TopLevelNamedReexportsIgnore
	}
	switch TopLevelNamedReexportsMode(strings.ToLower(string(e.TopLevelNamedReexports))) {
	case TopLevelNamedReexportsFollow:
		return TopLevelNamedReexportsFollow
	default:
		return TopLevelNamedReexportsIgnore
	}
}

func normalizePackageList(packages []string) []string {
	if len(packages) == 0 {
		return []string{}
	}
	normalized := make([]string, 0, len(packages))
	for _, pkg := range packages {
		if pkg = normalizePackageName(pkg); pkg != "" {
			normalized = append(normalized, pkg)
		}
	}
	return normalized
}

func normalizeImportAliases(aliases map[string]string) map[string]string {
	if len(aliases) == 0 {
		return map[string]string{}
	}
	normalized := make(map[string]string, len(aliases))
	for pkg, alias := range aliases {
		if pkg = normalizePackageName(pkg); pkg != "" {
			normalized[pkg] = alias
		}
	}
	return normalized
}

func normalizePackageName(pkg string) string {
	return strings.ToLower(strings.TrimSpace(pkg))
}
