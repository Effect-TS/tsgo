use std::borrow::Cow;
use std::sync::LazyLock;

use rustc_hash::FxHashMap;

use ets_go::reflect::{StructField, StructType, Type};

use crate::*;

// Rust: the Go structs below carry `json:` tags (the `.tsbuildinfo` encoding of patch 028) and `schema_*` tags
// (read by the schema generator in options_schema_test.go). The tags reach Rust as the Rust-only `TYPE` constants
// below each struct (`ets_go::reflect`: the Go fields with their verbatim tags), which the ported schema generator
// reads; the JSON encoding itself belongs to the port of patch 028. A Go slice or map that may be nil is an
// `Option` here whenever nil and empty differ for `encoding/json` or `reflect.DeepEqual` (patch 007's `Equals`).
// A Go `*[]T` / `*map` override field is an `Option<Vec<T>>` / `Option<FxHashMap<..>>`: a non-nil pointer to a
// nil slice (only `cloneStringSlicePtr` of an empty slice builds one) and a pointer to an empty one both encode
// as `[]` and are not told apart.

// go: etscore/options.go:9 KeyPattern 76fa156498920a0b
// KeyPattern defines a key pattern configuration for the deterministicKeys rule.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct KeyPattern {
    // Target is the category this pattern applies to: "service", "error", "custom", or "taggedClass".
    pub target: String,

    // Pattern is the formula to compute the key: "default", "package-identifier", "default-hashed", or "identifier".
    pub pattern: String,

    // SkipLeadingPath is a list of path prefixes to strip from the sub-directory segment.
    pub skip_leading_path: Option<Vec<String>>,
}

// Rust-only: Go's `reflect.TypeOf(KeyPattern{})` as the schema generator in options_schema_test.go reads it:
// every field in Go order with its Go name, type kind and verbatim struct tag. Keep it in sync with the Go
// struct's tags.
impl KeyPattern {
    pub const TYPE: StructType = StructType {
        fields: &[
            StructField::new(
                "Target",
                &Type::STRING,
                r#"json:"target" schema_description:"Target category this pattern applies to." schema_default:"\"service\"" schema_enum:"[\"service\",\"error\",\"custom\",\"taggedClass\"]""#,
            ),
            StructField::new(
                "Pattern",
                &Type::STRING,
                r#"json:"pattern" schema_description:"Formula used to compute the key." schema_default:"\"default\"" schema_enum:"[\"default\",\"package-identifier\",\"default-hashed\",\"identifier\"]""#,
            ),
            StructField::new(
                "SkipLeadingPath",
                &Type::SLICE,
                r#"json:"skipLeadingPath" schema_description:"Path prefixes to strip from the sub-directory segment." schema_default:"[\"src/\"]" schema_items_type:"string""#,
            ),
        ],
    };
}

// go: etscore/options.go:23 EffectPluginOptions de58f6dc1e78a96c
// EffectPluginOptions defines the configuration schema for
// `@effect/language-service` as parsed from tsconfig plugin JSON.
// This type is stored in CompilerOptions.Effect after parsing tsconfig.json.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct EffectPluginOptions {
    // Refactors enables Effect refactor actions in the language service.
    pub refactors: bool,

    // Diagnostics enables Effect diagnostics in the checker.
    pub diagnostics: bool,

    // IncludeSuggestionsInTsc controls whether suggestion-level Effect diagnostics appear
    // in tsc CLI output. Default: true (suggestions are included).
    pub include_suggestions_in_tsc: bool,

    // Quickinfo enables Effect hover enrichment in the language service.
    pub quickinfo: bool,

    // Completions enables Effect completions in the language service.
    pub completions: bool,

    // Goto enables Effect goto/definition helpers in the language service.
    pub goto: bool,

    // Renames enables Effect rename helpers in the language service.
    pub renames: bool,

    // IgnoreEffectSuggestionsInTscExitCode controls whether Effect suggestion/message-category
    // diagnostics affect the tsc exit code. Default: true (suggestions do NOT affect exit code).
    pub ignore_effect_suggestions_in_tsc_exit_code: bool,

    // IgnoreEffectWarningsInTscExitCode controls whether Effect warning-category diagnostics
    // affect the tsc exit code. Default: false (warnings DO affect exit code).
    pub ignore_effect_warnings_in_tsc_exit_code: bool,

    // IgnoreEffectErrorsInTscExitCode controls whether Effect error-category diagnostics
    // affect the tsc exit code. Default: false (errors DO affect exit code).
    pub ignore_effect_errors_in_tsc_exit_code: bool,

    // SkipDisabledOptimization bypasses the optimization that skips off-severity rules entirely.
    // When true, disabled rules are still processed so per-line or per-section directive overrides
    // can enable them.
    pub skip_disabled_optimization: bool,

    // MermaidProvider selects the Mermaid rendering service for hover links.
    // Accepted values: "" or "mermaid.live" (default), "mermaid.com", or a custom URL.
    pub mermaid_provider: String,

    // NoExternal suppresses external links (Mermaid URLs) in hover output. Default: false.
    pub no_external: bool,

    // LayerGraphFollowDepth controls how many levels deep the graph extraction
    // follows symbol references when building the layer graph. Default: 0.
    pub layer_graph_follow_depth: i64,

    // Inlays enables inlay hint middleware. When true, suppresses redundant
    // return-type inlay hints on Effect.gen, Effect.fn, and Effect.fnUntraced
    // generator functions. Default: false.
    pub inlays: bool,

    // NamespaceImportPackages configures package names that should prefer namespace imports.
    // Package matching is case-insensitive.
    pub namespace_import_packages: Option<Vec<String>>,

    // BarrelImportPackages configures package names that should prefer barrel named imports.
    // Package matching is case-insensitive.
    pub barrel_import_packages: Option<Vec<String>>,

    // ImportAliases configures package-level import aliases keyed by package name.
    // Package matching for keys is case-insensitive.
    pub import_aliases: Option<FxHashMap<String, String>>,

    // TopLevelNamedReexports controls whether named reexports are followed at package top-level.
    // Accepted values are "ignore" (default) and "follow".
    pub top_level_named_reexports: TopLevelNamedReexportsMode,

    // KeyPatterns configures key pattern formulas for the deterministicKeys rule.
    // If nil, GetKeyPatterns() returns the defaults.
    pub key_patterns: Option<Vec<KeyPattern>>,

    // ExtendedKeyDetection enables matching constructors with @effect-identifier annotations.
    pub extended_key_detection: bool,

    // PipeableMinArgCount is the minimum number of contiguous pipeable transformations
    // required to trigger the missedPipeableOpportunity diagnostic. Default: 2.
    pub pipeable_min_arg_count: i64,

    // AllowedUnstableApis allows unstable declaration modules, subtrees, or exports.
    pub allowed_unstable_apis: Option<Vec<String>>,

    // AllowedExperimentalApis allows experimental declaration modules, subtrees, or exports.
    pub allowed_experimental_apis: Option<Vec<String>>,

    // AllowedDuplicatedPackages permits multiple versions of these packages.
    pub allowed_duplicated_packages: Option<Vec<String>>,

    // EffectFn controls which effectFnOpportunity quickfix variants are offered.
    // Valid values: "span", "untraced", "no-span", "inferred-span", "suggested-span".
    // Default (when empty/nil): ["span"].
    pub effect_fn: Option<Vec<String>>,

    // BrandedSchemaClasses controls which Schema class kinds get a nominal brand.
    // Valid values: "Class", "TaggedClass", "Error", "TaggedError", "Opaque".
    // Default (when nil): all five. An empty list disables the brand for every kind.
    pub branded_schema_classes: Option<Vec<String>>,

    // DiagnosticSeverity maps rule names to severity levels.
    // If nil, diagnostics are explicitly disabled.
    // If empty map {}, diagnostics are enabled with defaults.
    pub diagnostic_severity: Option<FxHashMap<String, Severity>>,

    // Overrides applies ordered per-file diagnostic option overrides.
    pub overrides: Option<Vec<Override>>,
}

// Rust-only: Go's `reflect.TypeOf(EffectPluginOptions{})` as the schema generator in options_schema_test.go reads it:
// every field in Go order with its Go name, type kind and verbatim struct tag. Keep it in sync with the Go
// struct's tags.
impl EffectPluginOptions {
    pub const TYPE: StructType = StructType {
        fields: &[
            StructField::new(
                "Refactors",
                &Type::BOOL,
                r#"json:"refactors,omitzero" schema_description:"Controls Effect refactors." schema_default:"true""#,
            ),
            StructField::new(
                "Diagnostics",
                &Type::BOOL,
                r#"json:"diagnostics,omitzero" schema_description:"Controls Effect diagnostics." schema_default:"true""#,
            ),
            StructField::new(
                "IncludeSuggestionsInTsc",
                &Type::BOOL,
                r#"json:"includeSuggestionsInTsc,omitzero" schema_description:"When false, suggestion-level Effect diagnostics are omitted from tsc CLI output." schema_default:"true""#,
            ),
            StructField::new(
                "Quickinfo",
                &Type::BOOL,
                r#"json:"quickinfo,omitzero" schema_description:"Controls Effect quickinfo." schema_default:"true""#,
            ),
            StructField::new(
                "Completions",
                &Type::BOOL,
                r#"json:"completions,omitzero" schema_description:"Controls Effect completions." schema_default:"true""#,
            ),
            StructField::new(
                "Goto",
                &Type::BOOL,
                r#"json:"goto,omitzero" schema_description:"Controls Effect goto references support." schema_default:"true""#,
            ),
            StructField::new(
                "Renames",
                &Type::BOOL,
                r#"json:"renames,omitzero" schema_description:"Controls Effect rename helpers." schema_default:"true""#,
            ),
            StructField::new(
                "IgnoreEffectSuggestionsInTscExitCode",
                &Type::BOOL,
                r#"json:"ignoreEffectSuggestionsInTscExitCode,omitzero" schema_description:"When true, suggestion diagnostics do not affect the tsc exit code." schema_default:"true""#,
            ),
            StructField::new(
                "IgnoreEffectWarningsInTscExitCode",
                &Type::BOOL,
                r#"json:"ignoreEffectWarningsInTscExitCode,omitzero" schema_description:"When true, warning diagnostics do not affect the tsc exit code." schema_default:"false""#,
            ),
            StructField::new(
                "IgnoreEffectErrorsInTscExitCode",
                &Type::BOOL,
                r#"json:"ignoreEffectErrorsInTscExitCode,omitzero" schema_description:"When true, error diagnostics do not affect the tsc exit code." schema_default:"false""#,
            ),
            StructField::new(
                "SkipDisabledOptimization",
                &Type::BOOL,
                r#"json:"skipDisabledOptimization,omitzero" schema_description:"When true, disabled diagnostics are still processed so directives can re-enable them." schema_default:"false""#,
            ),
            StructField::new(
                "MermaidProvider",
                &Type::STRING,
                r#"json:"mermaidProvider,omitzero" schema_description:"Mermaid rendering service for layer graph links. Accepts mermaid.live, mermaid.com, or a custom URL." schema_default:"\"mermaid.live\"""#,
            ),
            StructField::new(
                "NoExternal",
                &Type::BOOL,
                r#"json:"noExternal,omitzero" schema_description:"When true, suppresses external Mermaid links in hover output." schema_default:"false""#,
            ),
            StructField::new(
                "LayerGraphFollowDepth",
                &Type::INT,
                r#"json:"layerGraphFollowDepth,omitzero" schema_description:"How many levels deep the layer graph extraction follows symbol references." schema_default:"0" schema_minimum:"0""#,
            ),
            StructField::new(
                "Inlays",
                &Type::BOOL,
                r#"json:"inlays,omitzero" schema_description:"When true, suppresses redundant return-type inlay hints on supported Effect generator functions." schema_default:"false""#,
            ),
            StructField::new(
                "NamespaceImportPackages",
                &Type::SLICE,
                r#"json:"namespaceImportPackages,omitzero" schema_description:"Package names that should prefer namespace imports." schema_default:"[]" schema_items_type:"string""#,
            ),
            StructField::new(
                "BarrelImportPackages",
                &Type::SLICE,
                r#"json:"barrelImportPackages,omitzero" schema_description:"Package names that should prefer barrel named imports." schema_default:"[]" schema_items_type:"string""#,
            ),
            StructField::new(
                "ImportAliases",
                &Type::MAP,
                r#"json:"importAliases,omitzero" schema_description:"Package-level import aliases keyed by package name." schema_default:"{}" schema_additional_properties_type:"string""#,
            ),
            StructField::new(
                "TopLevelNamedReexports",
                &Type::STRING,
                r#"json:"topLevelNamedReexports,omitzero" schema_description:"Controls whether named reexports are followed at package top-level." schema_default:"\"ignore\"" schema_enum:"[\"ignore\",\"follow\"]""#,
            ),
            StructField::new(
                "KeyPatterns",
                &Type::SLICE,
                r#"json:"keyPatterns,omitzero" schema_description:"Configures key pattern formulas for the deterministicKeys rule.""#,
            ),
            StructField::new(
                "ExtendedKeyDetection",
                &Type::BOOL,
                r#"json:"extendedKeyDetection,omitzero" schema_description:"Enables matching constructors with @effect-identifier annotations." schema_default:"false""#,
            ),
            StructField::new(
                "PipeableMinArgCount",
                &Type::INT,
                r#"json:"pipeableMinArgCount,omitzero" schema_description:"Minimum number of contiguous pipeable transformations to trigger missedPipeableOpportunity." schema_default:"2" schema_minimum:"1""#,
            ),
            StructField::new(
                "AllowedUnstableApis",
                &Type::SLICE,
                r#"json:"allowedUnstableApis,omitzero" schema_description:"Unstable APIs allowed by package/module subtree or package/module#export name." schema_default:"[]" schema_items_type:"string""#,
            ),
            StructField::new(
                "AllowedExperimentalApis",
                &Type::SLICE,
                r#"json:"allowedExperimentalApis,omitzero" schema_description:"Experimental APIs allowed by package/module subtree or package/module#export name." schema_default:"[]" schema_items_type:"string""#,
            ),
            StructField::new(
                "AllowedDuplicatedPackages",
                &Type::SLICE,
                r#"json:"allowedDuplicatedPackages,omitzero" schema_description:"Package names allowed to have multiple versions without triggering duplicatePackage." schema_default:"[]" schema_items_type:"string""#,
            ),
            StructField::new(
                "EffectFn",
                &Type::SLICE,
                r#"json:"effectFn,omitzero" schema_description:"Controls which effectFnOpportunity quickfix variants are offered." schema_default:"[\"span\"]" schema_items_type:"string" schema_items_enum:"[\"span\",\"untraced\",\"no-span\",\"inferred-span\",\"suggested-span\"]" schema_unique_items:"true""#,
            ),
            StructField::new(
                "BrandedSchemaClasses",
                &Type::SLICE,
                r#"json:"brandedSchemaClasses,omitzero" schema_description:"Schema class kinds that get a nominal brand, used by schemaClassMissingBrand and the Schema class completions." schema_default:"[\"Class\",\"TaggedClass\",\"Error\",\"TaggedError\",\"Opaque\"]" schema_items_type:"string" schema_items_enum:"[\"Class\",\"TaggedClass\",\"Error\",\"TaggedError\",\"Opaque\"]" schema_unique_items:"true""#,
            ),
            StructField::new(
                "DiagnosticSeverity",
                &Type::MAP,
                r#"json:"diagnosticSeverity,omitzero" schema_description:"Maps rule names to severity levels. Use {} to enable diagnostics with rule defaults.""#,
            ),
            StructField::new(
                "Overrides",
                &Type::SLICE,
                r#"json:"overrides,omitzero" schema_description:"Ordered per-file diagnostic option overrides.""#,
            ),
        ],
    };
}

// go: etscore/options.go:136 ResolvedEffectPluginOptions de6a05c81787d5b0
// ResolvedEffectPluginOptions contains Effect plugin options that may vary per
// source file after applying matching overrides.
//
// Rust: never encoded or deep-compared, so nil and empty slices and maps are both empty.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ResolvedEffectPluginOptions {
    pub namespace_import_packages: Vec<String>,
    pub barrel_import_packages: Vec<String>,
    pub import_aliases: FxHashMap<String, String>,
    pub top_level_named_reexports: TopLevelNamedReexportsMode,
    pub key_patterns: Vec<KeyPattern>,
    pub extended_key_detection: bool,
    pub pipeable_min_arg_count: i64,
    pub allowed_duplicated_packages: Vec<String>,
    pub allowed_unstable_apis: Vec<String>,
    pub allowed_experimental_apis: Vec<String>,
    pub effect_fn: Vec<String>,
    // Rust: unlike the other slices, nil (all kinds) and empty (no kind) differ here.
    pub branded_schema_classes: Option<Vec<String>>,
}

// go: etscore/options.go:153 EffectPluginDiagnosticsOptions 542c095d5e144c5d
// EffectPluginDiagnosticsOptions is a compatibility alias for the resolved
// per-file diagnostic options.
pub type EffectPluginDiagnosticsOptions = ResolvedEffectPluginOptions;

// go: etscore/options.go:156 Override 21131281ffc972d1
// Override is a path-scoped partial override for diagnostic configuration.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Override {
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
    pub options: OverrideOptions,
}

// Rust-only: Go's `reflect.TypeOf(Override{})` as the schema generator in options_schema_test.go reads it:
// every field in Go order with its Go name, type kind and verbatim struct tag. Keep it in sync with the Go
// struct's tags.
impl Override {
    pub const TYPE: StructType = StructType {
        fields: &[
            StructField::new(
                "Include",
                &Type::SLICE,
                r#"json:"include,omitzero" schema_description:"Files included by this override." schema_items_type:"string""#,
            ),
            StructField::new(
                "Exclude",
                &Type::SLICE,
                r#"json:"exclude,omitzero" schema_description:"Files excluded from this override." schema_items_type:"string""#,
            ),
            StructField::new(
                "Options",
                &Type::STRUCT,
                r#"json:"options,omitzero" schema_description:"Diagnostic option overrides applied when this override matches.""#,
            ),
        ],
    };
}

// go: etscore/options.go:163 OverrideOptions b982dd74d3310c9e
// OverrideOptions is the subset of plugin options that can vary per file.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct OverrideOptions {
    pub diagnostic_severity: Option<FxHashMap<String, Severity>>,
    pub pipeable_min_arg_count: Option<i64>,
    pub key_patterns: Option<Vec<KeyPattern>>,
    pub extended_key_detection: Option<bool>,
    pub allowed_unstable_apis: Option<Vec<String>>,
    pub allowed_experimental_apis: Option<Vec<String>>,
    pub allowed_duplicated_packages: Option<Vec<String>>,
    pub effect_fn: Option<Vec<String>>,
    pub branded_schema_classes: Option<Vec<String>>,
    pub namespace_import_packages: Option<Vec<String>>,
    pub barrel_import_packages: Option<Vec<String>>,
    pub import_aliases: Option<FxHashMap<String, String>>,
    pub top_level_named_reexports: Option<TopLevelNamedReexportsMode>,
}

// Rust-only: Go's `reflect.TypeOf(OverrideOptions{})` as the schema generator in options_schema_test.go reads it:
// every field in Go order with its Go name, type kind and verbatim struct tag. Keep it in sync with the Go
// struct's tags.
impl OverrideOptions {
    pub const TYPE: StructType = StructType {
        fields: &[
            StructField::new(
                "DiagnosticSeverity",
                &Type::MAP,
                r#"json:"diagnosticSeverity,omitzero" schema_description:"Severity overrides applied when this override matches.""#,
            ),
            StructField::new(
                "PipeableMinArgCount",
                &Type::pointer_to(&Type::INT),
                r#"json:"pipeableMinArgCount,omitzero" schema_description:"Minimum number of contiguous pipeable transformations to trigger missedPipeableOpportunity." schema_minimum:"1""#,
            ),
            StructField::new(
                "KeyPatterns",
                &Type::pointer_to(&Type::SLICE),
                r#"json:"keyPatterns,omitzero" schema_description:"Scoped key pattern configuration for deterministicKeys.""#,
            ),
            StructField::new(
                "ExtendedKeyDetection",
                &Type::pointer_to(&Type::BOOL),
                r#"json:"extendedKeyDetection,omitzero" schema_description:"Scoped override for extended deterministic key detection.""#,
            ),
            StructField::new(
                "AllowedUnstableApis",
                &Type::pointer_to(&Type::SLICE),
                r#"json:"allowedUnstableApis,omitzero" schema_description:"Scoped allow-list for unstableApiUsage." schema_items_type:"string""#,
            ),
            StructField::new(
                "AllowedExperimentalApis",
                &Type::pointer_to(&Type::SLICE),
                r#"json:"allowedExperimentalApis,omitzero" schema_description:"Scoped allow-list for experimentalApiUsage." schema_items_type:"string""#,
            ),
            StructField::new(
                "AllowedDuplicatedPackages",
                &Type::pointer_to(&Type::SLICE),
                r#"json:"allowedDuplicatedPackages,omitzero" schema_description:"Scoped allow-list for duplicatePackage." schema_items_type:"string""#,
            ),
            StructField::new(
                "EffectFn",
                &Type::pointer_to(&Type::SLICE),
                r#"json:"effectFn,omitzero" schema_description:"Scoped override for effectFn quickfix variants." schema_items_type:"string" schema_items_enum:"[\"span\",\"untraced\",\"no-span\",\"inferred-span\",\"suggested-span\"]" schema_unique_items:"true""#,
            ),
            StructField::new(
                "BrandedSchemaClasses",
                &Type::pointer_to(&Type::SLICE),
                r#"json:"brandedSchemaClasses,omitzero" schema_description:"Scoped override for the Schema class kinds that get a nominal brand." schema_items_type:"string" schema_items_enum:"[\"Class\",\"TaggedClass\",\"Error\",\"TaggedError\",\"Opaque\"]" schema_unique_items:"true""#,
            ),
            StructField::new(
                "NamespaceImportPackages",
                &Type::pointer_to(&Type::SLICE),
                r#"json:"namespaceImportPackages,omitzero" schema_description:"Scoped package names that should prefer namespace imports." schema_items_type:"string""#,
            ),
            StructField::new(
                "BarrelImportPackages",
                &Type::pointer_to(&Type::SLICE),
                r#"json:"barrelImportPackages,omitzero" schema_description:"Scoped package names that should prefer barrel named imports." schema_items_type:"string""#,
            ),
            StructField::new(
                "ImportAliases",
                &Type::pointer_to(&Type::MAP),
                r#"json:"importAliases,omitzero" schema_description:"Scoped package-level import aliases keyed by package name." schema_additional_properties_type:"string""#,
            ),
            StructField::new(
                "TopLevelNamedReexports",
                &Type::pointer_to(&Type::STRING),
                r#"json:"topLevelNamedReexports,omitzero" schema_description:"Scoped override for whether named reexports are followed at package top-level." schema_enum:"[\"ignore\",\"follow\"]""#,
            ),
        ],
    };
}

// go: etscore/options.go:180 TopLevelNamedReexportsMode c512bad5d128f4dc
// TopLevelNamedReexportsMode configures top-level named reexport resolution behavior.
//
// Rust: a newtype over Go's `string` (any string can be stored; the getter maps unknown ones to "ignore").
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct TopLevelNamedReexportsMode(pub Cow<'static, str>);

// go: etscore/options.go:182 TopLevelNamedReexportsIgnore 2e09b463f8964365
impl TopLevelNamedReexportsMode {
    pub const Ignore: TopLevelNamedReexportsMode =
        TopLevelNamedReexportsMode(Cow::Borrowed("ignore"));
    pub const Follow: TopLevelNamedReexportsMode =
        TopLevelNamedReexportsMode(Cow::Borrowed("follow"));
}

// go: etscore/options.go:188 EffectFnSpan 0c444b8346bbd449
// EffectFn variant constants.
pub const EFFECT_FN_SPAN: &str = "span";
pub const EFFECT_FN_UNTRACED: &str = "untraced";
pub const EFFECT_FN_NO_SPAN: &str = "no-span";
pub const EFFECT_FN_INFERRED_SPAN: &str = "inferred-span";
pub const EFFECT_FN_SUGGESTED_SPAN: &str = "suggested-span";

// go: etscore/options.go:197 DefaultEffectFn af1e5c26f2386ddd
// DefaultEffectFn is the default effectFn configuration.
pub static DEFAULT_EFFECT_FN: LazyLock<Vec<String>> =
    LazyLock::new(|| vec![EFFECT_FN_SPAN.to_string()]);

// go: etscore/options.go:200 BrandedSchemaClassClass 1ebcba3249f30224
// BrandedSchemaClasses kind constants, named after the Schema API.
pub const BRANDED_SCHEMA_CLASS_CLASS: &str = "Class";
pub const BRANDED_SCHEMA_CLASS_TAGGED_CLASS: &str = "TaggedClass";
pub const BRANDED_SCHEMA_CLASS_ERROR: &str = "Error";
pub const BRANDED_SCHEMA_CLASS_TAGGED_ERROR: &str = "TaggedError";
pub const BRANDED_SCHEMA_CLASS_OPAQUE: &str = "Opaque";

// go: etscore/options.go:209 DefaultBrandedSchemaClasses d62888efd11afec0
// DefaultBrandedSchemaClasses is the default brandedSchemaClasses configuration.
pub static DEFAULT_BRANDED_SCHEMA_CLASSES: LazyLock<Vec<String>> = LazyLock::new(|| {
    vec![
        BRANDED_SCHEMA_CLASS_CLASS.to_string(),
        BRANDED_SCHEMA_CLASS_TAGGED_CLASS.to_string(),
        BRANDED_SCHEMA_CLASS_ERROR.to_string(),
        BRANDED_SCHEMA_CLASS_TAGGED_ERROR.to_string(),
        BRANDED_SCHEMA_CLASS_OPAQUE.to_string(),
    ]
});

// go: etscore/options.go:219 DiagnosticsEnabled 00c021c08faded94
// DiagnosticsEnabled reports whether Effect diagnostics are enabled for the
// parsed config in the current program.
pub fn diagnostics_enabled(config: Option<&EffectPluginOptions>) -> bool {
    config.is_some_and(|config| config.diagnostics && config.diagnostic_severity.is_some())
}

// Rust: Go's methods on a possibly nil `*ResolvedEffectPluginOptions` / `*EffectPluginOptions` take
// `e: Option<&Self>`.
impl ResolvedEffectPluginOptions {
    // go: etscore/options.go:224 ResolvedEffectPluginOptions.GetEffectFn 04838d419dcc1f78
    // GetEffectFn returns the configured effectFn variants, or the default ["span"] when unset/empty.
    pub fn get_effect_fn(e: Option<&Self>) -> &[String] {
        match e {
            Some(e) if !e.effect_fn.is_empty() => &e.effect_fn,
            _ => &DEFAULT_EFFECT_FN,
        }
    }

    // go: etscore/options.go:232 ResolvedEffectPluginOptions.EffectFnIncludes 780861476e12f039
    // EffectFnIncludes checks if a variant is in the configured (or default) effectFn list.
    pub fn effect_fn_includes(e: Option<&Self>, variant: &str) -> bool {
        for v in Self::get_effect_fn(e) {
            if v == variant {
                return true;
            }
        }
        false
    }

    // go: etscore/options.go:243 ResolvedEffectPluginOptions.GetBrandedSchemaClasses 40f0c93d2d3e792f
    // GetBrandedSchemaClasses returns the configured branded Schema class kinds, or all kinds when unset.
    // Unlike effectFn, an explicitly empty list is kept and means no kind is branded.
    pub fn get_branded_schema_classes(e: Option<&Self>) -> &[String] {
        match e.and_then(|e| e.branded_schema_classes.as_deref()) {
            Some(kinds) => kinds,
            None => &DEFAULT_BRANDED_SCHEMA_CLASSES,
        }
    }

    // go: etscore/options.go:251 ResolvedEffectPluginOptions.BrandedSchemaClassesIncludes 945a15e1d0a05886
    // BrandedSchemaClassesIncludes checks if a Schema class kind is in the configured (or default) list.
    pub fn branded_schema_classes_includes(e: Option<&Self>, kind: &str) -> bool {
        Self::get_branded_schema_classes(e).iter().any(|v| v == kind)
    }
}

impl EffectPluginOptions {
    // go: etscore/options.go:255 EffectPluginOptions.GetRefactorsEnabled 8f2054226fc872d6
    pub fn get_refactors_enabled(e: Option<&Self>) -> bool {
        let Some(e) = e else {
            return true;
        };
        e.refactors
    }

    // go: etscore/options.go:262 EffectPluginOptions.GetQuickinfoEnabled 0af8b32dfaa7a270
    pub fn get_quickinfo_enabled(e: Option<&Self>) -> bool {
        let Some(e) = e else {
            return true;
        };
        e.quickinfo
    }

    // go: etscore/options.go:269 EffectPluginOptions.GetCompletionsEnabled 41d4fe8e0014ea5f
    pub fn get_completions_enabled(e: Option<&Self>) -> bool {
        let Some(e) = e else {
            return true;
        };
        e.completions
    }

    // go: etscore/options.go:278 EffectPluginOptions.GetIncludeSuggestionsInTsc 3932720e2d92886c
    // GetIncludeSuggestionsInTsc returns whether suggestion diagnostics should appear in tsc output.
    // Returns true (include suggestions) when the receiver is nil.
    pub fn get_include_suggestions_in_tsc(e: Option<&Self>) -> bool {
        let Some(e) = e else {
            return true;
        };
        e.include_suggestions_in_tsc
    }
}

impl ResolvedEffectPluginOptions {
    // go: etscore/options.go:286 ResolvedEffectPluginOptions.GetPipeableMinArgCount bfafc61db8d45f85
    // GetPipeableMinArgCount returns the configured minimum pipeable arg count, or 2 if not set.
    pub fn get_pipeable_min_arg_count(e: Option<&Self>) -> i64 {
        if let Some(e) = e
            && e.pipeable_min_arg_count > 0
        {
            return e.pipeable_min_arg_count;
        }
        2
    }
}

// go: etscore/options.go:294 DefaultKeyPatterns 37a72324958d7c1a
// DefaultKeyPatterns is the default key patterns configuration.
pub static DEFAULT_KEY_PATTERNS: LazyLock<Vec<KeyPattern>> = LazyLock::new(|| {
    vec![
        KeyPattern {
            target: "service".to_string(),
            pattern: "default".to_string(),
            skip_leading_path: Some(vec!["src/".to_string()]),
        },
        KeyPattern {
            target: "custom".to_string(),
            pattern: "default".to_string(),
            skip_leading_path: Some(vec!["src/".to_string()]),
        },
    ]
});

impl ResolvedEffectPluginOptions {
    // go: etscore/options.go:300 ResolvedEffectPluginOptions.GetKeyPatterns 5e8dd05a0b93baac
    // GetKeyPatterns returns the configured key patterns or the defaults if none are configured.
    pub fn get_key_patterns(e: Option<&Self>) -> &[KeyPattern] {
        match e {
            Some(e) if !e.key_patterns.is_empty() => &e.key_patterns,
            _ => &DEFAULT_KEY_PATTERNS,
        }
    }
}

impl EffectPluginOptions {
    // go: etscore/options.go:311 EffectPluginOptions.GetMermaidBaseURL a26f8c658c2b112c
    // GetMermaidBaseURL resolves the MermaidProvider value to a full base URL.
    //   - "" or "mermaid.live" → "https://mermaid.live/edit#"
    //   - "mermaid.com" → "https://www.mermaidchart.com/play#"
    //   - Any other string → "<value>/edit#"
    pub fn get_mermaid_base_url(e: Option<&Self>) -> String {
        let mut provider = "";
        if let Some(e) = e {
            provider = &e.mermaid_provider;
        }
        match provider {
            "" | "mermaid.live" => "https://mermaid.live/edit#".to_string(),
            "mermaid.com" => "https://www.mermaidchart.com/play#".to_string(),
            _ => format!("{provider}/edit#"),
        }
    }

    // go: etscore/options.go:327 EffectPluginOptions.GetLayerGraphFollowDepth 441901b35e8af021
    // GetLayerGraphFollowDepth returns the configured layer graph follow depth, or 0 if not set.
    pub fn get_layer_graph_follow_depth(e: Option<&Self>) -> i64 {
        if let Some(e) = e
            && e.layer_graph_follow_depth > 0
        {
            return e.layer_graph_follow_depth;
        }
        0
    }
}

impl ResolvedEffectPluginOptions {
    // go: etscore/options.go:336 ResolvedEffectPluginOptions.GetAllowedDuplicatedPackages ad56d29328c1ca0f
    // GetAllowedDuplicatedPackages returns the list of package names allowed to
    // have multiple versions, or nil if none are configured.
    pub fn get_allowed_duplicated_packages(e: Option<&Self>) -> &[String] {
        let Some(e) = e else {
            return &[];
        };
        &e.allowed_duplicated_packages
    }
}

impl EffectPluginOptions {
    // go: etscore/options.go:345 EffectPluginOptions.GetNamespaceImportPackages fea530028fbacf69
    // GetNamespaceImportPackages returns normalized package names configured for namespace imports.
    // Defaults to an empty list when unset.
    pub fn get_namespace_import_packages(e: Option<&Self>) -> Vec<String> {
        let Some(e) = e else {
            return Vec::new();
        };
        normalize_package_list(e.namespace_import_packages.as_deref().unwrap_or_default())
    }

    // go: etscore/options.go:354 EffectPluginOptions.GetBarrelImportPackages 8ad905f15cbae4d9
    // GetBarrelImportPackages returns normalized package names configured for barrel imports.
    // Defaults to an empty list when unset.
    pub fn get_barrel_import_packages(e: Option<&Self>) -> Vec<String> {
        let Some(e) = e else {
            return Vec::new();
        };
        normalize_package_list(e.barrel_import_packages.as_deref().unwrap_or_default())
    }

    // go: etscore/options.go:363 EffectPluginOptions.GetImportAliases f3d8556a49b0ff68
    // GetImportAliases returns normalized import aliases keyed by lower-cased package names.
    // Defaults to an empty map when unset.
    pub fn get_import_aliases(e: Option<&Self>) -> FxHashMap<String, String> {
        let Some(e) = e else {
            return FxHashMap::default();
        };
        normalize_import_aliases(e.import_aliases.as_ref())
    }

    // go: etscore/options.go:372 EffectPluginOptions.GetTopLevelNamedReexports 850021b5e0662c70
    // GetTopLevelNamedReexports returns the normalized top-level named reexports mode.
    // Defaults to "ignore" when unset or invalid.
    pub fn get_top_level_named_reexports(e: Option<&Self>) -> TopLevelNamedReexportsMode {
        let Some(e) = e else {
            return TopLevelNamedReexportsMode::Ignore;
        };
        match TopLevelNamedReexportsMode(Cow::Owned(ets_go::strings::to_lower(
            &e.top_level_named_reexports.0,
        ))) {
            m if m == TopLevelNamedReexportsMode::Follow => TopLevelNamedReexportsMode::Follow,
            _ => TopLevelNamedReexportsMode::Ignore,
        }
    }
}

// go: etscore/options.go:384 normalizePackageList 7f0be0028ca0df54
pub(crate) fn normalize_package_list(packages: &[String]) -> Vec<String> {
    if packages.is_empty() {
        return Vec::new();
    }
    let mut normalized = Vec::with_capacity(packages.len());
    for pkg in packages {
        let pkg = normalize_package_name(pkg);
        if !pkg.is_empty() {
            normalized.push(pkg);
        }
    }
    normalized
}

// go: etscore/options.go:397 normalizeImportAliases 05683580c425e74d
pub(crate) fn normalize_import_aliases(
    aliases: Option<&FxHashMap<String, String>>,
) -> FxHashMap<String, String> {
    let Some(aliases) = aliases.filter(|aliases| !aliases.is_empty()) else {
        return FxHashMap::default();
    };
    let mut normalized =
        FxHashMap::with_capacity_and_hasher(aliases.len(), rustc_hash::FxBuildHasher);
    for (pkg, alias) in aliases {
        let pkg = normalize_package_name(pkg);
        if !pkg.is_empty() {
            normalized.insert(pkg, alias.clone());
        }
    }
    normalized
}

// go: etscore/options.go:410 normalizePackageName 1e2e7bee544d0618
pub(crate) fn normalize_package_name(pkg: &str) -> String {
    ets_go::strings::to_lower(ets_go::strings::trim_space(pkg))
}
