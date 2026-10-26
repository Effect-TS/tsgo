use std::hash::Hash;
use std::sync::Arc;

use ets_importstyle::{BarrelReader, ProgramFiles, new_program_barrel_reader};
use ets_shim::ast::{Node, SourceFile, Symbol};
use ets_shim::checker::{Checker, Signature, Type};
use ets_shim::compiler::Program;
use ets_shim::core::P;
use ets_shim::module::packagejson::PackageJson;
use parking_lot::{Mutex, MutexGuard};
use rustc_hash::FxHashMap;

use crate::*;

// go: internal/typeparser/type_parser.go:13 TypeParser
// TypeParser groups checker-backed typeparser operations behind a shared
// checker/program pair so callers do not need to thread them through each call.
//
// Rust: tsrs's checker methods take `&mut Checker`, so the parser borrows the checker mutably for its lifetime and
// its methods take `&mut self`. `links` is the handle of the checker's `c.effect_links` (see `effect_links_of`). Go's `checker.Program` is the concrete `compiler.Program` (the checker's own
// program), so the interface assertions of `packageJsonProgram` and `sourceFileProgram` always succeed.
pub struct TypeParser<'c> {
    pub(crate) program: &'static Program,
    pub(crate) checker: &'c mut Checker,
    pub(crate) links: Arc<Mutex<EffectLinks>>,
}

impl TypeParser<'_> {
    /// Rust-only: the checker the parser runs on (Go reads the `checker` field from the same package).
    pub fn checker(&mut self) -> &mut Checker {
        self.checker
    }

    /// Rust-only: the program the parser was built for.
    pub fn program(&self) -> &'static Program {
        self.program
    }

    /// Rust-only: the checker's links. Never hold the guard across a call back into the parser: computations
    /// read and fill the links recursively (Go's single goroutine per checker needs no lock).
    pub(crate) fn links(&self) -> MutexGuard<'_, EffectLinks> {
        self.links.lock()
    }
}

// go: internal/typeparser/type_parser.go:19 denseNodeStore
pub type DenseNodeStore<V> = FxHashMap<P<Node>, V>;

// go: internal/typeparser/type_parser.go:23 EffectLinks
// EffectLinks holds per-checker cached type-parser results.
// One instance is lazily created per Checker and cached on TypeParser.
//
// Rust: Go's `checker.NodeLinkStore`, `core.LinkStore` and `checker.SymbolLinkStore` are maps keyed by the handle
// (pointer identity, as in Go). Go's pointer-valued results (`*Effect`, `*ParsedPipeCallResult`, ...) are `Option`
// of the value, behind an `Arc` where the result holds lazies that its copies share.
#[derive(Default)]
pub struct EffectLinks {
    pub type_at_location: DenseNodeStore<Option<P<Type>>>,
    pub reference_symbol: DenseNodeStore<Option<P<Symbol>>>,
    pub effect_context_flags: DenseNodeStore<EffectContextFlags>,
    pub effect_yield_generator_function: DenseNodeStore<Option<P<Node>>>,
    pub data_first_or_last_call: DenseNodeStore<Option<Arc<ParsedDataFirstOrLastCall>>>,
    // Rust: written only by RecordCheckedExpressionType, which the checker calls through the port of effect-tsgo's
    // TypeScript patch 033 (_patches/tsrs/033-checker-expression-checked-callback.patch) once it is registered.
    pub checked_expression_type: DenseNodeStore<Option<P<Type>>>,
    pub(crate) checked_expression_types_paused: bool,
    // checkedExpressionTypeSourceFile is the last source file found type
    // checked by checkedExpressionType.
    pub(crate) checked_expression_type_source_file: Option<P<SourceFile>>,

    pub object_literal_contextual_type: FxHashMap<P<Node>, Option<P<Type>>>,
    pub object_literal_constraint_contextual_type: FxHashMap<P<Node>, Option<P<Type>>>,
    pub object_literal_bare_constraint_contextual_type: FxHashMap<P<Node>, Option<P<Type>>>,

    pub effect_type: FxHashMap<P<Type>, Option<Effect>>,
    pub stream_type: FxHashMap<P<Type>, Option<Effect>>,
    pub strict_effect_type: FxHashMap<P<Type>, Option<Effect>>,
    pub effect_subtype: FxHashMap<P<Type>, Option<Effect>>,
    pub fiber_type: FxHashMap<P<Type>, Option<Effect>>,
    pub effect_yieldable_type: FxHashMap<P<Type>, Option<Effect>>,
    pub has_effect_type_id: FxHashMap<P<Type>, bool>,
    pub layer_type: FxHashMap<P<Type>, Option<Layer>>,
    pub service_type: FxHashMap<P<Type>, Option<Service>>,
    pub context_tag: FxHashMap<P<Type>, Option<Service>>,
    pub effect_schema_types: FxHashMap<P<Type>, Option<SchemaTypes>>,
    pub is_scope_type: FxHashMap<P<Type>, bool>,
    pub is_pipeable_type: FxHashMap<P<Type>, bool>,
    pub global_interface_has_pipe: FxHashMap<String, bool>,
    pub promise_type: FxHashMap<P<Type>, Option<P<Type>>>,
    pub is_global_error_type: FxHashMap<P<Type>, bool>,
    pub is_yieldable_error_type: FxHashMap<P<Type>, bool>,
    pub(crate) module_export_identities: FxHashMap<P<Symbol>, Arc<[ModuleExportIdentity]>>,
    pub(crate) module_export_filter: FxHashMap<P<Symbol>, ModuleExportFilter>,
    pub(crate) module_export_names: FxHashMap<P<SourceFile>, ModuleExportNames>,
    pub(crate) module_file_filter: FxHashMap<P<SourceFile>, ModuleExportFilter>,
    pub(crate) pipeable_signature_shape: FxHashMap<PipeableSignatureShapeCacheKey, bool>,
    // Rust: the registries are shared handles, so a reader holds one while it calls back into the checker.
    pub(crate) native_array_registry: Option<Arc<NativeArrayRegistry>>,
    pub(crate) barrel_reader: Option<Arc<Mutex<BarrelReader>>>,
    pub(crate) native_array_member: FxHashMap<P<Symbol>, NativeArrayMember>,
    pub(crate) native_object_registry: Option<Arc<NativeObjectRegistry>>,
    pub(crate) native_object_member: FxHashMap<P<Symbol>, NativeObjectMember>,
    pub(crate) native_string_registry: Option<Arc<NativeStringRegistry>>,
    pub(crate) native_string_member: FxHashMap<P<Symbol>, NativeStringMember>,
    // API-stability caches. Declared lookups persist per checker: the declared
    // stability of a symbol, of a raw signature overload and of one
    // declaration, shared with the unstableApiUsage/experimentalApiUsage rules.
    // Declared lookups never trigger computed analysis. A computed stability
    // surface lives in an analysis-local ApiStabilitySession, and every
    // complete, settled, context-free concrete type or signature surface is
    // additionally published here as an immutable snapshot so a later export,
    // file or TypeParser over the same checker composes it instead of
    // recomputing. The snapshot excludes the component's own declared tag,
    // which stays in the declared caches and is re-added by each consumer; a
    // substitution-context or incomplete/blocked result is never published and
    // stays analysis-local. Ceilings, locations and diagnostics are never
    // cached.
    pub api_stability_declared_symbol: FxHashMap<P<Symbol>, ApiStabilityDeclaration>,
    pub api_stability_declared_signature: FxHashMap<P<Signature>, ApiStabilityDeclaration>,
    pub api_stability_declared_declaration: FxHashMap<P<Node>, ApiStabilityDeclaration>,
    pub(crate) api_stability_surface_type:
        FxHashMap<ApiStabilitySurfaceTypeKey, ApiStabilitySharedSurface>,
    pub(crate) api_stability_surface_signature: FxHashMap<P<Signature>, ApiStabilitySharedSurface>,

    pub extends_context_tag: FxHashMap<P<Node>, Option<ContextTagResult>>,
    pub extends_data_error: FxHashMap<P<Node>, Option<DataErrorResult>>,
    pub extends_data_tagged_class: FxHashMap<P<Node>, Option<DataTaggedResult>>,
    pub extends_data_tagged_error: FxHashMap<P<Node>, Option<DataTaggedResult>>,
    pub extends_effect_model_class: FxHashMap<P<Node>, Option<EffectModelClassResult>>,
    pub extends_effect_service: FxHashMap<P<Node>, Option<EffectServiceResult>>,
    pub extends_effect_tag: FxHashMap<P<Node>, Option<EffectTagResult>>,
    pub extends_schema_class: FxHashMap<P<Node>, Option<SchemaClassResult>>,
    pub extends_schema_error: FxHashMap<P<Node>, Option<SchemaClassResult>>,
    pub extends_schema_opaque: FxHashMap<P<Node>, Option<SchemaOpaqueResult>>,
    pub extends_schema_request_class: FxHashMap<P<Node>, Option<SchemaClassResult>>,
    pub extends_schema_tagged_class: FxHashMap<P<Node>, Option<SchemaTaggedResult>>,
    pub extends_schema_tagged_error: FxHashMap<P<Node>, Option<SchemaTaggedResult>>,
    pub extends_schema_tagged_request: FxHashMap<P<Node>, Option<SchemaTaggedResult>>,
    pub extends_service_map_service: FxHashMap<P<Node>, Option<ServiceMapServiceResult>>,
    pub extends_effect_sql_model_class: FxHashMap<P<Node>, Option<SqlModelClassResult>>,

    pub effect_gen_call: FxHashMap<P<Node>, Option<Arc<EffectGenCallResult>>>,
    pub effect_fn_call: FxHashMap<P<Node>, Option<Arc<EffectFnCallResult>>>,
    pub parse_effect_fn_opportunity: FxHashMap<P<Node>, Option<Arc<EffectFnOpportunityResult>>>,
    pub parse_pipe_call: FxHashMap<P<Node>, Option<Arc<ParsedPipeCallResult>>>,
    pub execution_flow: FxHashMap<P<SourceFile>, Option<Arc<ExecutionFlow>>>,

    pub(crate) discover_packages_computed: bool,
    pub(crate) discover_packages_value: Option<Arc<[DiscoveredPackage]>>,
    pub(crate) detect_effect_version_computed: bool,
    pub(crate) detect_effect_version_value: EffectMajorVersion,
    pub package_json_for_source_file: FxHashMap<P<SourceFile>, Option<P<PackageJson>>>,
    pub effect_context_analyzed: FxHashMap<P<SourceFile>, bool>,
    pub expected_and_real_types: FxHashMap<P<SourceFile>, Option<Arc<[ExpectedAndRealType]>>>,
    pub piping_flows_with_effect_fn: FxHashMap<P<SourceFile>, Option<Arc<[PipingFlow]>>>,
    pub piping_flows_without_effect_fn: FxHashMap<P<SourceFile>, Option<Arc<[PipingFlow]>>>,
}

// go: internal/typeparser/type_parser.go:124 Cached
// Cached checks the store for an existing value. On miss, it calls compute,
// stores the result, and returns it. This correctly caches zero/nil values
// as valid negative results.
//
// Rust: the store is selected from the parser's links (Go passes a pointer into them); the links are not locked
// while compute runs, so it may read and fill the same store.
#[expect(clippy::implicit_hasher, reason = "the stores are the links' own maps")]
pub fn cached<'c, K: Eq + Hash + Clone, V: Clone>(
    tp: &mut TypeParser<'c>,
    store: fn(&mut EffectLinks) -> &mut FxHashMap<K, V>,
    key: K,
    compute: impl FnOnce(&mut TypeParser<'c>) -> V,
) -> V {
    if let Some(value) = store(&mut tp.links()).get(&key) {
        return value.clone();
    }
    let value = compute(tp);
    store(&mut tp.links()).insert(key, value.clone());
    value
}

// go: internal/typeparser/type_parser.go:134 CachedNode
// CachedNode is Cached for node-keyed stores.
pub fn cached_node<'c, V: Clone>(
    tp: &mut TypeParser<'c>,
    store: fn(&mut EffectLinks) -> &mut DenseNodeStore<V>,
    node: P<Node>,
    compute: impl FnOnce(&mut TypeParser<'c>) -> V,
) -> V {
    if let Some(value) = store(&mut tp.links()).get(&node) {
        return value.clone();
    }
    let value = compute(tp);
    store(&mut tp.links()).insert(node, value.clone());
    value
}

impl TypeParser<'_> {
    // go: internal/typeparser/type_parser.go:146 TypeParser.BarrelReader
    // BarrelReader returns the checker's barrel cache, or nil when the program
    // cannot read files.
    //
    // Rust: Go's `importstyle.ProgramFiles` assertion always succeeds for the compiler program (see `TypeParser`),
    // so the cache is always there. Go hands out the cached pointer; here the cache lives behind the links' lock,
    // so the parser hands out a shared handle with its own lock (Go: not safe for concurrent use).
    pub fn barrel_reader(&self) -> Arc<Mutex<BarrelReader>> {
        let program: &'static dyn ProgramFiles = self.program;
        let mut links = self.links();
        let reader = links
            .barrel_reader
            .get_or_insert_with(|| Arc::new(Mutex::new(new_program_barrel_reader(program))));
        Arc::clone(reader)
    }
}

// go: internal/typeparser/type_parser.go:157 NewTypeParser
// NewTypeParser builds a checker-backed TypeParser.
//
// Rust: Go panics on a nil program or checker; references cannot be nil.
pub fn new_type_parser<'c>(p: &'static Program, c: &'c mut Checker) -> TypeParser<'c> {
    let links = effect_links_of(c);
    TypeParser { program: p, checker: c, links }
}

// go: internal/typeparser/type_parser.go:167 effectLinksOf
// Rust: `c.effect_links` is effect-tsgo's TypeScript patch 023 (`_patches/tsrs/023-checker-effect-links.patch`). It
// holds the links behind an `Arc` (Go's `*EffectLinks`) that the parser shares while it borrows the checker, and
// they are freed with the checker. The mutex is the checker's own (Go: none, one goroutine per checker).
pub(crate) fn effect_links_of(c: &mut Checker) -> Arc<Mutex<EffectLinks>> {
    Arc::clone(effect_links_ref(c))
}

/// Rust-only: `effect_links_of` without cloning the handle, for `record_checked_expression_type` (it runs on every
/// checked expression).
pub(crate) fn effect_links_ref(c: &mut Checker) -> &Arc<Mutex<EffectLinks>> {
    c.effect_links
        .get_or_insert_with(|| Box::new(Arc::new(Mutex::new(EffectLinks::default()))))
        .downcast_ref::<Arc<Mutex<EffectLinks>>>()
        .expect("Checker::effect_links holds the type parser's links")
}
