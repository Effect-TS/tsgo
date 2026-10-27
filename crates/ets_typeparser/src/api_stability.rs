use std::rc::Rc;

use ets_readonly::Slice;
use ets_shim::ast::{
    ClassLikeBase, INTERNAL_SYMBOL_NAME_CALL, INTERNAL_SYMBOL_NAME_CONSTRUCTOR,
    INTERNAL_SYMBOL_NAME_NEW, INTERNAL_SYMBOL_NAME_PREFIX, Kind, ModifierFlags, Node, NodeFlags,
    NodeList, Symbol, SymbolFlags, SymbolTable,
};
use ets_shim::checker::{
    Checker, IndexInfo, ObjectFlags, Signature, SignatureFlags, SignatureKind, Type, TypeAlias,
    TypeFlags, TypeMapper,
};
use ets_shim::core::P;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::*;

// go: internal/typeparser/api_stability.go:18 ApiStabilityLevel 54453667d23bc6d2
// ApiStabilityLevel orders the Effect stability tiers. A higher level is less
// stable, so a public surface may only expose dependencies whose level is at
// most the level declared for the export that owns the surface.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum ApiStabilityLevel {
    // go: internal/typeparser/api_stability.go:20 ApiStabilityStable 4d7882dd11a562a2
    #[default]
    Stable,
    Unstable,
    Experimental,
}

// go: internal/typeparser/api_stability.go:32 ApiStabilityLevelTag b7a37b136a31656e
// ApiStabilityLevelTag returns the `@stability` tag spelling for a level as
// rendered by the usage diagnostics. The stable tier is spelled as the empty
// string because it is the implicit default; an explicit `@stability stable`
// tag is still parsed (see stabilityOfDeclarationTag) and keeps its declaration
// as provenance, so the declared accessors distinguish it from an absent tag
// even though this spelling helper returns the same empty string for both.
pub fn api_stability_level_tag(level: ApiStabilityLevel) -> &'static str {
    match level {
        ApiStabilityLevel::Unstable => "unstable",
        ApiStabilityLevel::Experimental => "experimental",
        ApiStabilityLevel::Stable => "",
    }
}

// go: internal/typeparser/api_stability.go:45 ApiStabilityLevelFromTag cfcbb213dfa3a3aa
// ApiStabilityLevelFromTag maps a `@stability` tag spelling to a level. An
// unrecognized or empty tag is stable.
pub fn api_stability_level_from_tag(tag: &str) -> ApiStabilityLevel {
    api_stability_rank(tag)
}

// go: internal/typeparser/api_stability.go:54 ApiStabilityDeclaration f6b5ed37409084b7
// ApiStabilityDeclaration is a declared stability tier together with the
// declaration that carries the tag. The declaration is retained for callers
// that need provenance (allow-list naming and diagnostic locations). An
// explicit `@stability stable` tag keeps its declaration, distinct from an
// untagged default-stable symbol, which has no declaration.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ApiStabilityDeclaration {
    pub(crate) level: ApiStabilityLevel,
    pub(crate) declaration: Option<P<Node>>,
}

impl ApiStabilityDeclaration {
    // go: internal/typeparser/api_stability.go:59 ApiStabilityDeclaration.Level 2b221600faad4a70
    pub fn level(self) -> ApiStabilityLevel {
        self.level
    }

    // go: internal/typeparser/api_stability.go:61 ApiStabilityDeclaration.Declaration 7e023a52ae52bd81
    pub fn declaration(self) -> Option<P<Node>> {
        self.declaration
    }
}

// go: internal/typeparser/api_stability.go:67 ApiStabilityDependency e6a0231035bd28a2
// ApiStabilityDependency names one exposed component of a public API surface
// whose declared stability is above stable. The offender is either a symbol
// (properties, methods, referenced types) or a signature declaration; the
// declaration carries the `@stability` provenance used for reporting.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ApiStabilityDependency {
    pub(crate) symbol: Option<P<Symbol>>,
    pub(crate) signature: Option<P<Signature>>,
    pub(crate) declaration: Option<P<Node>>,
    pub(crate) level: ApiStabilityLevel,
}

impl ApiStabilityDependency {
    // go: internal/typeparser/api_stability.go:74 ApiStabilityDependency.Symbol b2128b5a39505c85
    pub fn symbol(&self) -> Option<P<Symbol>> {
        self.symbol
    }

    // go: internal/typeparser/api_stability.go:76 ApiStabilityDependency.Signature 75e5d0362a2d0094
    pub fn signature(&self) -> Option<P<Signature>> {
        self.signature
    }

    // go: internal/typeparser/api_stability.go:78 ApiStabilityDependency.Declaration 86c955ad77f38908
    pub fn declaration(&self) -> Option<P<Node>> {
        self.declaration
    }

    // go: internal/typeparser/api_stability.go:80 ApiStabilityDependency.Level d3a2ddfa36de02bb
    pub fn level(&self) -> ApiStabilityLevel {
        self.level
    }
}

// go: internal/typeparser/api_stability.go:82 NewApiStabilityDependency d86060dc2e25d202
pub fn new_api_stability_dependency(
    symbol: Option<P<Symbol>>,
    signature: Option<P<Signature>>,
    declaration: Option<P<Node>>,
    level: ApiStabilityLevel,
) -> ApiStabilityDependency {
    ApiStabilityDependency { symbol, signature, declaration, level }
}

// go: internal/typeparser/api_stability.go:104 ApiStabilityUsed 6db0e4a1fc0a6f30
// ApiStabilityUsed is the computed stability of the components a surface
// exposes: the minimum guaranteed stability (the least stable dependency found,
// stable when none was found) and the offending components themselves.
//
// The surface of a root component (a symbol, a represented type or a signature)
// excludes the root's own declared tag. Declared stability is a separate
// lookup: DeclaredApiStabilityOfSymbol and DeclaredApiStabilityOfSignature
// return the component's own tag and its declaration, are cached per checker,
// and never trigger a computed surface. The same component reached as a child
// of another root contributes its own tag to that parent surface; only the
// root conversion removes it, so the exclusion is a result semantic and never
// a traversal stop.
//
// Incomplete reports that the analysis could not establish every component of
// the surface (a component is unavailable or a guarded read was refused), so
// the dependency list may be missing findings and must not be treated as a
// stable result; the incomplete result is discarded with its session and a
// later export or call retries.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ApiStabilityUsed {
    pub(crate) minimum: ApiStabilityLevel,
    pub(crate) dependencies: Vec<ApiStabilityDependency>,
    pub(crate) incomplete: bool,
}

impl ApiStabilityUsed {
    // go: internal/typeparser/api_stability.go:110 ApiStabilityUsed.Minimum bb227e048d15395c
    pub fn minimum(&self) -> ApiStabilityLevel {
        self.minimum
    }

    // go: internal/typeparser/api_stability.go:112 ApiStabilityUsed.Dependencies b55dbec187ebe98b
    pub fn dependencies(&self) -> Slice<'_, ApiStabilityDependency> {
        ets_readonly::new(self.dependencies.as_slice())
    }

    // go: internal/typeparser/api_stability.go:116 ApiStabilityUsed.Incomplete f2a350166748908e
    pub fn incomplete(&self) -> bool {
        self.incomplete
    }
}

// go: internal/typeparser/api_stability.go:123 apiStabilityInspection dc6b95018278a9f3
// apiStabilityInspection distinguishes how a represented type is inspected. A
// named class or interface reached as a dependency stays shallow: its symbol
// and its public call/construct signatures are inspected, but its members are
// not recursively expanded. The direct surface of an export is expanded: own
// and inherited members, index signatures and signatures are all inspected.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum ApiStabilityInspection {
    // go: internal/typeparser/api_stability.go:125 apiStabilityShallow f7cefa51ab12a883
    Shallow,
    Expand,
}

// go: internal/typeparser/api_stability.go:136 apiStabilityFindingKey 9fe0b763be3172b3
// apiStabilityFindingKey identifies one offender inside a surface. A symbol
// finding and a signature finding are distinct, so a tagged overload of a
// stable symbol is reported even though the symbol itself is stable. A
// declaration finding names an offender that has no materialized symbol or
// signature (a tagged member of a declaration annotation), so its declaration
// is the identity.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub(crate) struct ApiStabilityFindingKey {
    pub(crate) symbol: Option<P<Symbol>>,
    pub(crate) signature: Option<P<Signature>>,
    pub(crate) declaration: Option<P<Node>>,
}

// go: internal/typeparser/api_stability.go:142 apiStabilityFinding 5ff5278895d3adba
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct ApiStabilityFinding {
    pub(crate) level: ApiStabilityLevel,
    pub(crate) declaration: Option<P<Node>>,
}

/// Rust-only: Go's struct conversion `apiStabilityFinding(declared)` (identical field sets).
impl From<ApiStabilityDeclaration> for ApiStabilityFinding {
    fn from(declared: ApiStabilityDeclaration) -> Self {
        ApiStabilityFinding { level: declared.level, declaration: declared.declaration }
    }
}

/// Rust-only: the findings map of a surface. Go shares one map between the copies of a surface value; the Rust
/// surface shares it through an `Rc` and copies it before the first write (`Rc::make_mut`), which is the
/// "allocate a fresh map before writing" Go does by hand.
pub(crate) type ApiStabilityFindings = FxHashMap<ApiStabilityFindingKey, ApiStabilityFinding>;

// go: internal/typeparser/api_stability.go:167 apiStabilitySurface 785c5351b29b416d
// apiStabilitySurface is a memoized set of stability findings. complete is
// false when the result was cut by an in-progress cycle or by a component the
// checker has not materialized; such a result is never treated as a finished
// memo entry. cutCycle marks a result that was cut only by cycle back-edges,
// which the settle fixpoint can establish as complete; blocked marks a result
// whose components are unavailable or whose compiler outcome must not be
// guessed, and which is never promoted.
//
// observedEpoch is analysis-local settlement bookkeeping. It records the
// analysis materialization epoch at which this result was last collected or
// recollected, so the settle fixpoint can tell whether a blocked result may
// have become collectable since (a later analysis read may have materialized a
// component the guard had refused). It is never part of equality or merge
// semantics: complete results ignore it, and blocked results are never
// promoted.
//
// findings stays nil until the first finding is added, and every write goes
// through add. A surface stored in a session map is never mutated afterwards:
// growApiStabilitySurface shares the stored map when a recollection adds
// nothing and allocates a fresh one before writing otherwise.
#[derive(Clone, Debug, Default)]
pub(crate) struct ApiStabilitySurface {
    pub(crate) findings: Rc<ApiStabilityFindings>,
    pub(crate) complete: bool,
    pub(crate) cut_cycle: bool,
    pub(crate) blocked: bool,

    pub(crate) observed_epoch: i64,
}

// go: internal/typeparser/api_stability.go:176 newApiStabilitySurface 473d86f701cd8431
pub(crate) fn new_api_stability_surface() -> ApiStabilitySurface {
    ApiStabilitySurface { complete: true, ..Default::default() }
}

// go: internal/typeparser/api_stability.go:184 cycleCutSurface ebf9ca49f6fdd989
// cycleCutSurface marks a result cut by an in-progress cycle. The settle
// fixpoint can promote it to complete once findings stop changing, because the
// back-edge's own direct findings are collected when the cut target is first
// entered.
pub(crate) fn cycle_cut_surface() -> ApiStabilitySurface {
    ApiStabilitySurface { cut_cycle: true, ..Default::default() }
}

// go: internal/typeparser/api_stability.go:191 blockedSurface 44d3306a09a12d02
// blockedSurface marks a result whose components are not materialized or whose
// represented outcome is unavailable. It is never promoted to complete, so a
// later query or session against a checked checker recomputes it.
pub(crate) fn blocked_surface() -> ApiStabilitySurface {
    ApiStabilitySurface { blocked: true, ..Default::default() }
}

impl ApiStabilitySurface {
    // go: internal/typeparser/api_stability.go:197 apiStabilitySurface.cutByCycle 94a01b173c9cbccb
    // cutByCycle marks a surface that an in-progress cycle back-edge cut short. The
    // settle fixpoint can establish it as complete once findings stop changing.
    pub(crate) fn cut_by_cycle(&mut self) {
        self.complete = false;
        self.cut_cycle = true;
    }

    // go: internal/typeparser/api_stability.go:205 apiStabilitySurface.block 4d3eea208a1a3b2f
    // block marks a surface whose remaining components are unavailable. A blocked
    // surface is never promoted to complete, so a later query or session against a
    // materialized checker recomputes it.
    pub(crate) fn block(&mut self) {
        self.complete = false;
        self.blocked = true;
    }

    // go: internal/typeparser/api_stability.go:210 apiStabilitySurface.merge 419bb47104a93e5f
    pub(crate) fn merge(&mut self, other: &ApiStabilitySurface) {
        if !other.complete {
            self.complete = false;
            self.cut_cycle = self.cut_cycle || other.cut_cycle;
            self.blocked = self.blocked || other.blocked;
        }
        for (&key, &finding) in other.findings.iter() {
            self.add(key, finding);
        }
    }

    // go: internal/typeparser/api_stability.go:223 apiStabilitySurface.add ddf70b09418ec248
    // add records a finding unless the surface already holds the key at an equal
    // or higher level, allocating the findings map on first use.
    pub(crate) fn add(&mut self, key: ApiStabilityFindingKey, finding: ApiStabilityFinding) {
        if let Some(existing) = self.findings.get(&key)
            && finding.level <= existing.level
        {
            return;
        }
        Rc::make_mut(&mut self.findings).insert(key, finding);
    }

    // go: internal/typeparser/api_stability.go:236 apiStabilitySurface.equals de25a4f4bf7f8cda
    // equals compares the semantic content of two surfaces. observedEpoch is
    // deliberately excluded: it is settlement bookkeeping, so updating it after a
    // recollection is not a fixpoint change.
    pub(crate) fn equals(&self, other: &ApiStabilitySurface) -> bool {
        self.complete == other.complete
            && self.cut_cycle == other.cut_cycle
            && self.blocked == other.blocked
            && *self.findings == *other.findings
    }
}

/// Rust-only: the identity of an interned `apiStabilityCarrier` (Go compares the interned `*apiStabilityCarrier`
/// pointers). The analysis's carrier table hands out one per distinct chain; the table and every holder are cleared
/// together (`reset`), so an identity never outlives its table.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct ApiStabilityCarrierRef(u32);

// go: internal/typeparser/api_stability.go:249 apiStabilityCarrier e9b44fd00b27c332
// apiStabilityCarrier is the interned, concrete enclosing identity of an
// internal raw-component traversal. It is built only from concrete compiler
// objects: a reference or instantiated owner type, a concrete signature, and
// the concrete argument types of a represented generic application. Interning
// in the analysis-local carrier table makes equal chains share one node
// pointer, so session and cycle keys compare by concrete identity instead of a
// serialized substitution context. No context string and no raw-target-only key
// ever participates.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub(crate) struct ApiStabilityCarrier {
    pub(crate) parent: Option<ApiStabilityCarrierRef>,
    pub(crate) owner: Option<P<Type>>,
    pub(crate) signature: Option<P<Signature>>,

    // argument marks a carrier link that contributes one concrete type
    // argument. marker keeps the link distinct even when the argument itself is
    // unrepresented (nil), so two different applications never share a carrier
    // by accident.
    pub(crate) argument: Option<P<Type>>,
    pub(crate) marker: bool,
}

// go: internal/typeparser/api_stability.go:268 apiStabilitySubstFrame 1b78ad19b73de721
// apiStabilitySubstFrame is one link in a composed substitution context. A
// frame maps a signature's type parameters, a reference's type parameters, or
// a compiler-provided mapper to represented types. The chain keeps nested
// substitutions intact: a base type's argument can itself refer to the
// enclosing type's parameters. carrier is the interned concrete identity of
// the whole chain.
//
// Rust: frames are immutable once built and shared by the chains and the frame tables, so they are `Rc`s.
#[derive(Debug, Default)]
pub(crate) struct ApiStabilitySubstFrame {
    pub(crate) parent: Option<Rc<ApiStabilitySubstFrame>>,
    pub(crate) mapper: Option<P<TypeMapper>>,
    pub(crate) signature: Option<P<Signature>>,

    // parameters and arguments bind a class or interface reference's type
    // parameters to the reference's represented type arguments. A nil argument
    // replaces the parameter without a represented value: dependencies of the
    // replaced parameter are unknown, so the traversal blocks instead of
    // falling back to the declaration's constraint or default.
    pub(crate) parameters: Option<&'static [P<Type>]>,
    pub(crate) arguments: Vec<Option<P<Type>>>,

    pub(crate) carrier: Option<ApiStabilityCarrierRef>,
}

// go: internal/typeparser/api_stability.go:284 apiStabilitySubstitution debc4406680da8b9
#[derive(Clone, Debug, Default)]
pub(crate) struct ApiStabilitySubstitution {
    pub(crate) frame: Option<Rc<ApiStabilitySubstFrame>>,
}

impl ApiStabilitySubstitution {
    // go: internal/typeparser/api_stability.go:290 apiStabilitySubstitution.carrier 2050be8c84602311
    // carrier returns the concrete enclosing identity of the whole substitution
    // chain, or nil when the traversal started at a concrete top-level object.
    pub(crate) fn carrier(&self) -> Option<ApiStabilityCarrierRef> {
        self.frame.as_ref()?.carrier
    }
}

impl ApiStabilityAnalysis {
    // go: internal/typeparser/api_stability.go:301 apiStabilityAnalysis.aliasArgumentsAreSelf 2f117dfd4cb45604
    // aliasArgumentsAreSelf reports whether an alias's recorded type arguments are
    // exactly its own declared type parameters. The declared type of a generic
    // alias is parameterized that way; such arguments are declaration operands and
    // are never part of an application's exposed surface.
    #[expect(clippy::unused_self, reason = "a method of Go's apiStabilityAnalysis")]
    pub(crate) fn alias_arguments_are_self(
        &self,
        tp: &mut TypeParser<'_>,
        alias: Option<P<TypeAlias>>,
    ) -> bool {
        let Some(alias) = alias else {
            return true;
        };
        let arguments = alias.type_arguments();
        if arguments.is_empty() {
            return true;
        }
        let Some(symbol) = alias.symbol() else {
            return false;
        };
        let parameters = tp
            .checker
            .get_local_type_parameters_of_class_or_interface_or_type_alias_exported(symbol);
        if parameters.len() != arguments.len() {
            return false;
        }
        for index in 0..arguments.len() {
            if arguments[index] != parameters[index] {
                return false;
            }
        }
        true
    }

    // go: internal/typeparser/api_stability.go:327 apiStabilityAnalysis.internCarrier e3fffd0228669fd1
    // internCarrier interns one concrete enclosing identity link into the analysis.
    // The table is discarded with the analysis, so no carrier survives an export.
    pub(crate) fn intern_carrier(
        &mut self,
        parent: Option<ApiStabilityCarrierRef>,
        owner: Option<P<Type>>,
        signature: Option<P<Signature>>,
    ) -> Option<ApiStabilityCarrierRef> {
        if owner.is_none() && signature.is_none() {
            return parent;
        }
        let key = ApiStabilityCarrier { parent, owner, signature, ..Default::default() };
        Some(self.intern_carrier_key(key))
    }

    /// Rust-only: the table lookup shared by `intern_carrier` and `intern_argument_carrier` (Go repeats it).
    fn intern_carrier_key(&mut self, key: ApiStabilityCarrier) -> ApiStabilityCarrierRef {
        if let Some(&existing) = self.carriers.get(&key) {
            return existing;
        }
        let carrier = ApiStabilityCarrierRef(
            u32::try_from(self.carriers.len()).expect("carrier table fits in u32"),
        );
        self.carriers.insert(key, carrier);
        carrier
    }

    // go: internal/typeparser/api_stability.go:343 apiStabilityAnalysis.internArgumentCarrier d0b75228aa6acccf
    // internArgumentCarrier interns one concrete type argument of a represented
    // generic application. A nil argument still interns a distinct link because the
    // marker keeps the position of an unrepresented argument part of the identity.
    pub(crate) fn intern_argument_carrier(
        &mut self,
        parent: Option<ApiStabilityCarrierRef>,
        argument: Option<P<Type>>,
    ) -> ApiStabilityCarrierRef {
        let key = ApiStabilityCarrier { parent, argument, marker: true, ..Default::default() };
        self.intern_carrier_key(key)
    }

    // go: internal/typeparser/api_stability.go:357 apiStabilityAnalysis.extendParametersSubstitution f696f8a0790af096
    // extendParametersSubstitution pushes one frame binding a set of type
    // parameters to represented arguments. The concrete owner is part of the
    // interned carrier, so the same raw component analyzed under two different
    // concrete applications never shares a cached result.
    pub(crate) fn extend_parameters_substitution(
        &mut self,
        subst: &ApiStabilitySubstitution,
        owner: Option<P<Type>>,
        parameters: Option<&'static [P<Type>]>,
        arguments: Vec<Option<P<Type>>>,
    ) -> ApiStabilitySubstitution {
        let mut carrier = self.intern_carrier(subst.carrier(), owner, None);
        for &argument in &arguments {
            carrier = Some(self.intern_argument_carrier(carrier, argument));
        }
        ApiStabilitySubstitution {
            frame: Some(Rc::new(ApiStabilitySubstFrame {
                parent: subst.frame.clone(),
                parameters,
                arguments,
                carrier,
                ..Default::default()
            })),
        }
    }

    // go: internal/typeparser/api_stability.go:373 apiStabilityAnalysis.extendMapperSubstitution 607a3002ff5f84c6
    // extendMapperSubstitution pushes one frame for a compiler-provided mapper
    // attached to an instantiated type (an inferred anonymous object, an
    // instantiated mapped type, or an instantiated conditional).
    pub(crate) fn extend_mapper_substitution(
        &mut self,
        subst: &ApiStabilitySubstitution,
        owner: Option<P<Type>>,
        mapper: P<TypeMapper>,
    ) -> ApiStabilitySubstitution {
        let carrier = self.intern_carrier(subst.carrier(), owner, None);
        ApiStabilitySubstitution {
            frame: Some(Rc::new(ApiStabilitySubstFrame {
                parent: subst.frame.clone(),
                mapper: Some(mapper),
                carrier,
                ..Default::default()
            })),
        }
    }

    // go: internal/typeparser/api_stability.go:386 apiStabilityAnalysis.extendSignatureSubstitution 83a00874a4a87d44
    // extendSignatureSubstitution pushes one frame for a concrete signature. The
    // raw signature's type parameters are resolved through the concrete
    // signature's represented type arguments and its instantiated parameter
    // mappers.
    pub(crate) fn extend_signature_substitution(
        &mut self,
        subst: &ApiStabilitySubstitution,
        signature: P<Signature>,
    ) -> ApiStabilitySubstitution {
        let carrier = self.intern_carrier(subst.carrier(), None, Some(signature));
        ApiStabilitySubstitution {
            frame: Some(Rc::new(ApiStabilitySubstFrame {
                parent: subst.frame.clone(),
                signature: Some(signature),
                carrier,
                ..Default::default()
            })),
        }
    }
}

// go: internal/typeparser/api_stability.go:399 apiStabilityTypeKey b7298bbdfe597f14
// Cache keys are concrete compiler identities. t and signature are the
// represented objects under inspection; carrier is the interned concrete
// enclosing identity when a raw (uninstantiated) component is walked inside a
// concrete context. No serialized substitution context is ever part of a key.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct ApiStabilityTypeKey {
    pub(crate) t: P<Type>,
    pub(crate) inspect: ApiStabilityInspection,
    pub(crate) carrier: Option<ApiStabilityCarrierRef>,
}

// go: internal/typeparser/api_stability.go:405 apiStabilitySignatureKey 5ca89282dab440f6
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct ApiStabilitySignatureKey {
    pub(crate) signature: P<Signature>,
    pub(crate) carrier: Option<ApiStabilityCarrierRef>,
}

// go: internal/typeparser/api_stability.go:410 apiStabilitySymbolKey 9563066e63856175
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct ApiStabilitySymbolKey {
    pub(crate) symbol: P<Symbol>,
    pub(crate) carrier: Option<ApiStabilityCarrierRef>,
}

// go: internal/typeparser/api_stability.go:415 apiStabilityAliasKey 12e072180b821a83
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct ApiStabilityAliasKey {
    pub(crate) t: P<Type>,
    pub(crate) carrier: Option<ApiStabilityCarrierRef>,
}

// go: internal/typeparser/api_stability.go:426 apiStabilitySurfaceTypeKey 458ee9545e46fd65
// apiStabilitySurfaceTypeKey is the concrete, context-free identity of one
// shared type-surface entry: the represented type pointer and the fixed
// inspection policy. The two policies occupy separate entry slots, so an
// expanded result can never contaminate a shallow one. No serialized context
// string and no substitution carrier participates; a surface computed under a
// substitution context stays analysis-local.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct ApiStabilitySurfaceTypeKey {
    pub(crate) t: P<Type>,
    pub(crate) inspect: ApiStabilityInspection,
}

// go: internal/typeparser/api_stability.go:439 apiStabilitySharedSurface f42f7a679b7c5f7f
// apiStabilitySharedSurface is one immutable snapshot of a complete, settled,
// context-free surface published on the per-checker EffectLinks. findings
// excludes every finding that matches the component's own declared tag (the
// same exclusion the root result conversion applies); removed records the
// concrete identities of those findings so a consumer can re-add them, with
// their declared level, from the per-checker declared caches. The snapshot is
// never mutated after publication: a consumer copies the findings into its own
// analysis-local surface before composing.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub(crate) struct ApiStabilitySharedSurface {
    pub(crate) findings: ApiStabilityFindings,
    pub(crate) removed: Vec<ApiStabilityFindingKey>,
}

// go: internal/typeparser/api_stability.go:446 apiStabilityMaterializationReadKind cf0eaf0337b6fd25
// apiStabilityMaterializationReadKind identifies one lazy read operation whose
// materialization epoch advance is deduplicated per concrete identity.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum ApiStabilityMaterializationReadKind {
    // go: internal/typeparser/api_stability.go:448 apiStabilityMaterializationReadDeclaredType 3878639802c1008e
    DeclaredType,
    Signatures,
    Members,
    NodeType,
}

// go: internal/typeparser/api_stability.go:459 apiStabilityMaterializationReadKey 764f01b3f6b62ec3
// apiStabilityMaterializationReadKey is the concrete identity of one
// deduplicated lazy read: the operation and the symbol or annotation node it
// resolves. It only decides whether the materialization epoch advances once
// more; it is never a cache identity.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct ApiStabilityMaterializationReadKey {
    pub(crate) kind: ApiStabilityMaterializationReadKind,
    pub(crate) symbol: Option<P<Symbol>>,
    pub(crate) node: Option<P<Node>>,
}

/// Rust-only: Go's `declaration.Parent != nil && declaration.Parent.Kind == ast.KindVariableDeclarationList &&
/// declaration.Parent.Parent != nil && declaration.Parent.Parent.Kind == ast.KindVariableStatement`, the enclosing
/// variable statement both tag parsers also inspect (Go repeats the condition).
fn enclosing_variable_statement(declaration: P<Node>) -> Option<P<Node>> {
    let list = declaration.parent()?;
    if list.kind() != Kind::VariableDeclarationList {
        return None;
    }
    let statement = list.parent()?;
    (statement.kind() == Kind::VariableStatement).then_some(statement)
}

// go: internal/typeparser/api_stability.go:470 stabilityOfDeclarationTag 87318c023c9a7ac8
// stabilityOfDeclarationTag resolves the `@stability` tag of a declaration. A
// variable's JSDoc is usually attached to its enclosing variable statement.
// This is the single declaration-level parser shared by the API usage rules and
// this analysis. An explicit `stable` tag is recognized and is distinct from an
// absent tag: it overrides an inherited tag instead of being ignored.
pub(crate) fn stability_of_declaration_tag(declaration: Option<P<Node>>) -> &'static str {
    let Some(declaration) = declaration else {
        return "";
    };
    let mut nodes = vec![declaration];
    if let Some(statement) = enclosing_variable_statement(declaration) {
        nodes.push(statement);
    }
    for node in nodes {
        if !node.flags().intersects(NodeFlags::PossiblyContainsStabilityTag) {
            continue;
        }
        for &doc in node.jsdoc(None) {
            let Some(tags) = doc.as_jsdoc().tags() else {
                continue;
            };
            for &tag in tags.nodes() {
                if tag.kind() != Kind::JSDocUnknownTag || tag.tag_name().text() != "stability" {
                    continue;
                }
                let comment = tag.as_jsdoc_unknown_tag().comment();
                if let Some(comment) = comment
                    && let Some(&first) = comment.nodes().first()
                    && first.kind() == Kind::JSDocText
                {
                    match first.text().trim() {
                        "stable" => return "stable",
                        "unstable" => return "unstable",
                        "experimental" => return "experimental",
                        _ => {}
                    }
                }
            }
        }
    }
    ""
}

// go: internal/typeparser/api_stability.go:506 StabilityTagOfDeclaration 47d78f293521cf83
// StabilityTagOfDeclaration exposes the declaration-level `@stability` parser
// so the existing usage diagnostics reuse one implementation.
pub fn stability_tag_of_declaration(declaration: impl Into<Option<P<Node>>>) -> &'static str {
    stability_of_declaration_tag(declaration.into())
}

// go: internal/typeparser/api_stability.go:517 InternalTagOfDeclaration 5141c124ec310f70
// InternalTagOfDeclaration reports whether a declaration carries the parsed
// JSDoc `@internal` tag. Native TypeScript recognizes the same tag for
// `stripInternal` declaration emit: a tagged declaration is omitted from the
// emitted declarations, so it is not part of the public API. Only parsed tags
// match, so prose mentioning the word is never treated as a tag. A variable's
// JSDoc is usually attached to its enclosing variable statement, so that is
// checked too, mirroring the `@stability` parser.
pub fn internal_tag_of_declaration(declaration: impl Into<Option<P<Node>>>) -> bool {
    let Some(declaration) = declaration.into() else {
        return false;
    };
    let mut nodes = vec![declaration];
    if let Some(statement) = enclosing_variable_statement(declaration) {
        nodes.push(statement);
    }
    for node in nodes {
        if !node.flags().intersects(NodeFlags::HasJSDoc) {
            continue;
        }
        for &doc in node.jsdoc(None) {
            let Some(tags) = doc.as_jsdoc().tags() else {
                continue;
            };
            for &tag in tags.nodes() {
                if tag.kind() == Kind::JSDocUnknownTag && tag.tag_name().text() == "internal" {
                    return true;
                }
            }
        }
    }
    false
}

impl TypeParser<'_> {
    // go: internal/typeparser/api_stability.go:549 TypeParser.DeclaredApiStability fa70683d6bd4d739
    // DeclaredApiStability returns the stability tier declared for a symbol. The
    // result is cached per checker on EffectLinks so runs over different source
    // files share the same lookup. Declared lookups never trigger any computed
    // surface analysis.
    pub fn declared_api_stability(
        &mut self,
        symbol: impl Into<Option<P<Symbol>>>,
    ) -> ApiStabilityLevel {
        self.declared_api_stability_of_symbol(symbol).level
    }

    // go: internal/typeparser/api_stability.go:556 TypeParser.DeclaredApiStabilityOfSymbol 3ad9d39dd0232fd7
    // DeclaredApiStabilityOfSymbol returns the declared stability of a symbol,
    // resolving import/export aliases and honouring a tag on the enclosing export
    // declaration. Untagged symbols are stable. The result is cached per checker.
    pub fn declared_api_stability_of_symbol(
        &mut self,
        symbol: impl Into<Option<P<Symbol>>>,
    ) -> ApiStabilityDeclaration {
        let Some(symbol) = symbol.into() else {
            return ApiStabilityDeclaration::default();
        };
        cached(
            self,
            |l| &mut l.api_stability_declared_symbol,
            symbol,
            |tp| declared_stability_of_symbol_chain(tp.checker, Some(symbol)),
        )
    }

    // go: internal/typeparser/api_stability.go:570 TypeParser.DeclaredApiStabilityOfSignature b89b6f76149f7a2e
    // DeclaredApiStabilityOfSignature returns the declared stability carried by a
    // selected signature declaration. It is cached per checker on the raw
    // (uninstantiated) signature so different overloads keep distinct tags; a
    // symbol-level tag must never collapse them. An untagged signature is stable
    // with no declaration, which lets callers fall back to the symbol.
    pub fn declared_api_stability_of_signature(
        &mut self,
        signature: impl Into<Option<P<Signature>>>,
    ) -> ApiStabilityDeclaration {
        let Some(signature) = signature.into() else {
            return ApiStabilityDeclaration::default();
        };
        let Some(raw) = raw_signature(Some(signature)) else {
            return ApiStabilityDeclaration::default();
        };
        cached(
            self,
            |l| &mut l.api_stability_declared_signature,
            raw,
            |tp| tp.declared_api_stability_of_declaration(raw.declaration()),
        )
    }

    // go: internal/typeparser/api_stability.go:587 TypeParser.DeclaredApiStabilityOfDeclaration 12331e5593fbf8ec
    // DeclaredApiStabilityOfDeclaration returns the stability tag attached to a
    // declaration and the declaration that carries it. The result is cached per
    // checker; this is the shared declaration-level lookup used by the symbol and
    // signature accessors.
    pub fn declared_api_stability_of_declaration(
        &mut self,
        declaration: impl Into<Option<P<Node>>>,
    ) -> ApiStabilityDeclaration {
        let Some(declaration) = declaration.into() else {
            return ApiStabilityDeclaration::default();
        };
        cached(
            self,
            |l| &mut l.api_stability_declared_declaration,
            declaration,
            |_| {
                let stability = stability_of_declaration_tag(Some(declaration));
                if !stability.is_empty() {
                    return ApiStabilityDeclaration {
                        level: api_stability_rank(stability),
                        declaration: Some(declaration),
                    };
                }
                ApiStabilityDeclaration::default()
            },
        )
    }
}

// go: internal/typeparser/api_stability.go:599 apiStabilityRank c71b7d28957e5fde
pub(crate) fn api_stability_rank(stability: &str) -> ApiStabilityLevel {
    match stability {
        "unstable" => ApiStabilityLevel::Unstable,
        "experimental" => ApiStabilityLevel::Experimental,
        _ => ApiStabilityLevel::Stable,
    }
}

// go: internal/typeparser/api_stability.go:614 declaredStabilityOfSymbolChain a30ccb3cace3df42
// declaredStabilityOfSymbolChain mirrors the reference resolution used by the
// stability usage rules: a declaration's own tag wins, then import/export
// aliases are followed. Alias cycles are broken by symbol identity rather than
// a fixed hop count, so long re-export chains keep their inherited stability.
pub(crate) fn declared_stability_of_symbol_chain(
    c: &mut Checker,
    symbol: Option<P<Symbol>>,
) -> ApiStabilityDeclaration {
    let mut seen: FxHashSet<P<Symbol>> = FxHashSet::default();
    let mut symbol = symbol;
    while let Some(current) = symbol {
        if seen.contains(&current) {
            break;
        }
        seen.insert(current);
        for &declaration in current.declarations() {
            let stability = stability_of_declaration_tag(Some(declaration));
            if !stability.is_empty() {
                return ApiStabilityDeclaration {
                    level: api_stability_rank(stability),
                    declaration: Some(declaration),
                };
            }
        }
        if !current.flags().intersects(SymbolFlags::Alias) {
            break;
        }
        // The checker synthesizes a declaration-less `default` alias for `export =`
        // and JSON modules and panics when asked for its immediate target.
        if !has_alias_declaration(current) {
            break;
        }
        let next = api_stability_immediate_aliased_symbol(c, Some(current));
        if next == Some(current) {
            break;
        }
        symbol = next;
    }
    ApiStabilityDeclaration::default()
}

// go: internal/typeparser/api_stability.go:643 hasAliasDeclaration 7cb32c802b0c3345
pub(crate) fn has_alias_declaration(symbol: P<Symbol>) -> bool {
    symbol.declarations().iter().any(|&d| ets_shim::ast::is_alias_symbol_declaration(d))
}

// go: internal/typeparser/api_stability.go:652 ApiStabilityImmediateAliasedSymbol e093401f0820771a
// ApiStabilityImmediateAliasedSymbol follows one valid import/export alias hop.
// Immediate target lookup can repeat diagnostics for failed resolutions on
// the legacy compiler. Skip those cached failures without resolving the alias,
// then keep the immediate hop so forwarding declarations retain their tags.
// Neither operation resolves or instantiates types.
pub fn api_stability_immediate_aliased_symbol(
    c: &mut Checker,
    symbol: impl Into<Option<P<Symbol>>>,
) -> Option<P<Symbol>> {
    let symbol = symbol.into()?;
    if !symbol.flags().intersects(SymbolFlags::Alias) || !has_alias_declaration(symbol) {
        return None;
    }
    if is_alias_resolution_failed(c, Some(symbol)) {
        return None;
    }
    c.get_immediate_aliased_symbol_exported(symbol)
}

// go: internal/typeparser/api_stability.go:672 ApiStabilitySession 5276c1a26944045f
// ApiStabilitySession is the analysis-local memo for the computed stability of
// one export. The rule keeps one session per file, resets it before each export
// and queries the export's symbol (and any namespace member) through it, so
// repeated components are computed once within the export. After settling, the
// session publishes every complete, context-free concrete type and signature
// surface to the per-checker shared caches, so a later export, file or
// TypeParser over the same checker composes that snapshot instead of
// recomputing; the session's symbol and carrier memos, and every incomplete or
// blocked result, are discarded with the session. Declared stability is read
// from the per-checker declared caches and never needs a session.
//
// Rust: Go's analysis keeps the `*TypeParser` it was created by; the Rust session would then borrow the parser
// mutably for as long as it lives, while the rule keeps using the parser between queries. So the session owns only
// the analysis state, and each query takes the parser (it must be one over the same checker, as in Go).
pub struct ApiStabilitySession {
    pub(crate) analysis: ApiStabilityAnalysis,
}

impl TypeParser<'_> {
    // go: internal/typeparser/api_stability.go:680 TypeParser.NewApiStabilitySession a83d4eca766da806
    // NewApiStabilitySession starts a fresh computed-stability analysis. The
    // session owns its symbol, signature, concrete type/component and substitution
    // carrier memos; only the complete context-free surfaces published through
    // settled snapshots outlive it.
    #[expect(clippy::unused_self, reason = "a TypeParser method in Go")]
    pub fn new_api_stability_session(&self) -> ApiStabilitySession {
        ApiStabilitySession { analysis: new_api_stability_analysis() }
    }
}

impl ApiStabilitySession {
    // go: internal/typeparser/api_stability.go:690 ApiStabilitySession.Reset 875d036c918ee032
    // Reset discards every analysis-local result so the session analyzes the next
    // export exactly like a new session would, while keeping its allocated tables.
    // The published shared snapshots are unaffected.
    pub fn reset(&mut self) {
        self.analysis.reset();
    }

    // go: internal/typeparser/api_stability.go:710 ApiStabilitySession.UsedBySymbol 2a836aef9828e561
    // UsedBySymbol computes the stability the children of a symbol's public
    // surface expose inside this session and settles cycle cuts: the root symbol's
    // own declared tag is excluded from the returned dependencies and minimum. The
    // own tag is exactly what DeclaredApiStabilityOfSymbol reports, read from the
    // per-checker declared cache; a tag a function's declaration carries is
    // recorded as a signature finding and is excluded together with the root
    // symbol finding, while a tagged overload of another declaration stays an
    // exposed child. Excluding the root tag is a result conversion only: the
    // stored surface always keeps it, so the same symbol reached as a child of
    // another root contributes its own tag to that parent. A complete result is
    // memoized for later queries of the same session; a result cut by an
    // unavailable component stays incomplete and is retried by a later query or
    // session.
    pub fn used_by_symbol(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: impl Into<Option<P<Symbol>>>,
    ) -> ApiStabilityUsed {
        let Some(symbol) = symbol.into() else {
            return ApiStabilityUsed::default();
        };
        self.analysis.symbol_surface(tp, Some(symbol));
        self.analysis.settle(tp);
        let surface = self.analysis.session_symbols.get(&symbol).cloned().unwrap_or_default();
        let own = self.analysis.symbol_own_tag(tp, Some(symbol));
        api_stability_used_of_own(&surface, own)
    }

    // go: internal/typeparser/api_stability.go:727 ApiStabilitySession.UsedByType 8156ebca67df6db4
    // UsedByType computes the stability the children of an already represented
    // checker type expose inside this session. The type's own declared tag is
    // excluded from the returned dependencies and minimum: a type alias
    // application's own tag is its alias symbol's, and a plain represented type's
    // own tag is its own symbol's. An underlying named type an alias resolves to
    // is an exposed child and keeps contributing. The stored surface is untouched,
    // so the same type reached as a child of another root still contributes its
    // own tag to that parent.
    pub fn used_by_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: impl Into<Option<P<Type>>>,
    ) -> ApiStabilityUsed {
        let Some(t) = t.into() else {
            return ApiStabilityUsed::default();
        };
        self.analysis.type_surface(
            tp,
            Some(t),
            ApiStabilityInspection::Expand,
            &ApiStabilitySubstitution::default(),
        );
        self.analysis.settle(tp);
        let key = ApiStabilityTypeKey { t, inspect: ApiStabilityInspection::Expand, carrier: None };
        let surface = self.analysis.session_types.get(&key).cloned().unwrap_or_default();
        let own = self.analysis.type_own_tag(tp, Some(t));
        api_stability_used_of_own(&surface, own)
    }

    // go: internal/typeparser/api_stability.go:742 ApiStabilitySession.UsedBySignature 99823b0efd3dd3c9
    // UsedBySignature computes the stability the children of an already
    // represented signature expose inside this session. The signature's own
    // declared tag is excluded from the returned dependencies and minimum; its
    // parameters, type parameters and return keep contributing. The raw signature
    // identity makes each overload's own tag distinct from every other overload's.
    pub fn used_by_signature(
        &mut self,
        tp: &mut TypeParser<'_>,
        signature: impl Into<Option<P<Signature>>>,
    ) -> ApiStabilityUsed {
        let Some(signature) = signature.into() else {
            return ApiStabilityUsed::default();
        };
        self.analysis.signature_surface(tp, Some(signature), &ApiStabilitySubstitution::default());
        self.analysis.settle(tp);
        let key = ApiStabilitySignatureKey { signature, carrier: None };
        let surface = self.analysis.session_signatures.get(&key).cloned().unwrap_or_default();
        let own = self.analysis.signature_own_tag(tp, Some(signature));
        api_stability_used_of_own(&surface, own)
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/api_stability.go:786 TypeParser.ApiStabilityUsedBySymbol df5a169dad464c45
    // ApiStabilityUsedBySymbol computes the stability the children of a symbol's
    // public surface expose in a fresh one-call session. The root symbol's own
    // declared tag is excluded; use DeclaredApiStabilityOfSymbol to read it. The
    // traversal walks compiler represented type graphs: an exported interface's own
    // and inherited members, index signatures, call/construct signatures, generic
    // constraints and defaults, type arguments of represented references, and the
    // alias symbols written in public annotations that the checker erases. A named
    // dependency reached inside the surface stays shallow (its own public call
    // signatures are still inspected), so arbitrary referenced member graphs are
    // never expanded. A child with an explicit `@stability` tag is a boundary: its
    // declared level contributes to the parent surface and the child's internals
    // are not expanded, while the same component keeps being audited when it is
    // itself the queried root. A child without an explicit tag composes within the
    // current shallow and anonymous graph boundaries: its public call and construct
    // signatures, inherited surface, index signatures and represented members are
    // inspected under the same rules. Each call starts from a fresh analysis, but a
    // complete, settled, context-free concrete type or signature surface is
    // published to the per-checker shared caches and reused by a later export, file
    // or TypeParser over the same checker; symbol-root results and every
    // incomplete, blocked or substitution-context result stay analysis-local.
    //
    // Public components the checker has not computed are materialized through its
    // ordinary lazy accessors, preferring an already computed result. Before a
    // potentially recursive read the non-evaluating pre-flight recursion guard
    // inspects the represented carrier and the raw declarations with the compiler's
    // own relations; a read whose expansion may evaluate a recursive alias, or
    // whose safety the guard cannot establish, is refused and leaves the result
    // incomplete (blocked), which is never reported as stable and is retried by a
    // later session. An inferred component (a function return or a variable
    // initializer) is read through the checker's own lazy inference, which
    // attributes the diagnostics it produces to the declaring file's expressions
    // exactly as that file's ordinary check would, so the rule never suppresses or
    // consumes compiler diagnostics and the checked program keeps its own types and
    // diagnostics exactly.
    pub fn api_stability_used_by_symbol(
        &mut self,
        symbol: impl Into<Option<P<Symbol>>>,
    ) -> ApiStabilityUsed {
        self.new_api_stability_session().used_by_symbol(self, symbol)
    }

    // go: internal/typeparser/api_stability.go:797 TypeParser.ApiStabilityUsedByType 9d6f674052460867
    // ApiStabilityUsedByType computes the stability the children of an already
    // represented checker type expose in a fresh one-call session. The type's own
    // declared tag is excluded; use the declared accessors to read it. It switches
    // on TypeFlags/ObjectFlags and reads represented inner fields; a reference is
    // inspected through its concrete public surface when expanded and through its
    // declaration plus represented type arguments when shallow. Recursive alias
    // applications are never expanded.
    pub fn api_stability_used_by_type(
        &mut self,
        t: impl Into<Option<P<Type>>>,
    ) -> ApiStabilityUsed {
        self.new_api_stability_session().used_by_type(self, t)
    }

    // go: internal/typeparser/api_stability.go:808 TypeParser.ApiStabilityUsedBySignature d266eea7ce61f01f
    // ApiStabilityUsedBySignature computes the stability the children of an
    // already represented signature expose in a fresh one-call session. The
    // signature's own declared tag is excluded; use DeclaredApiStabilityOfSignature
    // to read it. A concrete signature's instantiated parameter and return
    // components are preferred; the uninstantiated target plus the represented
    // substitution remains the fallback, so a directly exported inferred return
    // such as `make<E>()` still exposes `E` without expanding a recursive alias.
    pub fn api_stability_used_by_signature(
        &mut self,
        signature: impl Into<Option<P<Signature>>>,
    ) -> ApiStabilityUsed {
        self.new_api_stability_session().used_by_signature(self, signature)
    }
}

// go: internal/typeparser/api_stability.go:821 apiStabilityOwnTag 608a8a885296ddae
// apiStabilityOwnTag identifies a root component's own declared tag inside a
// stored surface: the root symbol or raw signature identity together with the
// declaration that carries the root's declared stability. Stored surfaces
// always keep a component's own tag so the same component reached as a child
// contributes it to the parent surface; the result conversion for a root query
// removes exactly the findings this structure matches. The declaration identity
// also matches a tagged root signature: a function declaration's tag is
// recorded as a signature finding, and it is the root's own tag exactly when
// the declared accessor reports the same declaration.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct ApiStabilityOwnTag {
    pub(crate) symbol: Option<P<Symbol>>,
    pub(crate) signature: Option<P<Signature>>,
    pub(crate) declaration: Option<P<Node>>,
}

impl ApiStabilityOwnTag {
    // go: internal/typeparser/api_stability.go:830 apiStabilityOwnTag.excludes c5b028608af1073a
    // excludes reports whether one stored finding is the root's own declared tag.
    // A nil root field never matches, so a child-only conversion (a component, a
    // nil owner) excludes nothing.
    pub(crate) fn excludes(
        &self,
        key: &ApiStabilityFindingKey,
        finding: ApiStabilityFinding,
    ) -> bool {
        if self.symbol.is_some() && key.signature.is_none() && key.symbol == self.symbol {
            return true;
        }
        if self.signature.is_some()
            && key.signature.is_some()
            && raw_signature(key.signature) == raw_signature(self.signature)
        {
            return true;
        }
        self.declaration.is_some() && finding.declaration == self.declaration
    }
}

impl ApiStabilityAnalysis {
    // go: internal/typeparser/api_stability.go:843 apiStabilityAnalysis.symbolOwnTag 842d90ac3f95a132
    // symbolOwnTag builds the own-tag identity of a symbol root from the declared
    // accessor. The declared lookup only reads the per-checker declared caches, so
    // building the identity never computes a surface.
    #[expect(clippy::unused_self, reason = "a method of Go's apiStabilityAnalysis")]
    pub(crate) fn symbol_own_tag(
        &self,
        tp: &mut TypeParser<'_>,
        symbol: Option<P<Symbol>>,
    ) -> ApiStabilityOwnTag {
        let Some(symbol) = symbol else {
            return ApiStabilityOwnTag::default();
        };
        let declared = tp.declared_api_stability_of_symbol(symbol);
        ApiStabilityOwnTag {
            symbol: Some(symbol),
            declaration: declared.declaration,
            ..Default::default()
        }
    }

    // go: internal/typeparser/api_stability.go:855 apiStabilityAnalysis.typeOwnTag 9f0d47985ca2bf79
    // typeOwnTag builds the own-tag identity of a represented type root. A type
    // alias application's own tag is the alias symbol's; an underlying named type
    // the alias resolves to stays an exposed child. Any other represented type's
    // own tag is its own symbol's.
    pub(crate) fn type_own_tag(
        &self,
        tp: &mut TypeParser<'_>,
        t: Option<P<Type>>,
    ) -> ApiStabilityOwnTag {
        let Some(t) = t else {
            return ApiStabilityOwnTag::default();
        };
        if let Some(alias) = t.alias()
            && let Some(alias_symbol) = alias.symbol()
        {
            return self.symbol_own_tag(tp, Some(alias_symbol));
        }
        if let Some(symbol) = t.symbol() {
            return self.symbol_own_tag(tp, Some(symbol));
        }
        ApiStabilityOwnTag::default()
    }

    // go: internal/typeparser/api_stability.go:870 apiStabilityAnalysis.signatureOwnTag d88e6b1a1b79cee1
    // signatureOwnTag builds the own-tag identity of a signature root. The raw
    // (uninstantiated) signature keeps distinct overload tags distinct.
    #[expect(clippy::unused_self, reason = "a method of Go's apiStabilityAnalysis")]
    pub(crate) fn signature_own_tag(
        &self,
        tp: &mut TypeParser<'_>,
        signature: Option<P<Signature>>,
    ) -> ApiStabilityOwnTag {
        let Some(raw) = raw_signature(signature) else {
            return ApiStabilityOwnTag::default();
        };
        let declared = tp.declared_api_stability_of_signature(raw);
        ApiStabilityOwnTag {
            signature: Some(raw),
            declaration: declared.declaration,
            ..Default::default()
        }
    }
}

// go: internal/typeparser/api_stability.go:883 apiStabilityUsedOf 214fcfd6a7cdd024
#[cfg_attr(not(test), expect(dead_code, reason = "Go calls it from the settlement tests only"))]
// apiStabilityUsedOf converts a stored symbol surface into the public result
// for a symbol-rooted query, excluding the root symbol's own finding. It is the
// settlement-test entry point; the session accessors build the richer own-tag
// identity, which also excludes a root's tagged signature declaration.
pub(crate) fn api_stability_used_of(
    surface: &ApiStabilitySurface,
    owner: Option<P<Symbol>>,
) -> ApiStabilityUsed {
    api_stability_used_of_own(surface, ApiStabilityOwnTag { symbol: owner, ..Default::default() })
}

// go: internal/typeparser/api_stability.go:890 apiStabilityUsedOfOwn 5a5240d3ef2971ce
// apiStabilityUsedOfOwn converts a stored surface into the public result,
// skipping every finding that is the root's own declared tag. The stored
// surface itself is never modified.
pub(crate) fn api_stability_used_of_own(
    surface: &ApiStabilitySurface,
    own: ApiStabilityOwnTag,
) -> ApiStabilityUsed {
    let mut used = ApiStabilityUsed {
        minimum: ApiStabilityLevel::Stable,
        incomplete: !surface.complete,
        ..Default::default()
    };
    for (key, finding) in surface.findings.iter() {
        if own.excludes(key, *finding) {
            continue;
        }
        let mut declaration = finding.declaration;
        if declaration.is_none() {
            declaration = key.declaration;
        }
        used.dependencies.push(ApiStabilityDependency {
            symbol: key.symbol,
            signature: key.signature,
            declaration,
            level: finding.level,
        });
        if finding.level > used.minimum {
            used.minimum = finding.level;
        }
    }
    sort_api_stability_dependencies(&mut used.dependencies);
    used
}

// go: internal/typeparser/api_stability.go:914 sortApiStabilityDependencies 17f9be188d71e2de
pub(crate) fn sort_api_stability_dependencies(deps: &mut [ApiStabilityDependency]) {
    ets_go::sort::slice(deps, |left, right| {
        let left_name = api_stability_dependency_sort_name(left);
        let right_name = api_stability_dependency_sort_name(right);
        if left_name != right_name {
            return left_name < right_name;
        }
        if left.level != right.level {
            return left.level > right.level;
        }
        left.symbol.is_some() && right.symbol.is_none()
    });
}

// go: internal/typeparser/api_stability.go:928 apiStabilityDependencySortName e00f29d44664abdf
pub(crate) fn api_stability_dependency_sort_name(dependency: &ApiStabilityDependency) -> String {
    let name = api_stability_usable_symbol_name(dependency.symbol);
    if !name.is_empty() {
        return name.to_string();
    }
    if let Some(name) = api_stability_declaration_display_name(dependency.declaration) {
        return name;
    }
    let name = api_stability_late_bound_symbol_display_name(dependency.symbol);
    if !name.is_empty() {
        return name;
    }
    if let Some(symbol) = dependency.symbol
        && !symbol.name().is_empty()
    {
        return symbol.name().to_string();
    }
    if dependency.signature.is_some() {
        return "signature".to_string();
    }
    "<anonymous>".to_string()
}

// go: internal/typeparser/api_stability.go:952 ApiStabilityDependencyName 39dcc8987234fae7
// ApiStabilityDependencyName returns the display name of an exposed offender
// for diagnostics: the symbol name, the declaration's safely read name, or a
// descriptive fallback for anonymous signatures. A computed declaration name is
// never evaluated; its display spelling comes from its declaration symbol or
// its well-known symbol expression.
pub fn api_stability_dependency_name(dependency: &ApiStabilityDependency) -> String {
    let name = api_stability_usable_symbol_name(dependency.symbol);
    if !name.is_empty() {
        return name.to_string();
    }
    if let Some(name) = api_stability_declaration_display_name(dependency.declaration) {
        return name;
    }
    let name = api_stability_late_bound_symbol_display_name(dependency.symbol);
    if !name.is_empty() {
        return name;
    }
    if let Some(symbol) = dependency.symbol
        && !symbol.name().is_empty()
    {
        return symbol.name().to_string();
    }
    if let Some(declaration) = dependency.declaration {
        match declaration.kind() {
            Kind::CallSignature => return "call signature".to_string(),
            Kind::ConstructSignature => return "constructor".to_string(),
            Kind::IndexSignature => return "index signature".to_string(),
            _ => {}
        }
        if let Some(name) = ets_shim::ast::get_name_of_declaration(declaration)
            && name.kind() == Kind::ComputedPropertyName
        {
            return "[computed]".to_string();
        }
    }
    if let Some(signature) = dependency.signature {
        if signature.flags().intersects(SignatureFlags::Construct) {
            return "constructor".to_string();
        }
        return "call signature".to_string();
    }
    "<anonymous>".to_string()
}

// go: internal/typeparser/api_stability.go:991 apiStabilityUsableSymbolName 532db58e7ac6a4f5
// apiStabilityUsableSymbolName returns a symbol name suitable for display,
// excluding the binder's `__computed` placeholder and the checker's `__@`
// late-bound member spellings. A computed declaration renders its own readable
// spelling instead of exposing those internal names.
pub(crate) fn api_stability_usable_symbol_name(symbol: Option<P<Symbol>>) -> &'static str {
    let Some(symbol) = symbol else {
        return "";
    };
    let name = symbol.name();
    if name.is_empty() || name.starts_with(INTERNAL_SYMBOL_NAME_PREFIX) || name.starts_with("__@") {
        return "";
    }
    name
}

// go: internal/typeparser/api_stability.go:1007 apiStabilityLateBoundSymbolDisplayName 6dc7a2440ee79a64
// apiStabilityLateBoundSymbolDisplayName renders the bound property name of a
// checker late-bound member (`__@name@id`) for diagnostics and sorting. The
// checker records that name when it late-binds a computed declaration, so
// reading it never evaluates the source expression and never leaks the internal
// spelling. It returns "" for anything that is not a late-bound symbol name.
pub(crate) fn api_stability_late_bound_symbol_display_name(symbol: Option<P<Symbol>>) -> String {
    let Some(symbol) = symbol else {
        return String::new();
    };
    if !ets_shim::checker::is_known_symbol(symbol) {
        return String::new();
    }
    let full = symbol.name();
    let prefix = format!("{INTERNAL_SYMBOL_NAME_PREFIX}@");
    let mut name = full.strip_prefix(prefix.as_str()).unwrap_or(full);
    if let Some(index) = name.rfind('@') {
        name = &name[..index];
    }
    if name.is_empty() {
        return String::new();
    }
    format!("[{name}]")
}

// go: internal/typeparser/api_stability.go:1028 apiStabilityDeclarationDisplayName eaf11fe0fa3e6b6d
// apiStabilityDeclarationDisplayName returns the name of a declaration for
// sorting and display without evaluating the name. A simply named declaration
// keeps its source spelling. A computed name is never asked for its text: a
// well-known `Symbol.<name>` expression is rendered from its own identifiers, a
// symbol the binder already attached to the declaration names it, and any
// other computed name reports no name so the caller can fall back
// deterministically. The boolean reports whether a name was available.
pub(crate) fn api_stability_declaration_display_name(
    declaration: Option<P<Node>>,
) -> Option<String> {
    let declaration = declaration?;
    let name = ets_shim::ast::get_name_of_declaration(declaration)?;
    if let Some(simple) = api_stability_simple_name_text(Some(name)) {
        return Some(simple.to_string());
    }
    if name.kind() == Kind::ComputedPropertyName {
        let well_known = api_stability_well_known_symbol_display_name(name.expression());
        if !well_known.is_empty() {
            return Some(well_known);
        }
        // A literal computed name denotes exactly the literal it is written
        // with; reading that literal is not an evaluation. An identifier
        // computed name denotes the identifier's value, not its spelling, so
        // it is never rendered from source text.
        if let Some(expression) = name.expression() {
            match expression.kind() {
                Kind::StringLiteral
                | Kind::NumericLiteral
                | Kind::BigIntLiteral
                | Kind::NoSubstitutionTemplateLiteral => {
                    return Some(expression.text().to_string());
                }
                _ => {}
            }
        }
        if let Some(declaration_symbol) = declaration.symbol() {
            let symbol_name = declaration_symbol.name();
            if !symbol_name.is_empty() && !symbol_name.starts_with(INTERNAL_SYMBOL_NAME_PREFIX) {
                return Some(symbol_name.to_string());
            }
        }
    }
    None
}

// go: internal/typeparser/api_stability.go:1066 apiStabilitySimpleNameText 0a5141a90a91b0d4
// apiStabilitySimpleNameText returns the source text of a declaration name that
// has one without evaluation. A computed or otherwise dynamic name reports
// false; its text is never read.
pub(crate) fn api_stability_simple_name_text(name: Option<P<Node>>) -> Option<&'static str> {
    let name = name?;
    match name.kind() {
        Kind::Identifier
        | Kind::StringLiteral
        | Kind::NumericLiteral
        | Kind::BigIntLiteral
        | Kind::PrivateIdentifier => Some(name.text()),
        _ => None,
    }
}

// go: internal/typeparser/api_stability.go:1079 apiStabilityScopeList a907e102a20a32ee
// apiStabilityScopeList is an insertion-ordered set. Settlement iterates it
// instead of a Go map so the visit order is the order a query reached results.
#[derive(Clone, Debug)]
pub(crate) struct ApiStabilityScopeList<K> {
    pub(crate) keys: Vec<K>,
    pub(crate) has: FxHashSet<K>,
}

impl<K> Default for ApiStabilityScopeList<K> {
    fn default() -> Self {
        ApiStabilityScopeList { keys: Vec::new(), has: FxHashSet::default() }
    }
}

impl<K: Copy + Eq + std::hash::Hash> ApiStabilityScopeList<K> {
    // go: internal/typeparser/api_stability.go:1084 apiStabilityScopeList.add 236476ef80903b1a
    pub(crate) fn add(&mut self, key: K) {
        if self.has.contains(&key) {
            return;
        }
        self.has.insert(key);
        self.keys.push(key);
    }

    // go: internal/typeparser/api_stability.go:1095 apiStabilityScopeList.reset 931438bbf0037836
    pub(crate) fn reset(&mut self) {
        self.keys.clear();
        self.has.clear();
    }
}

// go: internal/typeparser/api_stability.go:1104 apiStabilitySettleScope ea4fa63fa692b611
// apiStabilitySettleScope holds the incomplete results the current query
// stored or read since the last converged settle. A result the query never
// reached cannot feed the query's root, so settlement only revisits these and
// whatever their recollections reach.
#[derive(Clone, Debug, Default)]
pub(crate) struct ApiStabilitySettleScope {
    pub(crate) symbols: ApiStabilityScopeList<P<Symbol>>,
    pub(crate) types: ApiStabilityScopeList<ApiStabilityTypeKey>,
    pub(crate) components: ApiStabilityScopeList<ApiStabilityTypeKey>,
    pub(crate) signatures: ApiStabilityScopeList<ApiStabilitySignatureKey>,
}

impl ApiStabilitySettleScope {
    // go: internal/typeparser/api_stability.go:1111 apiStabilitySettleScope.empty f449458ffebf2808
    pub(crate) fn empty(&self) -> bool {
        self.symbols.keys.is_empty()
            && self.types.keys.is_empty()
            && self.components.keys.is_empty()
            && self.signatures.keys.is_empty()
    }

    // go: internal/typeparser/api_stability.go:1115 apiStabilitySettleScope.reset dd753c0ebbb23a5e
    pub(crate) fn reset(&mut self) {
        self.symbols.reset();
        self.types.reset();
        self.components.reset();
        self.signatures.reset();
    }
}

// go: internal/typeparser/api_stability.go:1134 apiStabilityAnalysis 0db70def7c87d334
// apiStabilityAnalysis carries the cycle guards and memo for one computation.
// Complete symbol, signature, type and component results are reused inside the
// analysis, results contaminated by a cycle are settled with a bounded
// fixpoint, and a result whose components are unavailable stays incomplete and
// is never treated as stable. After the fixpoint has converged, every complete,
// settled, context-free concrete type and signature result is also published
// to the per-checker shared caches as a root-excluding snapshot; a result that
// is still in progress, cycle-cut before promotion, blocked or otherwise
// incomplete is never published, so a partial result can never leak into
// another export. Symbol surfaces stay analysis-local: a symbol root composes
// its own declared identity with the reusable concrete type and signature
// surfaces.
//
// Rust: Go's `tp` field is the parser every method takes as its `tp` parameter (see `ApiStabilitySession`). Go's
// `map[K]bool` and `map[K]struct{}` sets are `FxHashSet`s.
#[derive(Debug, Default)]
pub(crate) struct ApiStabilityAnalysis {
    pub(crate) session_symbols: FxHashMap<P<Symbol>, ApiStabilitySurface>,
    pub(crate) session_types: FxHashMap<ApiStabilityTypeKey, ApiStabilitySurface>,
    pub(crate) session_components: FxHashMap<ApiStabilityTypeKey, ApiStabilitySurface>,
    pub(crate) session_signatures: FxHashMap<ApiStabilitySignatureKey, ApiStabilitySurface>,
    pub(crate) type_frames: FxHashMap<ApiStabilityTypeKey, Option<Rc<ApiStabilitySubstFrame>>>,
    pub(crate) signature_frames:
        FxHashMap<ApiStabilitySignatureKey, Option<Rc<ApiStabilitySubstFrame>>>,

    // carriers interns the concrete enclosing identity of raw-component
    // traversals for this analysis only. Equal chains share one node pointer so
    // cache and cycle keys compare by concrete identity instead of a serialized
    // substitution context; the table is discarded with the analysis.
    pub(crate) carriers: FxHashMap<ApiStabilityCarrier, ApiStabilityCarrierRef>,

    // settleSymbols, settleTypes, settleComponents and settleSignatures record
    // the first-insertion order of the session results. reconcileSettleOrder
    // compares them with the session maps to find results stored without the
    // store helpers. The order is analysis-local bookkeeping; it is never part
    // of any cache key.
    pub(crate) settle_symbols: Vec<P<Symbol>>,
    pub(crate) settle_types: Vec<ApiStabilityTypeKey>,
    pub(crate) settle_components: Vec<ApiStabilityTypeKey>,
    pub(crate) settle_signatures: Vec<ApiStabilitySignatureKey>,

    // scope is the set of incomplete results the next settle revisits, in the
    // order the query reached them, so the fixpoint stays deterministic without
    // walking the whole session.
    pub(crate) scope: ApiStabilitySettleScope,

    // publishTypes and publishSignatures queue the complete context-free results
    // produced since the last settle, so publication only visits new results.
    pub(crate) publish_types: Vec<ApiStabilityTypeKey>,
    pub(crate) publish_signatures: Vec<ApiStabilitySignatureKey>,

    pub(crate) active_symbols: FxHashSet<ApiStabilitySymbolKey>,
    pub(crate) active_types: FxHashSet<ApiStabilityTypeKey>,
    pub(crate) active_components: FxHashSet<ApiStabilityTypeKey>,
    pub(crate) active_signatures: FxHashSet<ApiStabilitySignatureKey>,
    pub(crate) active_aliases: FxHashSet<ApiStabilityAliasKey>,
    pub(crate) active_conditionals: FxHashSet<ApiStabilityAliasKey>,

    pub(crate) signature_args: FxHashMap<P<Signature>, Vec<P<Type>>>,
    pub(crate) signature_parameter_cache: FxHashMap<P<Signature>, FxHashMap<P<Type>, P<Type>>>,

    // safetySafeMemo records completed Safe guard verdicts for this analysis,
    // keyed by the concrete read identity (operation, represented identity and
    // interned substitution carrier). Only Safe is ever recorded: Unknown,
    // recursion and exhaustion stay retryable by later scans and analyses.
    pub(crate) safety_safe_memo: FxHashSet<ApiStabilitySafetyVerdictKey>,

    // typeNameSymbols caches successful symbol lookups for raw type-name,
    // heritage, qualifier and declaration-name nodes by AST identity. A symbol
    // lookup at a node is a pure declaration/scope lookup that does not depend
    // on type bindings; no resolved type is ever cached here.
    pub(crate) type_name_symbols: FxHashMap<P<Node>, P<Symbol>>,

    // Optional tagged base eligibility depends only on declaration metadata.
    pub(crate) optional_tagged_symbols: FxHashSet<Option<P<Symbol>>>,

    pub(crate) work: i64,
    pub(crate) safety_work: i64,

    // materializationReads records the lazy reads whose checker accessor already
    // advanced the materialization epoch in this analysis, keyed by the read
    // operation and its concrete symbol or annotation node. Re-reading the same
    // component cannot materialize anything new (the checker caches it, or the
    // call is a declaration-only lookup), so the epoch only advances once per
    // identity. Without this, a repeated refused or cached lookup would keep
    // every blocked result stale and settlement would recollect surfaces that
    // provably cannot advance.
    pub(crate) materialization_reads: FxHashSet<ApiStabilityMaterializationReadKey>,

    // materializationEpoch advances whenever this analysis performs a lazy
    // checker read that can materialize represented components (a guarded read
    // that the pre-flight guard authorized, an inferred read the checker's own
    // check lifecycle allows, or a nested read a guard verification performed)
    // and whenever a settle round changes any surface. A blocked surface that
    // was last collected before the latest advancement may have become
    // collectable or may have a newly changed represented dependency; a surface
    // collected after it provably observed the current state.
    pub(crate) materialization_epoch: i64,
}

// go: internal/typeparser/api_stability.go:1219 apiStabilityMaxWork f71286b93aefba4e
// apiStabilityMaxWork bounds one analysis. The guard exists only so a
// pathological represented graph cannot run away; ordinary surfaces with
// deeply nested explicit components stay far below it. Exhausting the bound
// marks the result incomplete, which keeps it out of every cache.
pub(crate) const API_STABILITY_MAX_WORK: i64 = 100_000;
// apiStabilityMaxSettleRounds bounds the cycle fixpoint. Each round can only
// add findings, so convergence is monotone; the bound is a safety net.
pub(crate) const API_STABILITY_MAX_SETTLE_ROUNDS: i64 = 64;
// apiStabilityMaxSettleSweeps bounds the extra rounds an exhausted analysis
// runs after the fixpoint converges. A recollection in the final round may
// itself have advanced the epoch, making earlier results stale; the extra
// rounds recollect them and stop as soon as a round changes nothing. A
// result they do not recover stays blocked, and the analysis discards it
// with its session, so a later export or call retries it.
pub(crate) const API_STABILITY_MAX_SETTLE_SWEEPS: i64 = 4;

// go: internal/typeparser/api_stability.go:1237 newApiStabilityAnalysis c3bebb389b4795a0
// Rust: Go stores the parser; the Rust analysis receives it with every call (see `ApiStabilitySession`).
pub(crate) fn new_api_stability_analysis() -> ApiStabilityAnalysis {
    ApiStabilityAnalysis::default()
}

impl ApiStabilityAnalysis {
    // go: internal/typeparser/api_stability.go:1260 apiStabilityAnalysis.reset 271d5fce44fd1d16
    // reset returns the analysis to the state newApiStabilityAnalysis produces,
    // keeping map and slice capacity. Every per-export field must be cleared here.
    pub(crate) fn reset(&mut self) {
        self.session_symbols.clear();
        self.session_types.clear();
        self.session_components.clear();
        self.session_signatures.clear();
        self.type_frames.clear();
        self.signature_frames.clear();
        self.carriers.clear();
        self.settle_symbols.clear();
        self.settle_types.clear();
        self.settle_components.clear();
        self.settle_signatures.clear();
        self.scope.reset();
        self.publish_types.clear();
        self.publish_signatures.clear();
        self.active_symbols.clear();
        self.active_types.clear();
        self.active_components.clear();
        self.active_signatures.clear();
        self.active_aliases.clear();
        self.active_conditionals.clear();
        self.signature_args.clear();
        self.signature_parameter_cache.clear();
        self.safety_safe_memo.clear();
        self.type_name_symbols.clear();
        self.optional_tagged_symbols.clear();
        self.work = 0;
        self.safety_work = 0;
        self.materialization_reads.clear();
        self.materialization_epoch = 0;
    }

    // go: internal/typeparser/api_stability.go:1292 apiStabilityAnalysis.consumeWork 08cac84afd1e59a6
    pub(crate) fn consume_work(&mut self) -> bool {
        self.work += 1;
        self.work <= API_STABILITY_MAX_WORK
    }

    // go: internal/typeparser/api_stability.go:1329 apiStabilityAnalysis.settle eb4134475728a05c
    // settle resolves cycle cuts with a bounded monotone fixpoint, then marks
    // every result that was established complete as settled within this analysis.
    // Findings are recomputed until they stop changing; a recomputed surface is
    // unioned into the stored one so a dependency that is itself still settling can
    // never erase a finding an earlier round established. Results whose only
    // incompleteness came from cycle back-edges are then established complete,
    // because a back-edge's own direct findings are collected when the cut target
    // is first entered. Results that remain blocked (unavailable components or an
    // unevaluable conditional outcome) are never promoted, so a partial result is
    // never reported as stable; a later export or call retries from a new session.
    //
    // After the fixpoint converges, an exhausted analysis runs bounded extra
    // rounds. A recollection in the final round may itself have materialized a
    // component or advanced the epoch, making results collected earlier stale
    // again; the extra rounds recollect exactly those stale results. A blocked
    // result can only gain findings this way; it is never promoted. A result the
    // extra rounds do not recover stays blocked, and the analysis discards it with
    // its session, so a later export or call retries it.
    //
    // Once settling is done, every complete, context-free type and signature
    // result is published to the per-checker shared caches. A result that is still
    // in progress, blocked or cycle-cut before promotion is never published; a
    // promoted cycle result is published only here, after the fixpoint established
    // that its findings no longer change.
    //
    // The fixpoint only revisits the settle scope: the incomplete results the
    // current query stored or read since the last converged settle, plus whatever
    // their recollections reach. A session that answers many queries (every member
    // of a namespace export) would otherwise recollect every result an earlier
    // query left blocked once per later query. A result outside the scope stays as
    // it is until a later query reads it, which enrolls it again. An unconverged
    // fixpoint keeps its scope so the next settle retries the same cycle cuts.
    pub(crate) fn settle(&mut self, tp: &mut TypeParser<'_>) {
        self.reconcile_settle_order();
        let mut converged = true;
        if !self.scope.empty() {
            converged = false;
            for _ in 0..API_STABILITY_MAX_SETTLE_ROUNDS {
                if !self.settle_round(tp) {
                    converged = true;
                    break;
                }
                // A change to any surface can propagate into a result that was
                // collected earlier; advancing the epoch makes exactly those
                // earlier results stale, while a result collected after the change
                // has already observed it.
                self.materialization_epoch += 1;
            }
            if converged {
                if self.safety_budget_exhausted() {
                    for _ in 0..API_STABILITY_MAX_SETTLE_SWEEPS {
                        if !self.settle_round(tp) {
                            break;
                        }
                        // A recovery round can change a surface exactly like a
                        // fixpoint round: earlier blocked parents may now be
                        // stale, so they must be revisited within the bounded
                        // sweeps. This is the same propagation the fixpoint loop
                        // performs; it authorizes no additional guard read.
                        self.materialization_epoch += 1;
                    }
                }
                self.promote_settled();
            }
        }
        self.publish_complete_surfaces(tp);
        if converged {
            self.scope.reset();
        }
    }

    // go: internal/typeparser/api_stability.go:1370 apiStabilityAnalysis.storeSymbolSurface 76551cf59a942de3
    // storeSymbolSurface records a symbol result and its first-insertion order and
    // enrolls an incomplete result in the settle scope.
    pub(crate) fn store_symbol_surface(&mut self, symbol: P<Symbol>, surface: ApiStabilitySurface) {
        if !self.session_symbols.contains_key(&symbol) {
            self.settle_symbols.push(symbol);
        }
        let complete = surface.complete;
        self.session_symbols.insert(symbol, surface);
        self.enroll_symbol(symbol, complete);
    }

    // go: internal/typeparser/api_stability.go:1380 apiStabilityAnalysis.storeTypeSurface 174b0a6630a8a6c2
    // storeTypeSurface records a type result and its first-insertion order and
    // enrolls it for settlement or publication.
    pub(crate) fn store_type_surface(
        &mut self,
        key: ApiStabilityTypeKey,
        surface: ApiStabilitySurface,
    ) {
        if !self.session_types.contains_key(&key) {
            self.settle_types.push(key);
        }
        let complete = surface.complete;
        self.session_types.insert(key, surface);
        self.enroll_type(key, complete);
    }

    // go: internal/typeparser/api_stability.go:1390 apiStabilityAnalysis.storeComponentSurface 9e5000b4c06e6598
    // storeComponentSurface records an object-component result and its
    // first-insertion order and enrolls an incomplete result in the settle scope.
    pub(crate) fn store_component_surface(
        &mut self,
        key: ApiStabilityTypeKey,
        surface: ApiStabilitySurface,
    ) {
        if !self.session_components.contains_key(&key) {
            self.settle_components.push(key);
        }
        let complete = surface.complete;
        self.session_components.insert(key, surface);
        self.enroll_component(key, complete);
    }

    // go: internal/typeparser/api_stability.go:1400 apiStabilityAnalysis.storeSignatureSurface 9a6d732e867f677d
    // storeSignatureSurface records a signature result and its first-insertion
    // order and enrolls it for settlement or publication.
    pub(crate) fn store_signature_surface(
        &mut self,
        key: ApiStabilitySignatureKey,
        surface: ApiStabilitySurface,
    ) {
        if !self.session_signatures.contains_key(&key) {
            self.settle_signatures.push(key);
        }
        let complete = surface.complete;
        self.session_signatures.insert(key, surface);
        self.enroll_signature(key, complete);
    }

    // go: internal/typeparser/api_stability.go:1408 apiStabilityAnalysis.enrollSymbol 8f3010d46b7174eb
    // Rust: the enroll helpers take the surface's `complete` flag, the only part of the surface Go reads.
    pub(crate) fn enroll_symbol(&mut self, symbol: P<Symbol>, complete: bool) {
        if !complete {
            self.scope.symbols.add(symbol);
        }
    }

    // go: internal/typeparser/api_stability.go:1416 apiStabilityAnalysis.enrollType 924e6219b97cb449
    // enrollType puts an incomplete type result in the settle scope and queues a
    // complete context-free one for publication.
    pub(crate) fn enroll_type(&mut self, key: ApiStabilityTypeKey, complete: bool) {
        if !complete {
            self.scope.types.add(key);
        } else if key.carrier.is_none() {
            self.publish_types.push(key);
        }
    }

    // go: internal/typeparser/api_stability.go:1425 apiStabilityAnalysis.enrollComponent 1c1bb0f7cd59c858
    pub(crate) fn enroll_component(&mut self, key: ApiStabilityTypeKey, complete: bool) {
        if !complete {
            self.scope.components.add(key);
        }
    }

    // go: internal/typeparser/api_stability.go:1432 apiStabilityAnalysis.enrollSignature 64ee7903c348fb90
    // enrollSignature is enrollType for a signature result.
    pub(crate) fn enroll_signature(&mut self, key: ApiStabilitySignatureKey, complete: bool) {
        if !complete {
            self.scope.signatures.add(key);
        } else if key.carrier.is_none() {
            self.publish_signatures.push(key);
        }
    }

    // go: internal/typeparser/api_stability.go:1447 apiStabilityAnalysis.restoreOwnTagFinding c232766b4325cf21
    // restoreOwnTagFinding re-adds one finding a published snapshot removed as the
    // component's own declared tag. The declared level and provenance come from
    // the per-checker declared caches — the snapshot stores only the concrete
    // identity — so composition is always "declared separately". The removed key
    // was produced by exactly one of the three record helpers, which re-derives
    // the same level and declaration from the same caches.
    pub(crate) fn restore_own_tag_finding(
        &self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        key: ApiStabilityFindingKey,
    ) {
        if key.signature.is_some() {
            self.record_signature_stability(tp, surface, key.signature);
        } else if key.symbol.is_some() {
            self.record_symbol(tp, surface, key.symbol);
        } else if key.declaration.is_some() {
            self.record_declaration_stability(tp, surface, key.declaration);
        }
    }

    // go: internal/typeparser/api_stability.go:1463 apiStabilityAnalysis.sharedTypeSurface 02e9e485131983ce
    // sharedTypeSurface returns a fresh analysis-local copy of the published
    // complete surface of a concrete context-free type, re-adding the component's
    // own declared-tag findings from the declared caches. It reports false when no
    // entry exists, so the caller computes locally. The copy keeps the published
    // snapshot immutable while the caller composes and mutates its own surface.
    pub(crate) fn shared_type_surface(
        &self,
        tp: &mut TypeParser<'_>,
        t: Option<P<Type>>,
        inspect: ApiStabilityInspection,
    ) -> Option<ApiStabilitySurface> {
        let t = t?;
        let entry = tp
            .links()
            .api_stability_surface_type
            .get(&ApiStabilitySurfaceTypeKey { t, inspect })
            .cloned()?;
        let mut surface = new_api_stability_surface();
        surface.findings = Rc::new(entry.findings);
        for key in entry.removed {
            self.restore_own_tag_finding(tp, &mut surface, key);
        }
        Some(surface)
    }

    // go: internal/typeparser/api_stability.go:1481 apiStabilityAnalysis.sharedSignatureSurface fdc6c797088d76b7
    // sharedSignatureSurface is sharedTypeSurface for a concrete signature
    // identity.
    pub(crate) fn shared_signature_surface(
        &self,
        tp: &mut TypeParser<'_>,
        signature: Option<P<Signature>>,
    ) -> Option<ApiStabilitySurface> {
        let signature = signature?;
        let entry = tp.links().api_stability_surface_signature.get(&signature).cloned()?;
        let mut surface = new_api_stability_surface();
        surface.findings = Rc::new(entry.findings);
        for key in entry.removed {
            self.restore_own_tag_finding(tp, &mut surface, key);
        }
        Some(surface)
    }

    // go: internal/typeparser/api_stability.go:1506 apiStabilityAnalysis.publishTypeSurface 7a5e13b567768c77
    // publishTypeSurface publishes one complete, settled, context-free type surface
    // as an immutable root-excluding snapshot. Every finding that matches the
    // component's own declared tag is removed and its concrete identity recorded;
    // the declared level stays in the per-checker declared caches. An in-progress
    // result is never stored; a cycle-cut result only reaches complete after the
    // settle fixpoint promoted it; a blocked or otherwise incomplete result is
    // never published. The first entry for a key wins: complete recomputations over
    // the same checker state agree, so overwriting could not add information and
    // the snapshot stays deterministic.
    pub(crate) fn publish_type_surface(
        &self,
        tp: &mut TypeParser<'_>,
        t: Option<P<Type>>,
        inspect: ApiStabilityInspection,
        surface: &ApiStabilitySurface,
    ) {
        let Some(t) = t else {
            return;
        };
        if !surface.complete || surface.cut_cycle || surface.blocked {
            return;
        }
        let key = ApiStabilitySurfaceTypeKey { t, inspect };
        if tp.links().api_stability_surface_type.contains_key(&key) {
            return;
        }
        let own = self.type_own_tag(tp, Some(t));
        let mut entry = ApiStabilitySharedSurface::default();
        for (finding_key, finding) in surface.findings.iter() {
            if own.excludes(finding_key, *finding) {
                entry.removed.push(*finding_key);
                continue;
            }
            entry.findings.insert(*finding_key, *finding);
        }
        tp.links().api_stability_surface_type.insert(key, entry);
    }

    // go: internal/typeparser/api_stability.go:1533 apiStabilityAnalysis.publishSignatureSurface 702dd9ed607b98a2
    // publishSignatureSurface publishes one complete, settled, context-free
    // signature surface with the signature's own declared tag removed exactly like
    // publishTypeSurface does for a type.
    pub(crate) fn publish_signature_surface(
        &self,
        tp: &mut TypeParser<'_>,
        signature: Option<P<Signature>>,
        surface: &ApiStabilitySurface,
    ) {
        let Some(signature) = signature else {
            return;
        };
        if !surface.complete || surface.cut_cycle || surface.blocked {
            return;
        }
        if tp.links().api_stability_surface_signature.contains_key(&signature) {
            return;
        }
        let own = self.signature_own_tag(tp, Some(signature));
        let mut entry = ApiStabilitySharedSurface::default();
        for (finding_key, finding) in surface.findings.iter() {
            if own.excludes(finding_key, *finding) {
                entry.removed.push(*finding_key);
                continue;
            }
            entry.findings.insert(*finding_key, *finding);
        }
        tp.links().api_stability_surface_signature.insert(signature, entry);
    }

    // go: internal/typeparser/api_stability.go:1565 apiStabilityAnalysis.publishCompleteSurfaces 79c6de0c49a60bda
    // publishCompleteSurfaces pushes the complete, settled, context-free session
    // results queued since the last settle into the per-checker shared caches. It
    // runs after the settle fixpoint (including cycle promotion) so an in-progress
    // or cycle-cut surface is never published. Only stores of complete results and
    // promotions queue a key, and only for results without a substitution carrier,
    // so a session that answers many queries does not revisit the results every
    // earlier query already published. Symbol surfaces stay analysis-local: a symbol
    // root composes its own declared identity with the reusable concrete type and
    // signature surfaces.
    pub(crate) fn publish_complete_surfaces(&mut self, tp: &mut TypeParser<'_>) {
        let publish_types = std::mem::take(&mut self.publish_types);
        for &key in &publish_types {
            let surface = self.session_types.get(&key).cloned().unwrap_or_default();
            self.publish_type_surface(tp, Some(key.t), key.inspect, &surface);
        }
        let publish_signatures = std::mem::take(&mut self.publish_signatures);
        for &key in &publish_signatures {
            let surface = self.session_signatures.get(&key).cloned().unwrap_or_default();
            self.publish_signature_surface(tp, Some(key.signature), &surface);
        }
        // Rust: the queues were taken above; hand their allocations back, emptied (Go's `[:0]`).
        self.publish_types = publish_types;
        self.publish_signatures = publish_signatures;
        self.publish_types.clear();
        self.publish_signatures.clear();
    }
}

// go: internal/typeparser/api_stability.go:1579 apiStabilityNodeVisitKey 7d5e3965281302ad
// apiStabilityNodeVisitKey renders a stable visit key for a declaration node.
pub(crate) fn api_stability_node_visit_key(node: Option<P<Node>>) -> String {
    let Some(node) = node else {
        return String::new();
    };
    let mut file = String::new();
    if let Some(source_file) = ets_shim::ast::get_source_file_of_node(node) {
        file = source_file.file_name().to_string();
    }
    format!("{file}#{}", node_pos(node))
}

/// Rust-only: Go's `node.Pos()` as the `int` Go formats (a synthesized node is Go's -1).
fn node_pos(node: P<Node>) -> i64 {
    node.pos().source_offset().map_or(-1, i64::from)
}

// go: internal/typeparser/api_stability.go:1591 apiStabilitySymbolVisitKey 7afa3fc51468cf6f
// apiStabilitySymbolVisitKey renders a stable visit key for a symbol result.
pub(crate) fn api_stability_symbol_visit_key(symbol: Option<P<Symbol>>) -> String {
    let Some(symbol) = symbol else {
        return String::new();
    };
    if let Some(&declaration) = symbol.declarations().first() {
        return format!("{}@{}", symbol.name(), api_stability_node_visit_key(Some(declaration)));
    }
    symbol.name().to_string()
}

// go: internal/typeparser/api_stability.go:1606 apiStabilityTypeVisitKey 16d0089d619ac717
// apiStabilityTypeVisitKey renders a best-effort stable visit key for a
// represented type result. It only orders the rare results that settlement had
// to reconcile because they were stored without the store helpers.
pub(crate) fn api_stability_type_visit_key(key: &ApiStabilityTypeKey) -> String {
    let t = key.t;
    let mut name = "";
    if let Some(symbol) = t.symbol() {
        name = symbol.name();
    }
    if name.is_empty()
        && let Some(alias) = t.alias()
        && let Some(alias_symbol) = alias.symbol()
    {
        name = alias_symbol.name();
    }
    if name.is_empty()
        && t.object_flags().intersects(ObjectFlags::Reference)
        && let Some(target) = t.target()
        && let Some(symbol) = target.symbol()
    {
        name = symbol.name();
    }
    format!("{name}@{}", key.inspect as i64)
}

// go: internal/typeparser/api_stability.go:1628 apiStabilitySignatureVisitKey 1e1c1973095f36b8
// apiStabilitySignatureVisitKey renders a stable visit key for a signature
// result.
pub(crate) fn api_stability_signature_visit_key(key: &ApiStabilitySignatureKey) -> String {
    let Some(raw) = raw_signature(Some(key.signature)) else {
        return String::new();
    };
    api_stability_node_visit_key(raw.declaration())
}

impl ApiStabilityAnalysis {
    // go: internal/typeparser/api_stability.go:1646 apiStabilityAnalysis.reconcileSettleOrder 5d7f63f47e852673
    // reconcileSettleOrder enrolls session results that were stored without the
    // store helpers, so settlement still visits them. Production results always go
    // through the helpers, whose size fast path keeps the order slices in sync; an
    // analysis-local seeded result (a test that installs a cycle cut directly) is
    // appended here in a canonical order, and enrolled in the settle scope or the
    // publication queues exactly like a stored result, so the visit sequence stays
    // deterministic.
    pub(crate) fn reconcile_settle_order(&mut self) {
        if self.session_symbols.len() != self.settle_symbols.len() {
            let seen: FxHashSet<P<Symbol>> = self.settle_symbols.iter().copied().collect();
            let mut missing: Vec<P<Symbol>> =
                self.session_symbols.keys().copied().filter(|s| !seen.contains(s)).collect();
            ets_go::sort::slice(&mut missing, |left, right| {
                api_stability_symbol_visit_key(Some(*left))
                    < api_stability_symbol_visit_key(Some(*right))
            });
            self.settle_symbols.extend_from_slice(&missing);
            for symbol in missing {
                let complete = self.session_symbols.get(&symbol).is_some_and(|s| s.complete);
                self.enroll_symbol(symbol, complete);
            }
        }
        if self.session_types.len() != self.settle_types.len() {
            let seen: FxHashSet<ApiStabilityTypeKey> = self.settle_types.iter().copied().collect();
            let mut missing: Vec<ApiStabilityTypeKey> =
                self.session_types.keys().copied().filter(|k| !seen.contains(k)).collect();
            ets_go::sort::slice(&mut missing, |left, right| {
                api_stability_type_visit_key(left) < api_stability_type_visit_key(right)
            });
            self.settle_types.extend_from_slice(&missing);
            for key in missing {
                let complete = self.session_types.get(&key).is_some_and(|s| s.complete);
                self.enroll_type(key, complete);
            }
        }
        if self.session_components.len() != self.settle_components.len() {
            let seen: FxHashSet<ApiStabilityTypeKey> =
                self.settle_components.iter().copied().collect();
            let mut missing: Vec<ApiStabilityTypeKey> =
                self.session_components.keys().copied().filter(|k| !seen.contains(k)).collect();
            ets_go::sort::slice(&mut missing, |left, right| {
                api_stability_type_visit_key(left) < api_stability_type_visit_key(right)
            });
            self.settle_components.extend_from_slice(&missing);
            for key in missing {
                let complete = self.session_components.get(&key).is_some_and(|s| s.complete);
                self.enroll_component(key, complete);
            }
        }
        if self.session_signatures.len() != self.settle_signatures.len() {
            let seen: FxHashSet<ApiStabilitySignatureKey> =
                self.settle_signatures.iter().copied().collect();
            let mut missing: Vec<ApiStabilitySignatureKey> =
                self.session_signatures.keys().copied().filter(|k| !seen.contains(k)).collect();
            ets_go::sort::slice(&mut missing, |left, right| {
                api_stability_signature_visit_key(left) < api_stability_signature_visit_key(right)
            });
            self.settle_signatures.extend_from_slice(&missing);
            for key in missing {
                let complete = self.session_signatures.get(&key).is_some_and(|s| s.complete);
                self.enroll_signature(key, complete);
            }
        }
    }

    // go: internal/typeparser/api_stability.go:1734 apiStabilityAnalysis.settleRound 5c8da41104481b4b
    // settleRound recollects every incomplete surface in the settle scope once and
    // reports whether any surface changed. Complete results are always skipped: a
    // completed result is never recomputed. A blocked result is skipped only when
    // it was already collected at the current advancement epoch, so a materializing
    // read or a surface change that happened after its last collection is always
    // revisited. Results are visited in the order the query reached them so the
    // bounded fixpoint spends its rounds deterministically. A recollection that
    // stores or reads another incomplete result enrolls it in the scope, and the
    // round visits it before returning, whichever kind it is.
    pub(crate) fn settle_round(&mut self, tp: &mut TypeParser<'_>) -> bool {
        let mut changed = false;
        let (mut types, mut components, mut signatures, mut symbols) = (0, 0, 0, 0);
        // The scope lists grow while a round runs, so their lengths are read on
        // every iteration instead of being snapshotted.
        while types < self.scope.types.keys.len()
            || components < self.scope.components.keys.len()
            || signatures < self.scope.signatures.keys.len()
            || symbols < self.scope.symbols.keys.len()
        {
            while types < self.scope.types.keys.len() {
                if self.settle_type(tp, self.scope.types.keys[types]) {
                    changed = true;
                }
                types += 1;
            }
            while components < self.scope.components.keys.len() {
                if self.settle_component(tp, self.scope.components.keys[components]) {
                    changed = true;
                }
                components += 1;
            }
            while signatures < self.scope.signatures.keys.len() {
                if self.settle_signature(tp, self.scope.signatures.keys[signatures]) {
                    changed = true;
                }
                signatures += 1;
            }
            while symbols < self.scope.symbols.keys.len() {
                if self.settle_symbol(tp, self.scope.symbols.keys[symbols]) {
                    changed = true;
                }
                symbols += 1;
            }
        }
        changed
    }

    // go: internal/typeparser/api_stability.go:1767 apiStabilityAnalysis.settleType a36c015d7f8d4d66
    // settleType recollects one incomplete type result and reports whether its
    // surface changed.
    pub(crate) fn settle_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        key: ApiStabilityTypeKey,
    ) -> bool {
        let surface = self.session_types.get(&key).cloned().unwrap_or_default();
        if surface.complete || self.settle_cannot_advance(&surface) {
            return false;
        }
        let start_epoch = self.materialization_epoch;
        let frame = self.type_frames.get(&key).cloned().flatten();
        let recomputed = self.collect_type_surface(
            tp,
            Some(key.t),
            key.inspect,
            &ApiStabilitySubstitution { frame },
        );
        let mut merged = grow_api_stability_surface(&surface, &recomputed);
        let changed = !merged.equals(&surface);
        merged.observed_epoch = start_epoch;
        self.session_types.insert(key, merged);
        changed
    }

    // go: internal/typeparser/api_stability.go:1782 apiStabilityAnalysis.settleComponent 0a4ee14ffac1a03b
    // settleComponent is settleType for an object-component result.
    pub(crate) fn settle_component(
        &mut self,
        tp: &mut TypeParser<'_>,
        key: ApiStabilityTypeKey,
    ) -> bool {
        let surface = self.session_components.get(&key).cloned().unwrap_or_default();
        if surface.complete || self.settle_cannot_advance(&surface) {
            return false;
        }
        let start_epoch = self.materialization_epoch;
        let frame = self.type_frames.get(&key).cloned().flatten();
        let recomputed = self.collect_object_component_surface(
            tp,
            Some(key.t),
            key.inspect,
            &ApiStabilitySubstitution { frame },
        );
        let mut merged = grow_api_stability_surface(&surface, &recomputed);
        let changed = !merged.equals(&surface);
        merged.observed_epoch = start_epoch;
        self.session_components.insert(key, merged);
        changed
    }

    // go: internal/typeparser/api_stability.go:1797 apiStabilityAnalysis.settleSignature 092743f52be6b9b8
    // settleSignature is settleType for a signature result.
    pub(crate) fn settle_signature(
        &mut self,
        tp: &mut TypeParser<'_>,
        key: ApiStabilitySignatureKey,
    ) -> bool {
        let surface = self.session_signatures.get(&key).cloned().unwrap_or_default();
        if surface.complete || self.settle_cannot_advance(&surface) {
            return false;
        }
        let start_epoch = self.materialization_epoch;
        let frame = self.signature_frames.get(&key).cloned().flatten();
        let recomputed = self.collect_signature_surface(
            tp,
            raw_signature(Some(key.signature)),
            Some(key.signature),
            &ApiStabilitySubstitution { frame },
        );
        let mut merged = grow_api_stability_surface(&surface, &recomputed);
        let changed = !merged.equals(&surface);
        merged.observed_epoch = start_epoch;
        self.session_signatures.insert(key, merged);
        changed
    }

    // go: internal/typeparser/api_stability.go:1812 apiStabilityAnalysis.settleSymbol 6de632c5a74c00d5
    // settleSymbol is settleType for a symbol result.
    pub(crate) fn settle_symbol(&mut self, tp: &mut TypeParser<'_>, symbol: P<Symbol>) -> bool {
        let surface = self.session_symbols.get(&symbol).cloned().unwrap_or_default();
        if surface.complete || self.settle_cannot_advance(&surface) {
            return false;
        }
        let start_epoch = self.materialization_epoch;
        let recomputed = self.collect_symbol_surface(tp, symbol);
        let mut merged = grow_api_stability_surface(&surface, &recomputed);
        let changed = !merged.equals(&surface);
        merged.observed_epoch = start_epoch;
        self.session_symbols.insert(symbol, merged);
        changed
    }

    // go: internal/typeparser/api_stability.go:1838 apiStabilityAnalysis.settleCannotAdvance 44fb8b244a5d4ca0
    // settleCannotAdvance reports whether a fixpoint recollection of an incomplete
    // surface provably cannot add anything. A blocked surface is only skipped once
    // the guard budget is exhausted and the surface was last collected at the
    // current advancement epoch: no later analysis read could have materialized one
    // of its components since, and no later surface change could have propagated a
    // represented dependency finding into it. The surface stays blocked and is
    // never promoted; a later export or call retries it after the checker
    // materializes more of the program; an exhausted analysis also runs bounded
    // extra rounds after the fixpoint converges, so a result that a final
    // recollection made stale is revisited. Cycle-cut surfaces always keep
    // recomputing: represented dependencies can still propagate even while the
    // guard refuses new reads.
    pub(crate) fn settle_cannot_advance(&self, surface: &ApiStabilitySurface) -> bool {
        if !surface.blocked || !self.safety_budget_exhausted() {
            return false;
        }
        surface.observed_epoch == self.materialization_epoch
    }

    // go: internal/typeparser/api_stability.go:1849 apiStabilityAnalysis.noteMaterializingRead 585a5e246df0286a
    // noteMaterializingRead records that the analysis is about to perform a lazy
    // checker read that can materialize represented components. Settlement uses the
    // resulting epoch to decide whether a blocked result may have become
    // collectable without authorizing any new guard read.
    pub(crate) fn note_materializing_read(&mut self) {
        self.materialization_epoch += 1;
    }

    // go: internal/typeparser/api_stability.go:1857 apiStabilityAnalysis.noteMaterializingSymbolRead f20866b0455c2b83
    // noteMaterializingSymbolRead advances the materialization epoch for the first
    // lazy read of one symbol operation and is a no-op for repeats: a repeat cannot
    // materialize anything new, and letting it keep the epoch moving would mark
    // every blocked result stale forever.
    pub(crate) fn note_materializing_symbol_read(
        &mut self,
        kind: ApiStabilityMaterializationReadKind,
        symbol: Option<P<Symbol>>,
    ) {
        let key = ApiStabilityMaterializationReadKey { kind, symbol, node: None };
        if self.materialization_reads.contains(&key) {
            return;
        }
        self.materialization_reads.insert(key);
        self.note_materializing_read();
    }

    // go: internal/typeparser/api_stability.go:1871 apiStabilityAnalysis.noteMaterializingNodeRead 94b930d813d6f2b4
    // noteMaterializingNodeRead advances the materialization epoch for the first
    // lazy read of one annotation node and is a no-op for repeats.
    pub(crate) fn note_materializing_node_read(&mut self, node: Option<P<Node>>) {
        let key = ApiStabilityMaterializationReadKey {
            kind: ApiStabilityMaterializationReadKind::NodeType,
            symbol: None,
            node,
        };
        if self.materialization_reads.contains(&key) {
            return;
        }
        self.materialization_reads.insert(key);
        self.note_materializing_read();
    }

    // go: internal/typeparser/api_stability.go:1886 apiStabilityAnalysis.materializedMembersOfSymbol 364a09e33ce91ae1
    // materializedMembersOfSymbol resolves a symbol's member table through the
    // checker's ordinary lazy accessor after the caller established that the read
    // is safe. The read is recorded as materializing.
    pub(crate) fn materialized_members_of_symbol(
        &mut self,
        c: &mut Checker,
        symbol: P<Symbol>,
    ) -> Option<P<SymbolTable>> {
        self.note_materializing_symbol_read(
            ApiStabilityMaterializationReadKind::Members,
            Some(symbol),
        );
        c.get_members_of_symbol(symbol)
    }

    // go: internal/typeparser/api_stability.go:1894 apiStabilityAnalysis.materializedSignaturesOfSymbol 49e24be7a642051f
    // materializedSignaturesOfSymbol resolves a signature member's signatures
    // through the checker's ordinary lazy accessor after the caller established
    // that the read is safe. The read is recorded as materializing.
    pub(crate) fn materialized_signatures_of_symbol(
        &mut self,
        c: &mut Checker,
        symbol: P<Symbol>,
    ) -> Vec<P<Signature>> {
        self.note_materializing_symbol_read(
            ApiStabilityMaterializationReadKind::Signatures,
            Some(symbol),
        );
        c.get_signatures_of_symbol(Some(symbol))
    }
}

// go: internal/typeparser/api_stability.go:1906 growApiStabilitySurface 9aac8516702bfb88
// growApiStabilitySurface unions a recomputed surface into the previously
// stored one. Findings only grow across fixpoint rounds, so a result
// established in an earlier round is never lost when a later round observes a
// dependency that is itself still settling; incompleteness is latched until
// promotion, and a blocked result is never promoted. The merged surface shares
// the stored findings map when the recomputation adds nothing new; otherwise it
// gets a fresh map, so the stored map is never written.
pub(crate) fn grow_api_stability_surface(
    previous: &ApiStabilitySurface,
    recomputed: &ApiStabilitySurface,
) -> ApiStabilitySurface {
    let mut merged = previous.clone();
    for (key, finding) in recomputed.findings.iter() {
        if let Some(existing) = previous.findings.get(key)
            && finding.level <= existing.level
        {
            continue;
        }
        let mut findings = ApiStabilityFindings::default();
        findings.reserve(previous.findings.len() + recomputed.findings.len());
        findings.extend(previous.findings.iter().map(|(k, v)| (*k, *v)));
        merged.findings = Rc::new(findings);
        for (&recomputed_key, &recomputed_finding) in recomputed.findings.iter() {
            merged.add(recomputed_key, recomputed_finding);
        }
        break;
    }
    if !recomputed.complete {
        merged.complete = false;
        merged.cut_cycle = merged.cut_cycle || recomputed.cut_cycle;
        merged.blocked = merged.blocked || recomputed.blocked;
    }
    merged
}

impl ApiStabilityAnalysis {
    // go: internal/typeparser/api_stability.go:1932 apiStabilityAnalysis.promoteSettled 3e18d48e5565473a
    // promoteSettled marks the cycle-cut results in the settle scope as complete
    // after the fixpoint converged and queues the context-free ones for
    // publication. A blocked result is never promoted. A cycle-cut result outside
    // the scope was not reached by this query and stays cut until a later query
    // reads it.
    pub(crate) fn promote_settled(&mut self) {
        for i in 0..self.scope.symbols.keys.len() {
            let symbol = self.scope.symbols.keys[i];
            let mut surface = self.session_symbols.get(&symbol).cloned().unwrap_or_default();
            if !surface.complete && !surface.blocked {
                surface.complete = true;
                surface.cut_cycle = false;
                self.session_symbols.insert(symbol, surface);
            }
        }
        for i in 0..self.scope.types.keys.len() {
            let key = self.scope.types.keys[i];
            let mut surface = self.session_types.get(&key).cloned().unwrap_or_default();
            if !surface.complete && !surface.blocked {
                surface.complete = true;
                surface.cut_cycle = false;
                self.session_types.insert(key, surface);
                if key.carrier.is_none() {
                    self.publish_types.push(key);
                }
            }
        }
        for i in 0..self.scope.components.keys.len() {
            let key = self.scope.components.keys[i];
            let mut surface = self.session_components.get(&key).cloned().unwrap_or_default();
            if !surface.complete && !surface.blocked {
                surface.complete = true;
                surface.cut_cycle = false;
                self.session_components.insert(key, surface);
            }
        }
        for i in 0..self.scope.signatures.keys.len() {
            let key = self.scope.signatures.keys[i];
            let mut surface = self.session_signatures.get(&key).cloned().unwrap_or_default();
            if !surface.complete && !surface.blocked {
                surface.complete = true;
                surface.cut_cycle = false;
                self.session_signatures.insert(key, surface);
                if key.carrier.is_none() {
                    self.publish_signatures.push(key);
                }
            }
        }
    }

    // go: internal/typeparser/api_stability.go:1968 apiStabilityAnalysis.symbolSurface 0b7804ad6be28a10
    // symbolSurface computes the dependency surface of an export symbol in this
    // analysis. A complete result is reused by later queries of the same session; a
    // result blocked by an unavailable component is kept, but never promoted.
    pub(crate) fn symbol_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: Option<P<Symbol>>,
    ) -> ApiStabilitySurface {
        let Some(symbol) = symbol else {
            return new_api_stability_surface();
        };
        if let Some(cached) = self.session_symbols.get(&symbol) {
            let cached = cached.clone();
            if !cached.complete {
                self.scope.symbols.add(symbol);
            }
            return cached;
        }
        let start_epoch = self.materialization_epoch;
        let mut surface = self.collect_symbol_surface(tp, symbol);
        if surface.blocked {
            surface.observed_epoch = start_epoch;
        }
        self.store_symbol_surface(symbol, surface.clone());
        surface
    }

    // go: internal/typeparser/api_stability.go:1996 apiStabilityAnalysis.collectSymbolSurface 9cdcaa279fef4504
    // collectSymbolSurface selects the represented sources of a symbol's public
    // surface. A value symbol's type, a class/interface/enum declared type and a
    // type alias's declared type are read through the checker: an already computed
    // result is preferred, and a public component the checker has not computed is
    // materialized through the ordinary lazy accessors. Materializing one public
    // type or signature does not recursively expand referenced types; shallow
    // named references still use their declaration surface only. Declared member
    // and signature `@stability` tags are metadata and are read from the
    // declaration structure without resolving any type.
    pub(crate) fn collect_symbol_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: P<Symbol>,
    ) -> ApiStabilitySurface {
        let mut surface = new_api_stability_surface();
        let empty = ApiStabilitySubstitution::default();
        let flags = symbol.flags();
        let mut handled = false;

        // A default (or `export =`) assignment of an inline expression has no alias
        // target. The expression's represented checker type is the public surface;
        // it is materialized lazily when the checker has not computed it yet,
        // unless the expression would expand a recursive alias.
        for &declaration in symbol.declarations() {
            if declaration.kind() != Kind::ExportAssignment {
                continue;
            }
            let expression = declaration.as_export_assignment().expression();
            let represented = self.type_from_node_safely(tp, Some(expression), &empty);
            if represented.is_some() {
                let s = self.type_surface(tp, represented, ApiStabilityInspection::Expand, &empty);
                surface.merge(&s);
                handled = true;
            }
        }

        if handled {
        } else if flags.intersects(SymbolFlags::TypeAlias) {
            if let Some(declared) = self.declared_type_of_symbol(tp, Some(symbol)) {
                let s =
                    self.type_surface(tp, Some(declared), ApiStabilityInspection::Expand, &empty);
                surface.merge(&s);
                handled = true;
            } else {
                handled = self.collect_type_alias_declaration_surface(
                    tp,
                    &mut surface,
                    Some(symbol),
                    &empty,
                );
            }
        } else if flags.intersects(SymbolFlags::Interface) {
            // Building an interface's declared type is lazy and never instantiates
            // a generic application; members stay unresolved until a member type is
            // requested.
            if let Some(declared) = self.declared_type_of_symbol(tp, Some(symbol)) {
                let s =
                    self.type_surface(tp, Some(declared), ApiStabilityInspection::Expand, &empty);
                surface.merge(&s);
                handled = true;
            }
        } else if flags.intersects(SymbolFlags::Class | SymbolFlags::Enum) {
            if let Some(declared) = self.declared_type_of_symbol(tp, Some(symbol)) {
                let s =
                    self.type_surface(tp, Some(declared), ApiStabilityInspection::Expand, &empty);
                surface.merge(&s);
                handled = true;
            }
            if let Some(value) = self.value_type_of_symbol(tp, Some(symbol)) {
                let s = self.type_surface(tp, Some(value), ApiStabilityInspection::Expand, &empty);
                surface.merge(&s);
            }
        } else if let Some(value) = self.value_type_of_symbol(tp, Some(symbol)) {
            let s = self.type_surface(tp, Some(value), ApiStabilityInspection::Expand, &empty);
            surface.merge(&s);
            handled = true;
        }

        if !handled {
            for &declaration in symbol.declarations() {
                let annotation = self.declaration_annotation_node(Some(declaration));
                self.collect_declared_tag_surface(tp, &mut surface, annotation);
            }
            surface.block();
        }

        // Erased alias provenance: pair each declaration's annotation nodes with
        // the represented type the checker recorded for its annotation. The walk
        // only records alias symbols that survive in the represented component.
        for &declaration in symbol.declarations() {
            if declaration.function_like_data().is_some() {
                continue;
            }
            let represented = self.declaration_represented_type(tp, symbol, declaration);
            self.collect_declaration_provenance(
                tp,
                &mut surface,
                Some(declaration),
                represented,
                &empty,
            );
        }
        surface
    }

    // go: internal/typeparser/api_stability.go:2080 apiStabilityAnalysis.typeOfSymbolSafely 0f69290c67ffa13d
    // typeOfSymbolSafely returns a symbol's checker type, preferring an already
    // computed result and otherwise materializing it through the ordinary lazy
    // accessor when the pre-flight recursion guard establishes that the resolution
    // is bounded. An inferred type (an initializer or a function body) is read
    // through the checker's own lazy inference: diagnostics inference produces are
    // attributed to the declaring file's expressions, exactly as that file's
    // ordinary check would attribute them, so the read neither consumes nor
    // duplicates them.
    pub(crate) fn type_of_symbol_safely(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: Option<P<Symbol>>,
    ) -> Option<P<Type>> {
        let symbol = symbol?;
        if let Some(cached) = get_resolved_type_of_symbol_if_materialized(tp.checker, Some(symbol))
        {
            return Some(cached);
        }
        if !self.symbol_type_resolution_is_safe(tp, Some(symbol)) {
            return None;
        }
        self.note_materializing_read();
        Some(tp.checker.get_type_of_symbol_exported(symbol))
    }

    // go: internal/typeparser/api_stability.go:2097 apiStabilityAnalysis.typeFromNodeSafely 2c48e89e6b47941c
    // typeFromNodeSafely returns the checker type of an annotation node, preferring
    // an already resolved node and otherwise resolving it through the ordinary lazy
    // accessor when the recursion guard establishes that the resolution is bounded.
    pub(crate) fn type_from_node_safely(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Option<P<Node>>,
        subst: &ApiStabilitySubstitution,
    ) -> Option<P<Type>> {
        let node = node?;
        if let Some(cached) = get_resolved_type_from_type_node(tp.checker, node) {
            return Some(cached);
        }
        if !self.annotation_resolution_is_safe(tp, Some(node), subst) {
            return None;
        }
        self.note_materializing_read();
        Some(tp.checker.get_type_from_type_node_exported(node))
    }

    // go: internal/typeparser/api_stability.go:2115 apiStabilityAnalysis.declaredTypeSafely 88e24fc6a30319b3
    // declaredTypeSafely returns a symbol's declared type, preferring an already
    // computed result and otherwise materializing it through the ordinary lazy
    // accessor when the recursion guard establishes that the declaration resolution
    // is bounded.
    pub(crate) fn declared_type_safely(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: Option<P<Symbol>>,
    ) -> Option<P<Type>> {
        let symbol = symbol?;
        if let Some(cached) =
            get_resolved_declared_type_of_symbol_if_materialized(tp.checker, Some(symbol))
        {
            return Some(cached);
        }
        if !self.declared_type_resolution_is_safe(tp, Some(symbol)) {
            return None;
        }
        self.note_materializing_symbol_read(
            ApiStabilityMaterializationReadKind::DeclaredType,
            Some(symbol),
        );
        Some(tp.checker.get_declared_type_of_symbol_exported(symbol))
    }

    // go: internal/typeparser/api_stability.go:2135 apiStabilityAnalysis.returnTypeSafely 6bc8dfc0c76b5aa6
    // returnTypeSafely returns a signature's return type, preferring an already
    // computed result and otherwise materializing it through the ordinary lazy
    // accessor when the pre-flight recursion guard establishes that the resolution
    // is bounded. An inferred return is read through the checker's own lazy
    // inference; its diagnostics belong to the declaring file exactly as its
    // ordinary check would report them.
    pub(crate) fn return_type_safely(
        &mut self,
        tp: &mut TypeParser<'_>,
        signature: Option<P<Signature>>,
        subst: &ApiStabilitySubstitution,
    ) -> Option<P<Type>> {
        let signature = signature?;
        if let Some(cached) = get_resolved_return_type_of_signature_if_materialized(Some(signature))
        {
            return Some(cached);
        }
        if !self.signature_return_resolution_is_safe(tp, Some(signature), subst) {
            return None;
        }
        self.note_materializing_read();
        Some(tp.checker.get_return_type_of_signature_exported(signature))
    }

    // go: internal/typeparser/api_stability.go:2153 apiStabilityAnalysis.baseTypesSafely e6afb2e1ff14a356
    // baseTypesSafely returns a declaration's base types, preferring already
    // resolved base types and otherwise materializing the heritage through the
    // ordinary lazy accessor when the recursion guard establishes that the
    // resolution is bounded.
    pub(crate) fn base_types_safely(
        &mut self,
        tp: &mut TypeParser<'_>,
        declaration: Option<P<Type>>,
        subst: &ApiStabilitySubstitution,
    ) -> Option<&'static [P<Type>]> {
        let declaration = declaration?;
        if let Some(bases) = get_resolved_base_types_of_type_if_materialized(Some(declaration)) {
            return Some(bases);
        }
        if !self.base_types_resolution_is_safe(tp, Some(declaration), subst) {
            return None;
        }
        self.note_materializing_read();
        Some(tp.checker.get_base_types_exported(declaration))
    }

    // go: internal/typeparser/api_stability.go:2173 apiStabilityAnalysis.declaredTypeOfSymbol 24dab0ebe36859c6
    // declaredTypeOfSymbol returns the declared type of a class, interface, enum
    // or type alias, preferring an already computed result and otherwise
    // materializing it through the ordinary lazy accessor when the recursion guard
    // establishes that the declaration resolution is bounded. Building a declared
    // type does not expand its members; member and signature components are only
    // read when the public surface actually reaches them.
    pub(crate) fn declared_type_of_symbol(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: Option<P<Symbol>>,
    ) -> Option<P<Type>> {
        self.declared_type_safely(tp, symbol)
    }

    // go: internal/typeparser/api_stability.go:2182 apiStabilityAnalysis.valueTypeOfSymbol f90098068aec0154
    // valueTypeOfSymbol returns the value type of a symbol, preferring an already
    // computed result and otherwise materializing it through the ordinary lazy
    // accessor when the recursion guard establishes that the annotation resolution
    // is bounded. It is only called for symbols with a value side, so a type-only
    // export is never forced into a value lookup.
    pub(crate) fn value_type_of_symbol(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: Option<P<Symbol>>,
    ) -> Option<P<Type>> {
        let symbol = symbol?;
        if !symbol.flags().intersects(SymbolFlags::Value) {
            return None;
        }
        self.type_of_symbol_safely(tp, Some(symbol))
    }

    // go: internal/typeparser/api_stability.go:2192 apiStabilityAnalysis.representedTypeFromNode eb48765eacf03d7f
    // representedTypeFromNode returns the checker type of an annotation node under
    // the analysis safety guard. The node is never used as a syntax-level dependency
    // source: the returned represented type is what the surface traversal walks.
    pub(crate) fn represented_type_from_node(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Option<P<Node>>,
        subst: &ApiStabilitySubstitution,
    ) -> Option<P<Type>> {
        self.type_from_node_safely(tp, node, subst)
    }

    // go: internal/typeparser/api_stability.go:2200 apiStabilityAnalysis.collectTypeAliasDeclarationSurface 241693fbd18ef77d
    // collectTypeAliasDeclarationSurface reads the public surface of a type alias
    // whose declared type is not available from the checker. Only declaration
    // `@stability` tags are metadata-read here; the surface stays incomplete
    // because the represented type the traversal needs is genuinely unavailable.
    pub(crate) fn collect_type_alias_declaration_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        symbol: Option<P<Symbol>>,
        subst: &ApiStabilitySubstitution,
    ) -> bool {
        let Some(declaration) = api_stability_type_alias_declaration(symbol) else {
            return false;
        };
        let Some(rhs) = declaration.type_node() else {
            return false;
        };
        if let Some(represented) = self.represented_type_from_node(tp, Some(rhs), subst) {
            let s = self.type_surface(tp, Some(represented), ApiStabilityInspection::Expand, subst);
            surface.merge(&s);
            return true;
        }
        self.collect_declared_tag_surface(tp, surface, Some(rhs));
        false
    }

    // go: internal/typeparser/api_stability.go:2223 apiStabilityAnalysis.collectDeclaredTagSurface 1f68aa5db0970bb4
    // collectDeclaredTagSurface reads `@stability` tags from the declaration
    // structure of an annotation whose represented type is unavailable. Tags are
    // metadata: no annotation is resolved, no referenced declaration is followed,
    // and no substitution is built. Conditional branches are visited only when the
    // conditional is deferred, so a tag that the compiler eliminated on a concrete
    // conditional is never reported.
    pub(crate) fn collect_declared_tag_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        node: Option<P<Node>>,
    ) {
        let Some(node) = node else {
            return;
        };
        match node.kind() {
            Kind::ParenthesizedType => {
                self.collect_declared_tag_surface(
                    tp,
                    surface,
                    Some(node.as_parenthesized_type_node().type_()),
                );
            }
            Kind::TypeLiteral => {
                for &member in node.members() {
                    if api_stability_property_declaration_is_internal(Some(member)) {
                        continue;
                    }
                    if member.function_like_data().is_some() {
                        self.record_declaration_stability(tp, surface, Some(member));
                        continue;
                    }
                    match member.kind() {
                        Kind::PropertySignature => {
                            self.record_declaration_stability(tp, surface, Some(member));
                            self.collect_declared_tag_surface(
                                tp,
                                surface,
                                member.as_property_signature_declaration().type_(),
                            );
                        }
                        Kind::IndexSignature => {
                            self.record_declaration_stability(tp, surface, Some(member));
                        }
                        _ => {}
                    }
                }
            }
            Kind::FunctionType | Kind::ConstructorType => {
                self.record_declaration_stability(tp, surface, Some(node));
            }
            Kind::ConditionalType => {
                if !self.conditional_declaration_is_deferred(tp, node) {
                    return;
                }
                let conditional = node.as_conditional_type_node();
                self.collect_declared_tag_surface(tp, surface, Some(conditional.true_type()));
                self.collect_declared_tag_surface(tp, surface, Some(conditional.false_type()));
            }
            Kind::UnionType => {
                for &member in node.as_union_type_node().types().nodes() {
                    self.collect_declared_tag_surface(tp, surface, Some(member));
                }
            }
            Kind::IntersectionType => {
                for &member in node.as_intersection_type_node().types().nodes() {
                    self.collect_declared_tag_surface(tp, surface, Some(member));
                }
            }
            Kind::ArrayType => {
                self.collect_declared_tag_surface(
                    tp,
                    surface,
                    Some(node.as_array_type_node().element_type()),
                );
            }
            Kind::TupleType => {
                for &element in node.as_tuple_type_node().elements().nodes() {
                    self.collect_declared_tag_surface(tp, surface, Some(element));
                }
            }
            Kind::NamedTupleMember => {
                self.collect_declared_tag_surface(
                    tp,
                    surface,
                    Some(node.as_named_tuple_member().type_()),
                );
            }
            Kind::OptionalType => {
                self.collect_declared_tag_surface(
                    tp,
                    surface,
                    Some(node.as_optional_type_node().type_()),
                );
            }
            Kind::RestType => {
                self.collect_declared_tag_surface(
                    tp,
                    surface,
                    Some(node.as_rest_type_node().type_()),
                );
            }
            Kind::TypeOperator => {
                self.collect_declared_tag_surface(
                    tp,
                    surface,
                    Some(node.as_type_operator_node().type_()),
                );
            }
            _ => {}
        }
    }

    // go: internal/typeparser/api_stability.go:2293 apiStabilityAnalysis.typeSurface d1d345c5d68b39fc
    // typeSurface computes the surface of a represented type in this analysis. A
    // complete, context-free result is first looked up in the per-checker shared
    // caches and composed from the immutable snapshot plus the component's declared
    // tag when one exists; a result computed under a substitution carrier or cut by
    // an active type or by the analysis bound stays analysis-local, is returned
    // incomplete where relevant, and is settled before it is reused.
    pub(crate) fn type_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: Option<P<Type>>,
        inspect: ApiStabilityInspection,
        subst: &ApiStabilitySubstitution,
    ) -> ApiStabilitySurface {
        let Some(t) = t else {
            return new_api_stability_surface();
        };
        let key = ApiStabilityTypeKey { t, inspect, carrier: subst.carrier() };
        if let Some(cached) = self.session_types.get(&key) {
            let cached = cached.clone();
            if !cached.complete {
                self.scope.types.add(key);
            }
            return cached;
        }
        if key.carrier.is_none()
            && let Some(shared) = self.shared_type_surface(tp, Some(t), inspect)
        {
            // Already published, so it is recorded without being queued for
            // publication again.
            self.settle_types.push(key);
            self.session_types.insert(key, shared.clone());
            return shared;
        }
        self.type_frames.insert(key, subst.frame.clone());
        if self.active_types.contains(&key) {
            return cycle_cut_surface();
        }
        if !self.consume_work() {
            return blocked_surface();
        }
        self.active_types.insert(key);
        let start_epoch = self.materialization_epoch;
        let mut surface = self.collect_type_surface(tp, Some(t), inspect, subst);
        self.active_types.remove(&key);
        if surface.blocked {
            surface.observed_epoch = start_epoch;
        }
        self.store_type_surface(key, surface.clone());
        surface
    }

    // go: internal/typeparser/api_stability.go:2331 apiStabilityAnalysis.collectTypeSurface a49121ffcbc7deb5
    pub(crate) fn collect_type_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: Option<P<Type>>,
        inspect: ApiStabilityInspection,
        subst: &ApiStabilitySubstitution,
    ) -> ApiStabilitySurface {
        let mut surface = new_api_stability_surface();
        let Some(t) = t else {
            return surface;
        };
        if !self.consume_work() {
            surface.block();
            return surface;
        }

        // A component reached through the shallow named-reference boundary honors
        // its own explicit declared tag: the tag contributes at its declared level
        // and the component's internals are not expanded. Independently represented
        // type arguments stay exposed. Roots are never reached with the shallow
        // boundary, so a root tag never stops the root walk.
        if inspect == ApiStabilityInspection::Shallow
            && let Some(identity) = self.type_child_boundary_symbol(tp, Some(t))
        {
            self.record_symbol(tp, &mut surface, Some(identity));
            self.collect_child_type_arguments(tp, &mut surface, Some(t), subst);
            return surface;
        }

        // An alias keeps its symbol and represented type arguments. The underlying
        // representation is still inspected below; the alias is recorded first so an
        // erased or shallow alias can still be named. A repeated application of the
        // same concrete alias object in the same concrete carrier exposes nothing
        // new, which terminates recursive aliases such as `Deep<T>` without
        // evaluating them.
        let mut alias_key = None;
        if let Some(alias) = t.alias() {
            self.record_symbol(tp, &mut surface, alias.symbol());
            if api_stability_represented_surface_accepts_arguments(Some(t))
                && !self.alias_arguments_are_self(tp, Some(alias))
            {
                for &argument in alias.type_arguments() {
                    let s = self.type_surface(
                        tp,
                        Some(argument),
                        ApiStabilityInspection::Shallow,
                        subst,
                    );
                    surface.merge(&s);
                }
            }
            let key = ApiStabilityAliasKey { t, carrier: subst.carrier() };
            if self.active_aliases.contains(&key) {
                surface.cut_by_cycle();
                return surface;
            }
            self.active_aliases.insert(key);
            alias_key = Some(key);
        }

        self.collect_type_surface_by_flags(tp, &mut surface, t, inspect, subst);
        // Rust: Go's `defer delete(a.activeAliases, aliasKey)`.
        if let Some(key) = alias_key {
            self.active_aliases.remove(&key);
        }
        surface
    }

    // go: internal/typeparser/api_stability.go:2331 apiStabilityAnalysis.collectTypeSurface a49121ffcbc7deb5
    // Rust: the flag switch of collectTypeSurface, split out so the deferred alias release runs after it.
    fn collect_type_surface_by_flags(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: P<Type>,
        inspect: ApiStabilityInspection,
        subst: &ApiStabilitySubstitution,
    ) {
        let flags = t.flags();
        if flags.intersects(TypeFlags::TypeParameter) {
            self.record_symbol(tp, surface, t.symbol());
            if let Some((mapped, parent)) = self.substitute(tp, subst, Some(t)) {
                let Some(mapped) = mapped else {
                    // The parameter was replaced by a represented argument the
                    // checker has not materialized. The component is unknown, so
                    // the result is incomplete and never promoted.
                    surface.block();
                    return;
                };
                let s =
                    self.type_surface(tp, Some(mapped), ApiStabilityInspection::Shallow, &parent);
                surface.merge(&s);
                return;
            }
            self.collect_type_parameter_components(tp, surface, t, subst);
        } else if flags.intersects(TypeFlags::UnionOrIntersection) {
            for &member in t.types() {
                let s = self.type_surface(tp, Some(member), ApiStabilityInspection::Shallow, subst);
                surface.merge(&s);
            }
        } else if flags.intersects(TypeFlags::Conditional) {
            self.collect_conditional_surface(tp, surface, t, subst);
        } else if flags.intersects(TypeFlags::IndexedAccess) {
            let indexed = t.as_indexed_access_type();
            let s = self.type_surface(
                tp,
                indexed.object_type(),
                ApiStabilityInspection::Shallow,
                subst,
            );
            surface.merge(&s);
            let s =
                self.type_surface(tp, indexed.index_type(), ApiStabilityInspection::Shallow, subst);
            surface.merge(&s);
            // The projected result of an indexed access is a compiler outcome that
            // the represented operands do not establish; inspecting the operands
            // alone can never complete the surface.
            surface.block();
        } else if flags.intersects(TypeFlags::Index) {
            let s = self.type_surface(
                tp,
                t.as_index_type().target(),
                ApiStabilityInspection::Shallow,
                subst,
            );
            surface.merge(&s);
        } else if flags.intersects(TypeFlags::TemplateLiteral) {
            for &part in t.as_template_literal_type().types() {
                let s = self.type_surface(tp, Some(part), ApiStabilityInspection::Shallow, subst);
                surface.merge(&s);
            }
        } else if flags.intersects(TypeFlags::StringMapping) {
            self.record_symbol(tp, surface, t.symbol());
            let s = self.type_surface(
                tp,
                t.as_string_mapping_type().target(),
                ApiStabilityInspection::Shallow,
                subst,
            );
            surface.merge(&s);
        } else if flags.intersects(TypeFlags::Substitution) {
            let substitution = t.as_substitution_type();
            let s = self.type_surface(
                tp,
                substitution.base_type(),
                ApiStabilityInspection::Shallow,
                subst,
            );
            surface.merge(&s);
            let s = self.type_surface(
                tp,
                substitution.subst_constraint(),
                ApiStabilityInspection::Shallow,
                subst,
            );
            surface.merge(&s);
        } else if flags.intersects(TypeFlags::Object) {
            self.collect_object_surface(tp, surface, t, inspect, subst);
        } else if flags.intersects(TypeFlags::UniqueESSymbol | TypeFlags::Enum) {
            self.record_symbol(tp, surface, t.symbol());
        }
    }

    // go: internal/typeparser/api_stability.go:2439 apiStabilityAnalysis.collectTypeParameterComponents d8b2a1cec6576195
    // collectTypeParameterComponents records the constraint and default of a type
    // parameter, preferring cached checker materializations and otherwise
    // materializing the annotation through the ordinary lazy accessor. An
    // annotation that would expand a recursive alias, or one that is genuinely
    // unavailable, leaves the surface incomplete.
    pub(crate) fn collect_type_parameter_components(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: P<Type>,
        subst: &ApiStabilitySubstitution,
    ) {
        let mut constraint =
            get_resolved_constraint_of_type_parameter_if_materialized(tp.checker, Some(t));
        if constraint.is_none() {
            constraint = self.represented_type_from_node(
                tp,
                type_parameter_annotation_node(Some(t), true),
                subst,
            );
        }
        if constraint.is_some() {
            let s = self.type_surface(tp, constraint, ApiStabilityInspection::Shallow, subst);
            surface.merge(&s);
        } else if type_parameter_annotation_node(Some(t), true).is_some() {
            surface.block();
        }
        let mut default_type =
            get_resolved_default_from_type_parameter_if_materialized(tp.checker, Some(t));
        if default_type.is_none() {
            default_type = self.represented_type_from_node(
                tp,
                type_parameter_annotation_node(Some(t), false),
                subst,
            );
        }
        if default_type.is_some() {
            let s = self.type_surface(tp, default_type, ApiStabilityInspection::Shallow, subst);
            surface.merge(&s);
        } else if type_parameter_annotation_node(Some(t), false).is_some() {
            surface.block();
        }
    }

    // go: internal/typeparser/api_stability.go:2469 apiStabilityAnalysis.collectConditionalSurface 67859b6de7dc0b79
    // collectConditionalSurface inspects a represented conditional type without
    // guessing its selected branch. A conditional whose operands are substituted by
    // an active concrete application, or whose operands are fully concrete, has an
    // outcome the compiler relation selects; unless that outcome is already
    // represented on the type, the surface is blocked instead of guessed. A
    // deferred symbolic conditional (its operands still mention an unbound type
    // parameter) keeps its operands and both declared branches; the declared branch
    // components are materialized lazily through the checker's ordinary reads.
    pub(crate) fn collect_conditional_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: P<Type>,
        subst: &ApiStabilitySubstitution,
    ) {
        // A conditional re-entered on the same recursion path (a recursive alias
        // whose represented branch resolves back to the same type) has already
        // contributed its direct components at the first occurrence. The cut stays
        // incomplete until the settle fixpoint has unified the findings.
        let conditional_key = ApiStabilityAliasKey { t, carrier: subst.carrier() };
        if self.active_conditionals.contains(&conditional_key) {
            surface.cut_by_cycle();
            return;
        }
        self.active_conditionals.insert(conditional_key);
        self.collect_conditional_surface_operands(tp, surface, t, subst);
        // Rust: Go's `defer delete(a.activeConditionals, conditionalKey)`.
        self.active_conditionals.remove(&conditional_key);
    }

    // go: internal/typeparser/api_stability.go:2469 apiStabilityAnalysis.collectConditionalSurface 67859b6de7dc0b79
    // Rust: the body after the cycle guard, split out so the deferred guard release runs after it.
    fn collect_conditional_surface_operands(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: P<Type>,
        subst: &ApiStabilitySubstitution,
    ) {
        let conditional = t.as_conditional_type();
        let check = conditional.check_type();
        let extends = conditional.extends_type();
        if t.mapper().is_some()
            || self.contains_substituted_parameter(tp, subst, check, &mut FxHashSet::default())
            || self.contains_substituted_parameter(tp, subst, extends, &mut FxHashSet::default())
        {
            // A concrete application: the selected branch is a compiler outcome
            // that is not represented on the raw conditional. Never guess it.
            surface.block();
            return;
        }
        if !self.represented_contains_type_parameter(check, &mut FxHashSet::default())
            && !self.represented_contains_type_parameter(extends, &mut FxHashSet::default())
        {
            // A conditional over fully concrete operands also has an outcome that
            // is not represented; refuse to guess as well.
            surface.block();
            return;
        }
        let s = self.type_surface(tp, check, ApiStabilityInspection::Shallow, subst);
        surface.merge(&s);
        let s = self.type_surface(tp, extends, ApiStabilityInspection::Shallow, subst);
        surface.merge(&s);
        self.collect_conditional_branch_surface(tp, surface, t, true, subst);
        self.collect_conditional_branch_surface(tp, surface, t, false, subst);
    }

    // go: internal/typeparser/api_stability.go:2512 apiStabilityAnalysis.containsSubstitutedParameter 8ce562234ecafd42
    // containsSubstitutedParameter reports whether any type parameter reachable in
    // the represented operand graph is replaced by the active substitution. It only
    // reads represented components; it never evaluates a relation.
    pub(crate) fn contains_substituted_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        subst: &ApiStabilitySubstitution,
        t: Option<P<Type>>,
        active: &mut FxHashSet<P<Type>>,
    ) -> bool {
        let Some(t) = t else {
            return false;
        };
        if active.contains(&t) {
            return false;
        }
        active.insert(t);
        let result = self.contains_substituted_parameter_inner(tp, subst, t, active);
        // Rust: Go's `defer delete(active, t)`.
        active.remove(&t);
        result
    }

    // go: internal/typeparser/api_stability.go:2512 apiStabilityAnalysis.containsSubstitutedParameter 8ce562234ecafd42
    // Rust: the body after the cycle guard, split out so the deferred release runs after it.
    fn contains_substituted_parameter_inner(
        &mut self,
        tp: &mut TypeParser<'_>,
        subst: &ApiStabilitySubstitution,
        t: P<Type>,
        active: &mut FxHashSet<P<Type>>,
    ) -> bool {
        let flags = t.flags();
        if flags.intersects(TypeFlags::TypeParameter) {
            return self.substitute(tp, subst, Some(t)).is_some();
        }
        if flags.intersects(TypeFlags::UnionOrIntersection) {
            for &member in t.types() {
                if self.contains_substituted_parameter(tp, subst, Some(member), active) {
                    return true;
                }
            }
        } else if flags.intersects(TypeFlags::Object)
            && t.object_flags().intersects(ObjectFlags::Reference)
        {
            for &argument in get_resolved_type_arguments(Some(t)) {
                if self.contains_substituted_parameter(tp, subst, Some(argument), active) {
                    return true;
                }
            }
        } else if flags.intersects(TypeFlags::IndexedAccess) {
            let indexed = t.as_indexed_access_type();
            return self.contains_substituted_parameter(tp, subst, indexed.object_type(), active)
                || self.contains_substituted_parameter(tp, subst, indexed.index_type(), active);
        } else if flags.intersects(TypeFlags::Index) {
            return self.contains_substituted_parameter(
                tp,
                subst,
                t.as_index_type().target(),
                active,
            );
        } else if flags.intersects(TypeFlags::TemplateLiteral) {
            for &part in t.as_template_literal_type().types() {
                if self.contains_substituted_parameter(tp, subst, Some(part), active) {
                    return true;
                }
            }
        } else if flags.intersects(TypeFlags::Conditional) {
            let conditional = t.as_conditional_type();
            return self.contains_substituted_parameter(
                tp,
                subst,
                conditional.check_type(),
                active,
            ) || self.contains_substituted_parameter(
                tp,
                subst,
                conditional.extends_type(),
                active,
            );
        }
        false
    }

    // go: internal/typeparser/api_stability.go:2563 apiStabilityAnalysis.representedContainsTypeParameter 640c89478e91f7d9
    // representedContainsTypeParameter reports whether an unbound type parameter is
    // a reachable component of the represented operand graph.
    pub(crate) fn represented_contains_type_parameter(
        &mut self,
        t: Option<P<Type>>,
        active: &mut FxHashSet<P<Type>>,
    ) -> bool {
        let Some(t) = t else {
            return false;
        };
        if active.contains(&t) {
            return false;
        }
        active.insert(t);
        let result = self.represented_contains_type_parameter_inner(t, active);
        // Rust: Go's `defer delete(active, t)`.
        active.remove(&t);
        result
    }

    // go: internal/typeparser/api_stability.go:2563 apiStabilityAnalysis.representedContainsTypeParameter 640c89478e91f7d9
    // Rust: the body after the cycle guard, split out so the deferred release runs after it.
    fn represented_contains_type_parameter_inner(
        &mut self,
        t: P<Type>,
        active: &mut FxHashSet<P<Type>>,
    ) -> bool {
        let flags = t.flags();
        if flags.intersects(TypeFlags::TypeParameter) {
            return true;
        }
        if flags.intersects(TypeFlags::UnionOrIntersection) {
            for &member in t.types() {
                if self.represented_contains_type_parameter(Some(member), active) {
                    return true;
                }
            }
        } else if flags.intersects(TypeFlags::Object)
            && t.object_flags().intersects(ObjectFlags::Reference)
        {
            for &argument in get_resolved_type_arguments(Some(t)) {
                if self.represented_contains_type_parameter(Some(argument), active) {
                    return true;
                }
            }
        } else if flags.intersects(TypeFlags::IndexedAccess) {
            let indexed = t.as_indexed_access_type();
            return self.represented_contains_type_parameter(indexed.object_type(), active)
                || self.represented_contains_type_parameter(indexed.index_type(), active);
        } else if flags.intersects(TypeFlags::Index) {
            return self.represented_contains_type_parameter(t.as_index_type().target(), active);
        } else if flags.intersects(TypeFlags::TemplateLiteral) {
            for &part in t.as_template_literal_type().types() {
                if self.represented_contains_type_parameter(Some(part), active) {
                    return true;
                }
            }
        } else if flags.intersects(TypeFlags::Conditional) {
            let conditional = t.as_conditional_type();
            return self.represented_contains_type_parameter(conditional.check_type(), active)
                || self.represented_contains_type_parameter(conditional.extends_type(), active);
        } else if flags.intersects(TypeFlags::Substitution) {
            let substitution = t.as_substitution_type();
            return self.represented_contains_type_parameter(substitution.base_type(), active)
                || self
                    .represented_contains_type_parameter(substitution.subst_constraint(), active);
        }
        false
    }

    // go: internal/typeparser/api_stability.go:2622 apiStabilityAnalysis.collectConditionalBranchSurface cb6a0c93dfcc4e30
    // collectConditionalBranchSurface inspects one declared branch of a deferred
    // conditional. The checker's already resolved branch components are preferred;
    // otherwise the declared branch annotation is materialized lazily through the
    // ordinary type accessor and traversed as a represented component. A primitive
    // keyword branch exposes nothing. A genuinely unavailable branch leaves the
    // surface incomplete.
    pub(crate) fn collect_conditional_branch_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: P<Type>,
        true_branch: bool,
        subst: &ApiStabilitySubstitution,
    ) {
        let Some(node) = get_conditional_type_branch_node(Some(t), true_branch) else {
            surface.block();
            return;
        };
        let mut represented = get_resolved_conditional_type_branch(Some(t), true_branch);
        if represented.is_none() {
            represented = self.represented_type_from_node(tp, Some(node), subst);
        }
        let Some(represented) = represented else {
            if api_stability_primitive_type_node(Some(node)) {
                return;
            }
            surface.block();
            return;
        };
        let s = self.type_surface(tp, Some(represented), ApiStabilityInspection::Shallow, subst);
        surface.merge(&s);
        self.collect_type_node_provenance(tp, surface, Some(node), Some(represented), subst);
    }
}

// go: internal/typeparser/api_stability.go:2647 apiStabilityPrimitiveTypeNode 5e0ed7187db7718b
// apiStabilityPrimitiveTypeNode reports whether a type node resolves to a
// primitive singleton that carries no symbol or member surface. Those nodes do
// not cache a resolved type and expose nothing.
pub(crate) fn api_stability_primitive_type_node(node: Option<P<Node>>) -> bool {
    let Some(node) = node else {
        return false;
    };
    matches!(
        node.kind(),
        Kind::AnyKeyword
            | Kind::UnknownKeyword
            | Kind::NeverKeyword
            | Kind::VoidKeyword
            | Kind::UndefinedKeyword
            | Kind::NullKeyword
            | Kind::StringKeyword
            | Kind::NumberKeyword
            | Kind::BigIntKeyword
            | Kind::BooleanKeyword
            | Kind::SymbolKeyword
            | Kind::ObjectKeyword
    )
}

/// Rust-only: Go's `[]checker.SignatureKind{checker.SignatureKindCall, checker.SignatureKindConstruct}`.
const API_STABILITY_SIGNATURE_KINDS: [SignatureKind; 2] =
    [SignatureKind::Call, SignatureKind::Construct];

impl ApiStabilityAnalysis {
    // go: internal/typeparser/api_stability.go:2668 apiStabilityAnalysis.collectObjectSurface 6bbc38bd8d5aa236
    // collectObjectSurface handles object types with the represented structure
    // rules used by the data-first matcher. A class or interface declaration is
    // inspected through its declared members and heritage clauses; an expanded
    // reference prefers its concrete materialized surface and otherwise
    // materializes it lazily. A named reference reached in a shallow position
    // keeps its declaration, represented arguments and callable signatures and its
    // members are not expanded. Recursive alias expansions are refused by the
    // lazy guards.
    pub(crate) fn collect_object_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: P<Type>,
        inspect: ApiStabilityInspection,
        subst: &ApiStabilitySubstitution,
    ) {
        let flags = t.object_flags();
        if flags.intersects(ObjectFlags::ReverseMapped | ObjectFlags::EvolvingArray) {
            return;
        }
        if flags.intersects(ObjectFlags::InstantiationExpressionType) {
            // An instantiation expression such as `identity<E>` represents an
            // explicitly instantiated callable value whose signatures are safe to
            // read from the checker's represented members.
            self.collect_function_signatures(tp, surface, t, Some(t), subst);
            return;
        }
        // A class declared instance type is a class-or-interface surface inspected
        // through its declaration. The class static side is a separate anonymous
        // object type whose symbol is the class.
        if flags.intersects(ObjectFlags::ClassOrInterface) {
            self.collect_declaration_surface(tp, surface, Some(t), inspect, subst);
            return;
        }
        if flags.intersects(ObjectFlags::Anonymous)
            && let Some(symbol) = t.symbol()
            && symbol.flags().intersects(SymbolFlags::Class)
        {
            self.collect_class_static_surface(tp, surface, t, inspect, subst);
            return;
        }
        // A module namespace is a named reference to a whole module's exports, not a
        // finite anonymous surface. Recording the module symbol keeps the traversal
        // bounded instead of enumerating and reporting every exported member; the
        // rule enumerates namespace members explicitly.
        if let Some(symbol) = t.symbol()
            && symbol.flags().intersects(
                SymbolFlags::Module | SymbolFlags::ValueModule | SymbolFlags::NamespaceModule,
            )
        {
            self.record_symbol(tp, surface, Some(symbol));
            return;
        }
        if flags.intersects(ObjectFlags::Reference)
            && let Some(target) = t.target()
            && target != t
        {
            if self.should_inspect_symbol(tp, target.symbol()) {
                self.record_symbol(tp, surface, target.symbol());
            }
            let arguments = self.reference_arguments(tp, t, Some(target));
            for &argument in &arguments {
                if argument.is_none() {
                    continue;
                }
                let s = self.type_surface(tp, argument, ApiStabilityInspection::Shallow, subst);
                surface.merge(&s);
            }
            if target.object_flags().intersects(ObjectFlags::ClassOrInterface) {
                if !self.should_inspect_symbol(tp, target.symbol()) {
                    // A default-library target contributes only its represented
                    // type arguments; its member surface is never expanded.
                    return;
                }
                if inspect == ApiStabilityInspection::Shallow {
                    // The shallow boundary: declared symbol, represented type
                    // arguments and directly exposed call and construct signatures
                    // only. The raw declaration surface is read with the reference
                    // substitution, so an instantiated signature argument or return
                    // stays represented without expanding arbitrary members.
                    let parameters = self.reference_type_parameters(Some(target));
                    let reference_subst =
                        self.extend_parameters_substitution(subst, Some(t), parameters, arguments);
                    self.collect_declaration_surface(
                        tp,
                        surface,
                        Some(target),
                        ApiStabilityInspection::Shallow,
                        &reference_subst,
                    );
                    return;
                }
                self.collect_concrete_reference_surface(tp, surface, t);
                return;
            }
            return;
        }
        if flags.intersects(ObjectFlags::Mapped) {
            self.collect_mapped_surface(tp, surface, t, subst);
            return;
        }
        let s = self.component_surface(tp, Some(t), inspect, subst);
        surface.merge(&s);
    }

    // go: internal/typeparser/api_stability.go:2754 apiStabilityAnalysis.collectConcreteReferenceSurface d8a26b71dacaaaf0
    // collectConcreteReferenceSurface inspects the concrete public surface of a
    // class or interface reference in an expanded context. The checker's resolved
    // member table is preferred and read directly; the pre-flight recursion guard
    // is only consulted when an individual accessor has not been materialized yet.
    // Each unresolved member, signature and index read shares one completed Safe
    // verdict for the same reference and substitution, so an already resolved
    // accessor never consumes safety work. An unmaterialized member table whose
    // member resolution is not established as bounded falls back to the target
    // declaration under the reference substitution with its own gated reads, and a
    // surface whose member or index resolution would evaluate a recursive alias is
    // left incomplete. Concrete members are already instantiated, so no
    // substitution applies to them; nested named references reached inside those
    // members still use the shallow boundary.
    pub(crate) fn collect_concrete_reference_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        reference: P<Type>,
    ) {
        let empty = ApiStabilitySubstitution::default();
        let mut target = None;
        let mut reference_subst = ApiStabilitySubstitution::default();
        if reference.object_flags().intersects(ObjectFlags::Reference) {
            target = reference.target();
        }
        let owner = reference.symbol();
        let class_or_interface_target = target.filter(|&target| {
            target != reference && target.object_flags().intersects(ObjectFlags::ClassOrInterface)
        });
        if let Some(target) = class_or_interface_target {
            let parameters = self.reference_type_parameters(Some(target));
            let arguments = self.reference_arguments(tp, reference, Some(target));
            reference_subst =
                self.extend_parameters_substitution(&empty, Some(reference), parameters, arguments);
        }
        if let Some(members) = get_resolved_members_of_type_if_materialized(Some(reference)) {
            self.collect_resolved_member_table(tp, surface, members, owner, &empty);
        } else {
            let structured_safe =
                self.member_table_resolution_is_safe(tp, Some(reference), &reference_subst);
            if structured_safe {
                self.note_materializing_read();
                for &member in tp.checker.get_properties_of_type_exported(reference) {
                    if member.flags().intersects(SymbolFlags::TypeParameter)
                        || api_stability_symbol_is_non_public(Some(member))
                    {
                        continue;
                    }
                    if self.optional_tagged_inherited_member(tp, Some(member), owner) {
                        continue;
                    }
                    if let Some(boundary) =
                        self.inherited_child_boundary_symbol(tp, Some(member), owner)
                    {
                        self.record_symbol(tp, surface, Some(boundary));
                        continue;
                    }
                    let s = self.member_surface(tp, Some(member), &empty);
                    surface.merge(&s);
                }
            } else if let Some(target) = class_or_interface_target {
                // The declaration surface under the reference substitution never
                // resolves the instantiated member table; each member and index read
                // is gated on its own.
                self.collect_declaration_surface(
                    tp,
                    surface,
                    Some(target),
                    ApiStabilityInspection::Expand,
                    &reference_subst,
                );
                return;
            } else {
                surface.block();
            }
        }
        // The resolved member table has already flattened inherited members; the
        // tagged ancestors still expose their independently represented type
        // arguments even though their members stay bounded.
        self.collect_tagged_heritage_arguments(
            tp,
            surface,
            target,
            &reference_subst,
            &mut FxHashSet::default(),
        );
        for kind in API_STABILITY_SIGNATURE_KINDS {
            if let Some(signatures) =
                get_resolved_signatures_of_type_if_materialized(Some(reference), kind)
            {
                for &signature in signatures {
                    if self.child_signature_boundary(tp, surface, Some(signature), owner) {
                        continue;
                    }
                    let s = self.signature_surface(tp, Some(signature), &empty);
                    surface.merge(&s);
                }
                continue;
            }
            if !self.member_table_resolution_is_safe(tp, Some(reference), &reference_subst) {
                surface.block();
                continue;
            }
            self.note_materializing_read();
            for &signature in tp.checker.get_signatures_of_type_exported(reference, kind) {
                if self.child_signature_boundary(tp, surface, Some(signature), owner) {
                    continue;
                }
                let s = self.signature_surface(tp, Some(signature), &empty);
                surface.merge(&s);
            }
        }
        if let Some(infos) = get_resolved_index_infos_of_type_if_materialized(Some(reference)) {
            for &info in infos {
                self.collect_index_info_surface(tp, surface, Some(info), &empty, owner);
            }
            return;
        }
        if !self.member_table_resolution_is_safe(tp, Some(reference), &reference_subst) {
            surface.block();
            return;
        }
        self.note_materializing_read();
        for &info in tp.checker.get_index_infos_of_type_exported(reference) {
            self.collect_index_info_surface(tp, surface, Some(info), &empty, owner);
        }
    }

    // go: internal/typeparser/api_stability.go:2853 apiStabilityAnalysis.collectResolvedMemberTable cdb61df454642bac
    // collectResolvedMemberTable merges the public members of an already resolved
    // member table with the given substitution. Compiler-reserved internal entries
    // are skipped, while a late-bound computed member is traversed like any other
    // member: it is the resolved spelling of a public computed declaration and its
    // visibility is decided by its declaration provenance. A member the compiler
    // flattened into this table from an explicitly tagged base is a child boundary:
    // the base's declared level contributes and the member's surface is not
    // expanded. A same-named own override keeps the owner as its declaring symbol
    // and stays inspected.
    pub(crate) fn collect_resolved_member_table(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        members: Option<P<SymbolTable>>,
        owner: Option<P<Symbol>>,
        subst: &ApiStabilitySubstitution,
    ) {
        for name in sorted_symbol_table_keys(members) {
            let Some(member) = symbol_table_get(members, name) else {
                continue;
            };
            if member.flags().intersects(SymbolFlags::TypeParameter)
                || api_stability_symbol_is_non_public(Some(member))
            {
                continue;
            }
            if !api_stability_member_table_name_is_visible(name, Some(member)) {
                continue;
            }
            if self.optional_tagged_inherited_member(tp, Some(member), owner) {
                continue;
            }
            if let Some(boundary) = self.inherited_child_boundary_symbol(tp, Some(member), owner) {
                self.record_symbol(tp, surface, Some(boundary));
                continue;
            }
            let s = self.member_surface(tp, Some(member), subst);
            surface.merge(&s);
        }
    }

    // go: internal/typeparser/api_stability.go:2878 apiStabilityAnalysis.referenceArguments 155c74160bf423f8
    // referenceArguments returns the represented type arguments of a reference,
    // filling omitted arguments from the target's already materialized type
    // parameter defaults. A parameter the checker left unrepresented stays nil so
    // the substitution machinery blocks instead of assuming a constraint or
    // default.
    pub(crate) fn reference_arguments(
        &self,
        tp: &TypeParser<'_>,
        reference: P<Type>,
        target: Option<P<Type>>,
    ) -> Vec<Option<P<Type>>> {
        let class_or_interface = target.is_some_and(|target| {
            target.flags().intersects(TypeFlags::Object)
                && target.object_flags().intersects(ObjectFlags::ClassOrInterface)
        });
        if !class_or_interface {
            return get_resolved_type_arguments(Some(reference)).iter().map(|&t| Some(t)).collect();
        }
        let Some(parameters) = self.reference_type_parameters(target) else {
            return Vec::new();
        };
        if parameters.is_empty() {
            return Vec::new();
        }
        let resolved = get_resolved_type_arguments(Some(reference));
        let mut arguments = vec![None; parameters.len()];
        for (index, &parameter) in parameters.iter().enumerate() {
            if index < resolved.len() {
                arguments[index] = Some(resolved[index]);
                continue;
            }
            if let Some(default_type) = get_resolved_default_from_type_parameter_if_materialized(
                tp.checker,
                Some(parameter),
            ) {
                arguments[index] = Some(default_type);
            }
        }
        arguments
    }

    // go: internal/typeparser/api_stability.go:2903 apiStabilityAnalysis.referenceTypeParameters 0068d8bc3be13bec
    // referenceTypeParameters returns the type parameters of a class or interface
    // reference target. Other reference targets (tuples, arrays of primitives) have
    // no bindable type parameters and return nil.
    #[expect(clippy::unused_self, reason = "a method of Go's apiStabilityAnalysis")]
    pub(crate) fn reference_type_parameters(
        &self,
        t: Option<P<Type>>,
    ) -> Option<&'static [P<Type>]> {
        let t = t?;
        if !t.flags().intersects(TypeFlags::Object)
            || !t.object_flags().intersects(ObjectFlags::ClassOrInterface)
        {
            return None;
        }
        Some(t.as_interface_type().type_parameters())
    }

    // go: internal/typeparser/api_stability.go:2916 apiStabilityAnalysis.collectDeclarationSurface 42d1803898eab22d
    // collectDeclarationSurface inspects the public surface declared by a class or
    // interface declaration: its own members, index signatures and signatures,
    // its type parameters, and (when expanded) its inherited members. The
    // declaration type passed here is always uninstantiated; the substitution
    // context carries the represented type arguments of the reference that reached
    // it.
    pub(crate) fn collect_declaration_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: Option<P<Type>>,
        inspect: ApiStabilityInspection,
        subst: &ApiStabilitySubstitution,
    ) {
        let Some(declaration) = declaration else {
            return;
        };
        let Some(symbol) = declaration.symbol() else {
            return;
        };
        if !self.should_inspect_symbol(tp, Some(symbol)) {
            return;
        }
        self.record_symbol(tp, surface, Some(symbol));
        let key = ApiStabilitySymbolKey { symbol, carrier: subst.carrier() };
        if self.active_symbols.contains(&key) {
            surface.cut_by_cycle();
            return;
        }
        self.active_symbols.insert(key);
        self.collect_declaration_surface_members(tp, surface, declaration, symbol, inspect, subst);
        // Rust: Go's `defer delete(a.activeSymbols, key)`.
        self.active_symbols.remove(&key);
    }

    // go: internal/typeparser/api_stability.go:2916 apiStabilityAnalysis.collectDeclarationSurface 42d1803898eab22d
    // Rust: the body after the cycle guard, split out so the deferred guard release runs after it.
    fn collect_declaration_surface_members(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: P<Type>,
        symbol: P<Symbol>,
        inspect: ApiStabilityInspection,
        subst: &ApiStabilitySubstitution,
    ) {
        if inspect == ApiStabilityInspection::Shallow {
            // A named dependency keeps the shallow boundary: its own public call and
            // construct signatures (and their inherited signatures) are part of the
            // represented surface, but its arbitrary members are not expanded.
            self.collect_declaration_signatures(tp, surface, Some(symbol), subst, false);
            self.collect_heritage(tp, surface, declaration, ApiStabilityInspection::Shallow, subst);
            return;
        }

        for &type_parameter in declaration.as_interface_type().local_type_parameters() {
            let s =
                self.type_surface(tp, Some(type_parameter), ApiStabilityInspection::Shallow, subst);
            surface.merge(&s);
        }

        if self.collect_resolved_member_table_if_materialized(
            tp,
            surface,
            declaration,
            Some(symbol),
            subst,
        ) {
        } else if symbol.members().is_some() {
            self.collect_resolved_member_table(tp, surface, symbol.members(), Some(symbol), subst);
            // A computed member name that the checker must evaluate to resolve the
            // member table is not part of the early table; the surface is incomplete
            // and is never treated as stable.
            if self.symbol_has_late_bound_members(Some(symbol)) {
                surface.block();
            }
        } else if self.member_table_resolution_is_safe(tp, Some(declaration), subst) {
            let members = self.materialized_members_of_symbol(tp.checker, symbol);
            if members.is_some() {
                self.collect_resolved_member_table(tp, surface, members, Some(symbol), subst);
            } else {
                surface.block();
            }
        } else {
            surface.block();
        }

        self.collect_declaration_signatures(tp, surface, Some(symbol), subst, false);
        self.collect_index_infos_of_declaration(tp, surface, declaration, Some(symbol), subst);
        self.collect_heritage(tp, surface, declaration, inspect, subst);
    }

    // go: internal/typeparser/api_stability.go:2982 apiStabilityAnalysis.memberSurface a4c038827b3936e5
    // memberSurface inspects one class or interface member. The member's own
    // declared `@stability` tag is metadata and always contributes; when the tag is
    // explicit the member is a child boundary for its own represented type and its
    // internals are not expanded. Otherwise the member's represented type is
    // preferred from the checker's cache and otherwise materialized through the
    // ordinary lazy accessor; a member that genuinely has no type still exposes its
    // raw declaration signature.
    pub(crate) fn member_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        member: Option<P<Symbol>>,
        subst: &ApiStabilitySubstitution,
    ) -> ApiStabilitySurface {
        let mut surface = new_api_stability_surface();
        let Some(member) = member else {
            return surface;
        };
        self.record_symbol(tp, &mut surface, Some(member));
        if tp.declared_api_stability_of_symbol(member).declaration.is_some() {
            return surface;
        }
        let Some(member_type) = self.type_of_symbol_safely(tp, Some(member)) else {
            // A method or accessor whose function type is genuinely unavailable
            // still exposes its raw declaration signature. The signature belongs to
            // the class or interface that declares the member, so that declaring
            // symbol is the owner comparison: a tagged declaring class never bounds
            // its own member's raw signature.
            let mut signature_owner = self.member_declaring_symbol(tp, Some(member));
            if signature_owner.is_none() {
                signature_owner = Some(member);
            }
            let signatures = self.materialized_signatures_of_symbol(tp.checker, member);
            if !signatures.is_empty() {
                for signature in signatures {
                    if self.child_signature_boundary(
                        tp,
                        &mut surface,
                        Some(signature),
                        signature_owner,
                    ) {
                        continue;
                    }
                    let s = self.signature_surface(tp, Some(signature), subst);
                    surface.merge(&s);
                }
                return surface;
            }
            surface.block();
            return surface;
        };
        let s = self.type_surface(tp, Some(member_type), ApiStabilityInspection::Shallow, subst);
        surface.merge(&s);
        for &declaration in member.declarations() {
            if !self.should_inspect_member_declaration(Some(declaration)) {
                continue;
            }
            self.collect_declaration_provenance(
                tp,
                &mut surface,
                Some(declaration),
                Some(member_type),
                subst,
            );
        }
        surface
    }

    // go: internal/typeparser/api_stability.go:3031 apiStabilityAnalysis.collectDeclarationSignatures e2a61fdaa2c7cfc8
    // collectDeclarationSignatures collects a declaration's own public call and
    // construct signatures. Construct signatures declared under the class
    // constructor member are only included when includeClassConstructors is set;
    // a class instance surface does not expose them, while the static side does.
    // The early member table is read directly: call, construct and constructor
    // symbols are never late-bound, so inspecting them must not resolve a
    // late-bound computed-name member table.
    pub(crate) fn collect_declaration_signatures(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        symbol: Option<P<Symbol>>,
        subst: &ApiStabilitySubstitution,
        include_class_constructors: bool,
    ) {
        let Some(symbol) = symbol else {
            return;
        };
        let Some(members) = symbol.members() else {
            return;
        };
        let mut internals = vec![INTERNAL_SYMBOL_NAME_CALL, INTERNAL_SYMBOL_NAME_NEW];
        if include_class_constructors {
            internals.push(INTERNAL_SYMBOL_NAME_CONSTRUCTOR);
        }
        for internal in internals {
            let Some(member) = SymbolTable::get(&members, internal) else {
                continue;
            };
            if !self.signature_member_resolution_is_safe(tp, Some(member), subst) {
                surface.block();
                continue;
            }
            for signature in self.materialized_signatures_of_symbol(tp.checker, member) {
                if self.child_signature_boundary(tp, surface, Some(signature), Some(symbol)) {
                    continue;
                }
                let s = self.signature_surface(tp, Some(signature), subst);
                surface.merge(&s);
            }
        }
    }

    // go: internal/typeparser/api_stability.go:3071 apiStabilityAnalysis.collectIndexInfosOfDeclaration 9b20157b73ef2d78
    // collectIndexInfosOfDeclaration reads a declaration's index signatures. The
    // already resolved index infos are used when the checker materialized them;
    // otherwise the declared index signatures contribute their own `@stability`
    // tag and their materialized key/value annotations, and an unmaterialized
    // annotation leaves the surface incomplete. The resolved infos include index
    // signatures the compiler flattened in from inherited bases; each info is
    // attributed to its declaring class or interface so an explicitly tagged base's
    // index signature stays a child boundary. The declared-only fallback carries
    // own declarations, so warm and cold runs agree on that boundary.
    pub(crate) fn collect_index_infos_of_declaration(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: P<Type>,
        symbol: Option<P<Symbol>>,
        subst: &ApiStabilitySubstitution,
    ) {
        if let Some(infos) = get_resolved_index_infos_of_type_if_materialized(Some(declaration)) {
            for &info in infos {
                self.collect_index_info_surface(tp, surface, Some(info), subst, symbol);
            }
            return;
        }
        let infos = get_declared_index_infos_of_symbol(tp.checker, symbol);
        for info in infos {
            if info.declaration().is_none() {
                continue;
            }
            self.collect_index_signature_surface(
                tp,
                surface,
                info.declaration(),
                info.key_type(),
                info.value_type(),
                subst,
                symbol,
            );
        }
    }

    // go: internal/typeparser/api_stability.go:3098 apiStabilityAnalysis.collectIndexInfoSurface 7277eb3a8a6e05f5
    // collectIndexInfoSurface inspects one already resolved index info. The index
    // declaration's own `@stability` tag always contributes; when the tag is
    // explicit the index signature is a child boundary and its key and value
    // internals are not expanded. A resolved index info the compiler flattened in
    // from an explicitly tagged base (its declaring class or interface differs from
    // owner) is bounded the same way. A key or value type the checker has not
    // resolved is materialized lazily from the declaration when possible.
    pub(crate) fn collect_index_info_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        info: Option<P<IndexInfo>>,
        subst: &ApiStabilitySubstitution,
        owner: Option<P<Symbol>>,
    ) {
        let Some(info) = info else {
            return;
        };
        let declaration = info.declaration();
        if self.declaration_child_boundary(tp, surface, declaration, owner) {
            return;
        }
        let mut key_type = info.key_type.get();
        if key_type.is_none() {
            key_type = self.represented_type_from_node(
                tp,
                api_stability_index_signature_key_node(declaration),
                subst,
            );
        }
        let mut value_type = info.value_type.get();
        if value_type.is_none() {
            value_type = self.represented_type_from_node(
                tp,
                api_stability_index_signature_value_node(declaration),
                subst,
            );
        }
        if key_type.is_some() {
            let s = self.type_surface(tp, key_type, ApiStabilityInspection::Shallow, subst);
            surface.merge(&s);
        } else {
            surface.block();
        }
        if value_type.is_some() {
            let s = self.type_surface(tp, value_type, ApiStabilityInspection::Shallow, subst);
            surface.merge(&s);
            self.collect_type_node_provenance(
                tp,
                surface,
                api_stability_index_signature_value_node(declaration),
                value_type,
                subst,
            );
        } else {
            surface.block();
        }
    }

    // go: internal/typeparser/api_stability.go:3134 apiStabilityAnalysis.collectIndexSignatureSurface ad0f963cbaf2dc42
    // collectIndexSignatureSurface inspects one declared index signature. The key
    // and value types are used when the checker has already represented them;
    // otherwise the declaration's annotations are materialized lazily. An explicit
    // `@stability` tag makes the signature a child boundary: the tag contributes
    // and its key and value internals are not expanded, and a declaration whose
    // declaring class or interface is explicitly tagged (and differs from owner) is
    // bounded the same way.
    pub(crate) fn collect_index_signature_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: Option<P<Node>>,
        key_type: Option<P<Type>>,
        value_type: Option<P<Type>>,
        subst: &ApiStabilitySubstitution,
        owner: Option<P<Symbol>>,
    ) {
        let Some(declaration) = declaration else {
            return;
        };
        if self.declaration_child_boundary(tp, surface, Some(declaration), owner) {
            return;
        }
        let mut key_type = key_type;
        let index_signature = declaration.as_index_signature_declaration();
        if let Some(parameters) = index_signature.parameters() {
            for &parameter in parameters.nodes() {
                if parameter.kind() != Kind::Parameter {
                    continue;
                }
                let type_node = parameter.as_parameter_declaration().type_();
                if key_type.is_none() {
                    key_type = self.represented_type_from_node(tp, type_node, subst);
                }
                if key_type.is_some() {
                    let s = self.type_surface(tp, key_type, ApiStabilityInspection::Shallow, subst);
                    surface.merge(&s);
                    self.collect_type_node_provenance(tp, surface, type_node, key_type, subst);
                } else {
                    surface.block();
                }
            }
        }
        let mut value_type = value_type;
        if value_type.is_none() {
            value_type = self.represented_type_from_node(tp, index_signature.type_(), subst);
        }
        if value_type.is_some() {
            let s = self.type_surface(tp, value_type, ApiStabilityInspection::Shallow, subst);
            surface.merge(&s);
            self.collect_type_node_provenance(
                tp,
                surface,
                index_signature.type_(),
                value_type,
                subst,
            );
            return;
        }
        surface.block();
    }

    // go: internal/typeparser/api_stability.go:3177 apiStabilityAnalysis.collectHeritage 4c2a70cea6272bcc
    // collectHeritage inspects a declaration's inherited surface. Base types the
    // checker has already resolved are preferred; otherwise the declaration's
    // heritage is materialized through the ordinary lazy accessor. The clauses are
    // never resolved from syntax.
    pub(crate) fn collect_heritage(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: P<Type>,
        inspect: ApiStabilityInspection,
        subst: &ApiStabilitySubstitution,
    ) {
        let resolved = get_resolved_base_types_of_type_if_materialized(Some(declaration));
        let bases = if let Some(bases) = resolved {
            bases
        } else {
            if !api_stability_symbol_declares_heritage(declaration.symbol()) {
                return;
            }
            let Some(lazy_bases) = self.base_types_safely(tp, Some(declaration), subst) else {
                surface.block();
                return;
            };
            lazy_bases
        };
        for &base in bases {
            self.collect_inherited_base(tp, surface, Some(base), inspect, subst);
        }
    }

    // go: internal/typeparser/api_stability.go:3197 apiStabilityAnalysis.collectInheritedBase c82155e3d10497c4
    // collectInheritedBase preserves inheritance semantics through the intersection
    // return types of superclass factories. Direct references still use typeSurface.
    pub(crate) fn collect_inherited_base(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        base: Option<P<Type>>,
        inspect: ApiStabilityInspection,
        subst: &ApiStabilitySubstitution,
    ) {
        let Some(base) = base else {
            return;
        };
        if base.flags().intersects(TypeFlags::Intersection)
            && self.heritage_child_boundary_symbol(tp, Some(base), Some(base)).is_none()
        {
            self.collect_child_type_arguments(tp, surface, Some(base), subst);
            for &constituent in base.types() {
                self.collect_inherited_base(tp, surface, Some(constituent), inspect, subst);
            }
            return;
        }
        let mut target = base;
        if base.object_flags().intersects(ObjectFlags::Reference)
            && let Some(base_target) = base.target()
        {
            target = base_target;
        }
        if !target.object_flags().intersects(ObjectFlags::ClassOrInterface) {
            let s = self.type_surface(tp, Some(base), ApiStabilityInspection::Shallow, subst);
            surface.merge(&s);
            return;
        }
        if self.optional_tagged_base(tp, target.symbol()) {
            return;
        }
        if let Some(boundary) = self.heritage_child_boundary_symbol(tp, Some(base), Some(target)) {
            self.record_symbol(tp, surface, Some(boundary));
            self.collect_child_type_arguments(tp, surface, Some(base), subst);
            return;
        }
        let parameters = self.reference_type_parameters(Some(target));
        let arguments = self.reference_arguments(tp, base, Some(target));
        let base_subst =
            self.extend_parameters_substitution(subst, Some(base), parameters, arguments);
        self.collect_declaration_surface(tp, surface, Some(target), inspect, &base_subst);
    }

    // go: internal/typeparser/api_stability.go:3233 apiStabilityAnalysis.collectClassStaticSurface 4fbb3f8f7f0c0718
    // collectClassStaticSurface inspects the class static side. Its members are
    // declared on the class symbol's exports and cannot depend on class type
    // parameters. The instance side of a class value is included when the class
    // value is directly exported, because `typeof C` exposes it through
    // construction.
    pub(crate) fn collect_class_static_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: P<Type>,
        inspect: ApiStabilityInspection,
        subst: &ApiStabilitySubstitution,
    ) {
        let Some(symbol) = t.symbol() else {
            return;
        };
        if !self.should_inspect_symbol(tp, Some(symbol)) {
            return;
        }
        self.record_symbol(tp, surface, Some(symbol));
        if inspect == ApiStabilityInspection::Expand {
            if let Some(instance) = self.declared_type_safely(tp, Some(symbol)) {
                let s =
                    self.type_surface(tp, Some(instance), ApiStabilityInspection::Expand, subst);
                surface.merge(&s);
            }
            // The class static members of a directly expanded class value are part
            // of its own surface. A nested `typeof C` keeps the shallow boundary:
            // its declared symbol and directly exposed call and construct
            // signatures are inspected, but its static members are not expanded.
            if let Some(members) = get_resolved_members_of_type_if_materialized(Some(t)) {
                for name in sorted_symbol_table_keys(members) {
                    if name == "prototype" {
                        continue;
                    }
                    let Some(member) = symbol_table_get(members, name) else {
                        continue;
                    };
                    if api_stability_symbol_is_non_public(Some(member)) {
                        continue;
                    }
                    if !api_stability_member_table_name_is_visible(name, Some(member)) {
                        continue;
                    }
                    if self.optional_tagged_inherited_member(tp, Some(member), Some(symbol)) {
                        continue;
                    }
                    if let Some(boundary) =
                        self.inherited_child_boundary_symbol(tp, Some(member), Some(symbol))
                    {
                        self.record_symbol(tp, surface, Some(boundary));
                        continue;
                    }
                    let s =
                        self.member_surface(tp, Some(member), &ApiStabilitySubstitution::default());
                    surface.merge(&s);
                }
            } else if let Some(exports) = symbol.exports() {
                for name in sorted_symbol_table_keys(Some(exports)) {
                    if name == "prototype" {
                        continue;
                    }
                    let Some(member) = SymbolTable::get(&exports, name) else {
                        continue;
                    };
                    if api_stability_symbol_is_non_public(Some(member)) {
                        continue;
                    }
                    if !api_stability_member_table_name_is_visible(name, Some(member)) {
                        continue;
                    }
                    if self.optional_tagged_inherited_member(tp, Some(member), Some(symbol)) {
                        continue;
                    }
                    if let Some(boundary) =
                        self.inherited_child_boundary_symbol(tp, Some(member), Some(symbol))
                    {
                        self.record_symbol(tp, surface, Some(boundary));
                        continue;
                    }
                    let s =
                        self.member_surface(tp, Some(member), &ApiStabilitySubstitution::default());
                    surface.merge(&s);
                }
                // The early exports table omits computed members (unique-symbol and
                // dynamic names); the surface is incomplete until the checker
                // late-binds them.
                if self.symbol_has_late_bound_members(Some(symbol)) {
                    surface.block();
                }
            }
        }
        self.collect_declaration_signatures(tp, surface, Some(symbol), subst, true);
        for kind in API_STABILITY_SIGNATURE_KINDS {
            if let Some(signatures) = get_resolved_signatures_of_type_if_materialized(Some(t), kind)
            {
                for &signature in signatures {
                    if self.child_signature_boundary(tp, surface, Some(signature), Some(symbol)) {
                        continue;
                    }
                    let s = self.signature_surface(tp, Some(signature), subst);
                    surface.merge(&s);
                }
            }
        }
    }

    // go: internal/typeparser/api_stability.go:3320 apiStabilityAnalysis.collectAnonymousSurface 4592844a735d7f94
    // collectAnonymousSurface inspects a finite anonymous object type. The concrete
    // member surface of an instantiated anonymous type is preferred and read
    // directly when materialized. An unmaterialized instantiated member surface is
    // materialized through the ordinary lazy property accessor only when the
    // recursion guard establishes that member resolution is bounded; otherwise the
    // raw target members are read under the mapper substitution with their own
    // gated reads. A raw anonymous type (the target of an instantiation reached
    // through a heritage or mapper path) is inspected through its declared members
    // under the active substitution.
    pub(crate) fn collect_anonymous_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: P<Type>,
        subst: &ApiStabilitySubstitution,
    ) {
        let instantiated_target =
            if t.object_flags().intersects(ObjectFlags::Instantiated) { t.target() } else { None };
        let instantiated = instantiated_target.is_some();
        let mut source = t;
        let mut current = subst.clone();
        if let Some(target) = instantiated_target {
            source = target;
            if let Some(mapper) = t.mapper() {
                current = self.extend_mapper_substitution(subst, Some(t), mapper);
            }
        }
        self.record_symbol(tp, surface, source.symbol());
        if instantiated {
            let empty = ApiStabilitySubstitution::default();
            if self.collect_resolved_member_table_if_materialized(
                tp,
                surface,
                t,
                source.symbol(),
                &empty,
            ) {
            } else if self.member_table_resolution_is_safe(tp, Some(t), &current) {
                self.note_materializing_read();
                for &member in tp.checker.get_properties_of_type_exported(t) {
                    if member.flags().intersects(SymbolFlags::TypeParameter)
                        || api_stability_symbol_is_non_public(Some(member))
                    {
                        continue;
                    }
                    if let Some(boundary) =
                        self.inherited_child_boundary_symbol(tp, Some(member), source.symbol())
                    {
                        self.record_symbol(tp, surface, Some(boundary));
                        continue;
                    }
                    let s = self.member_surface(tp, Some(member), &empty);
                    surface.merge(&s);
                }
            } else if let Some(source_symbol) = source.symbol()
                && source_symbol.members().is_some()
            {
                self.collect_resolved_member_table(
                    tp,
                    surface,
                    source_symbol.members(),
                    Some(source_symbol),
                    &current,
                );
                if self.symbol_has_late_bound_members(Some(source_symbol)) {
                    surface.block();
                }
            } else {
                surface.block();
            }
            self.collect_anonymous_index_infos(tp, surface, t, Some(source), &empty, &current);
            self.collect_function_signatures(tp, surface, t, Some(source), &current);
            return;
        }
        if let Some(source_symbol) = source.symbol() {
            if self.collect_resolved_member_table_if_materialized(
                tp,
                surface,
                source,
                Some(source_symbol),
                &current,
            ) {
            } else if source_symbol.members().is_some() {
                self.collect_resolved_member_table(
                    tp,
                    surface,
                    source_symbol.members(),
                    Some(source_symbol),
                    &current,
                );
                if self.symbol_has_late_bound_members(Some(source_symbol)) {
                    surface.block();
                }
            } else if self.member_table_resolution_is_safe(tp, Some(source), &current) {
                let members = self.materialized_members_of_symbol(tp.checker, source_symbol);
                if members.is_some() {
                    self.collect_resolved_member_table(
                        tp,
                        surface,
                        members,
                        Some(source_symbol),
                        &current,
                    );
                }
                // A function-like symbol has no member table of its own; its call
                // signatures are declared directly on the symbol and are read by
                // collectFunctionSignatures.
            } else {
                surface.block();
            }
        }
        self.collect_anonymous_index_infos(tp, surface, t, Some(source), &current, &current);
        self.collect_function_signatures(tp, surface, t, Some(source), &current);
    }

    // go: internal/typeparser/api_stability.go:3386 apiStabilityAnalysis.collectResolvedMemberTableIfMaterialized 95e1bb51301f16ef
    // collectResolvedMemberTableIfMaterialized merges the checker's already
    // resolved member table when one exists.
    pub(crate) fn collect_resolved_member_table_if_materialized(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: P<Type>,
        owner: Option<P<Symbol>>,
        subst: &ApiStabilitySubstitution,
    ) -> bool {
        if let Some(members) = get_resolved_members_of_type_if_materialized(Some(t)) {
            self.collect_resolved_member_table(tp, surface, members, owner, subst);
            return true;
        }
        false
    }

    // go: internal/typeparser/api_stability.go:3398 apiStabilityAnalysis.collectAnonymousIndexInfos 8a42e04293103e73
    // collectAnonymousIndexInfos reads the index signatures of an anonymous type.
    // The resolved infos of the (possibly instantiated) type are preferred; when
    // the checker has not resolved them, the raw declared infos contribute under
    // the mapper substitution and their annotations are read through the guard.
    pub(crate) fn collect_anonymous_index_infos(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: P<Type>,
        source: Option<P<Type>>,
        subst: &ApiStabilitySubstitution,
        mapper_subst: &ApiStabilitySubstitution,
    ) {
        let owner = source.and_then(|source| source.symbol());
        if let Some(infos) = get_resolved_index_infos_of_type_if_materialized(Some(t)) {
            let mut resolved_subst = subst.clone();
            if source != Some(t) {
                resolved_subst = ApiStabilitySubstitution::default();
            }
            for &info in infos {
                self.collect_index_info_surface(tp, surface, Some(info), &resolved_subst, owner);
            }
            return;
        }
        let Some(source_symbol) = source.and_then(|source| source.symbol()) else {
            return;
        };
        if source != Some(t) {
            self.collect_index_infos_of_symbol(tp, surface, Some(source_symbol), mapper_subst);
            return;
        }
        self.collect_index_infos_of_symbol(tp, surface, Some(source_symbol), subst);
    }

    // go: internal/typeparser/api_stability.go:3431 apiStabilityAnalysis.collectIndexInfosOfSymbol a5cb8f8aa2422359
    // collectIndexInfosOfSymbol contributes the declared index signatures of a
    // symbol under a substitution context. Key and value annotations are read
    // through the recursion guard; an unavailable annotation leaves the surface
    // incomplete.
    pub(crate) fn collect_index_infos_of_symbol(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        symbol: Option<P<Symbol>>,
        subst: &ApiStabilitySubstitution,
    ) {
        if symbol.is_none() {
            return;
        }
        for info in get_declared_index_infos_of_symbol(tp.checker, symbol) {
            if info.declaration().is_none() {
                continue;
            }
            self.collect_index_signature_surface(
                tp,
                surface,
                info.declaration(),
                info.key_type(),
                info.value_type(),
                subst,
                symbol,
            );
        }
    }

    // go: internal/typeparser/api_stability.go:3449 apiStabilityAnalysis.collectFunctionSignatures 2c5cde883d27c750
    // collectFunctionSignatures reads the call and construct signatures of an
    // anonymous function-like type. Resolved signatures are preferred. An
    // unmaterialized surface is read from the raw declaration signatures under the
    // active substitution: creating a raw signature resolves its parameter
    // annotations but never instantiates the enclosing application, and the
    // guarded per-signature reads stay bounded.
    pub(crate) fn collect_function_signatures(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: P<Type>,
        source: Option<P<Type>>,
        subst: &ApiStabilitySubstitution,
    ) {
        let owner = source.and_then(|source| source.symbol());
        for kind in API_STABILITY_SIGNATURE_KINDS {
            if let Some(signatures) = get_resolved_signatures_of_type_if_materialized(Some(t), kind)
            {
                for &signature in signatures {
                    if self.child_signature_boundary(tp, surface, Some(signature), owner) {
                        continue;
                    }
                    let s = self.signature_surface(tp, Some(signature), subst);
                    surface.merge(&s);
                }
                continue;
            }
            let Some(symbol) = source.and_then(|source| source.symbol()) else {
                surface.block();
                continue;
            };
            let mut internal = INTERNAL_SYMBOL_NAME_CALL;
            if kind == SignatureKind::Construct {
                internal = INTERNAL_SYMBOL_NAME_NEW;
            }
            let mut member =
                symbol.members().and_then(|members| SymbolTable::get(&members, internal));
            if member.is_none()
                && kind == SignatureKind::Call
                && symbol.flags().intersects(
                    SymbolFlags::Function
                        | SymbolFlags::Method
                        | SymbolFlags::GetAccessor
                        | SymbolFlags::SetAccessor
                        | SymbolFlags::Accessor,
                )
            {
                // A function or accessor declaration keeps its call signatures on
                // its own symbol rather than in a member table.
                member = Some(symbol);
            }
            let Some(member) = member else {
                continue;
            };
            if !self.signature_member_resolution_is_safe(tp, Some(member), subst) {
                surface.block();
                continue;
            }
            for signature in self.materialized_signatures_of_symbol(tp.checker, member) {
                if self.child_signature_boundary(tp, surface, Some(signature), owner) {
                    continue;
                }
                let s = self.signature_surface(tp, Some(signature), subst);
                surface.merge(&s);
            }
        }
    }

    // go: internal/typeparser/api_stability.go:3502 apiStabilityAnalysis.collectMappedSurface 394b8099081fe60d
    // collectMappedSurface reads the represented mapped type components. An
    // instantiated mapped type is read through its target so the mapped parameter
    // stays uninstantiated and the mapper provides the substitution. Cached
    // components are preferred; a missing component is materialized through the
    // ordinary mapped accessors unless its annotation would expand a recursive
    // alias. Whatever remains unavailable leaves the surface incomplete.
    pub(crate) fn collect_mapped_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: P<Type>,
        subst: &ApiStabilitySubstitution,
    ) {
        self.record_symbol(tp, surface, t.symbol());
        let mut source = t;
        let mut current = subst.clone();
        if t.object_flags().intersects(ObjectFlags::Instantiated)
            && let Some(target) = t.target()
        {
            source = target;
            if let Some(mapper) = t.mapper() {
                current = self.extend_mapper_substitution(subst, Some(t), mapper);
            }
        }
        let (mut constraint_node, mut name_node, mut template_node) = (None, None, None);
        if let Some(symbol) = source.symbol() {
            for &declaration in symbol.declarations() {
                if declaration.kind() != Kind::MappedType {
                    continue;
                }
                let mapped_declaration = declaration.as_mapped_type_node();
                constraint_node = mapped_declaration
                    .type_parameter()
                    .as_type_parameter_declaration()
                    .constraint();
                name_node = mapped_declaration.name_type();
                template_node = mapped_declaration.type_();
                break;
            }
        }
        let mut constraint = get_mapped_type_constraint_type(source);
        if constraint.is_none() {
            constraint = self.type_from_node_safely(tp, constraint_node, &current);
        }
        if constraint.is_some() {
            let s = self.type_surface(tp, constraint, ApiStabilityInspection::Shallow, &current);
            surface.merge(&s);
        } else {
            surface.block();
        }
        let mut name = get_mapped_type_name_type(source);
        if name.is_none() {
            name = self.type_from_node_safely(tp, name_node, &current);
        }
        if name.is_some() {
            let s = self.type_surface(tp, name, ApiStabilityInspection::Shallow, &current);
            surface.merge(&s);
        } else if name_node.is_some() || api_stability_mapped_declaration_has_name(Some(source)) {
            surface.block();
        }
        let mut template = get_mapped_type_template_type(source);
        if template.is_none() {
            template = self.type_from_node_safely(tp, template_node, &current);
        }
        if template.is_some() {
            let s = self.type_surface(tp, template, ApiStabilityInspection::Shallow, &current);
            surface.merge(&s);
        } else {
            surface.block();
        }
    }

    // go: internal/typeparser/api_stability.go:3567 apiStabilityAnalysis.componentSurface 0e59d2b3723b4108
    // componentSurface inspects a finite anonymous component exactly once per
    // recursion path and substitution context.
    pub(crate) fn component_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: Option<P<Type>>,
        inspect: ApiStabilityInspection,
        subst: &ApiStabilitySubstitution,
    ) -> ApiStabilitySurface {
        let Some(t) = t else {
            return new_api_stability_surface();
        };
        let key = ApiStabilityTypeKey { t, inspect, carrier: subst.carrier() };
        if let Some(cached) = self.session_components.get(&key) {
            let cached = cached.clone();
            if !cached.complete {
                self.scope.components.add(key);
            }
            return cached;
        }
        self.type_frames.insert(key, subst.frame.clone());
        if self.active_components.contains(&key) {
            return cycle_cut_surface();
        }
        if !self.consume_work() {
            return blocked_surface();
        }
        self.active_components.insert(key);
        let start_epoch = self.materialization_epoch;
        let mut surface = self.collect_object_component_surface(tp, Some(t), inspect, subst);
        self.active_components.remove(&key);
        if surface.blocked {
            surface.observed_epoch = start_epoch;
        }
        self.store_component_surface(key, surface.clone());
        surface
    }

    // go: internal/typeparser/api_stability.go:3599 apiStabilityAnalysis.collectObjectComponentSurface 9efdae5dcd70d09b
    // collectObjectComponentSurface is used when a represented object type must be
    // inspected as a finite anonymous surface (mapped targets, reference targets
    // that are anonymous). Class and interface declarations never reach this path.
    pub(crate) fn collect_object_component_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: Option<P<Type>>,
        inspect: ApiStabilityInspection,
        subst: &ApiStabilitySubstitution,
    ) -> ApiStabilitySurface {
        let mut surface = new_api_stability_surface();
        let Some(t) = t else {
            return surface;
        };
        if t.object_flags().intersects(ObjectFlags::ClassOrInterface) {
            // A class or interface target is inspected through its declaration, not
            // through the resolved member surface.
            self.collect_declaration_surface(tp, &mut surface, Some(t), inspect, subst);
            return surface;
        }
        if let Some(symbol) = t.symbol()
            && symbol.flags().intersects(SymbolFlags::Class)
        {
            self.collect_class_static_surface(tp, &mut surface, t, inspect, subst);
            return surface;
        }
        self.collect_anonymous_surface(tp, &mut surface, t, subst);
        surface
    }

    // go: internal/typeparser/api_stability.go:3630 apiStabilityAnalysis.signatureSurface 8d01f83b62fd3880
    // signatureSurface reads a signature's uninstantiated target together with its
    // represented substitution. Using the target keeps a generic signature's
    // parameter and return types uninstantiated (so a recursive alias is never
    // evaluated), while the substitution frame retains the instantiated context
    // (so an inferred return such as `(x: T) => T` instantiated with `E` still
    // exposes `E`). The concrete signature pointer and the substitution context are
    // both part of the cache key, so the same raw signature analyzed under
    // different enclosing mappers never shares a cached result. A complete,
    // context-free concrete signature surface is looked up in the per-checker
    // shared caches and composed from the immutable snapshot plus the signature's
    // declared tag; a result computed under a substitution carrier stays
    // analysis-local.
    pub(crate) fn signature_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        signature: Option<P<Signature>>,
        subst: &ApiStabilitySubstitution,
    ) -> ApiStabilitySurface {
        let Some(signature) = signature else {
            return new_api_stability_surface();
        };
        let Some(raw) = raw_signature(Some(signature)) else {
            return new_api_stability_surface();
        };
        if api_stability_signature_is_non_public(raw) {
            return new_api_stability_surface();
        }
        let key = ApiStabilitySignatureKey { signature, carrier: subst.carrier() };
        if let Some(cached) = self.session_signatures.get(&key) {
            let cached = cached.clone();
            if !cached.complete {
                self.scope.signatures.add(key);
            }
            return cached;
        }
        if key.carrier.is_none()
            && let Some(shared) = self.shared_signature_surface(tp, Some(signature))
        {
            // Already published, so it is recorded without being queued for
            // publication again.
            self.settle_signatures.push(key);
            self.session_signatures.insert(key, shared.clone());
            return shared;
        }
        self.signature_frames.insert(key, subst.frame.clone());
        if self.active_signatures.contains(&key) {
            return cycle_cut_surface();
        }
        if !self.consume_work() {
            return blocked_surface();
        }
        self.active_signatures.insert(key);
        let start_epoch = self.materialization_epoch;
        let mut surface = self.collect_signature_surface(tp, Some(raw), Some(signature), subst);
        self.active_signatures.remove(&key);
        if surface.blocked {
            surface.observed_epoch = start_epoch;
        }
        self.store_signature_surface(key, surface.clone());
        surface
    }

    // go: internal/typeparser/api_stability.go:3672 apiStabilityAnalysis.collectSignatureSurface 739ce0473ae33dfd
    pub(crate) fn collect_signature_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        raw: Option<P<Signature>>,
        concrete: Option<P<Signature>>,
        subst: &ApiStabilitySubstitution,
    ) -> ApiStabilitySurface {
        let mut surface = new_api_stability_surface();
        let Some(raw) = raw else {
            return surface;
        };
        self.record_signature_stability(tp, &mut surface, Some(raw));

        let mut signature_subst = subst.clone();
        let mut concrete_parameters: &'static [P<Symbol>] = &[];
        let mut concrete_this = None;
        let instantiated = concrete.filter(|&concrete| concrete != raw);
        if let Some(concrete) = instantiated {
            signature_subst = self.extend_signature_substitution(subst, concrete);
            // The concrete signature's type arguments are represented in the
            // enclosing context; a compound argument is traversed shallowly.
            for argument in self.signature_arguments(tp, Some(concrete)) {
                let s =
                    self.type_surface(tp, Some(argument), ApiStabilityInspection::Shallow, subst);
                surface.merge(&s);
            }
            concrete_parameters = concrete.parameters();
            concrete_this = concrete.this_parameter();
        }
        for &type_parameter in raw.type_parameters() {
            if self.substitute(tp, &signature_subst, Some(type_parameter)).is_some() {
                // A bound argument replaces the declaration; its constraint and default
                // are not part of the instantiated surface.
                continue;
            }
            let s = self.type_surface(
                tp,
                Some(type_parameter),
                ApiStabilityInspection::Shallow,
                &signature_subst,
            );
            surface.merge(&s);
        }
        if let Some(this_parameter) = raw.this_parameter() {
            self.collect_signature_parameter(
                tp,
                &mut surface,
                this_parameter,
                concrete_this,
                subst,
                &signature_subst,
            );
        }
        for (index, &parameter) in raw.parameters().iter().enumerate() {
            let concrete_parameter = concrete_parameters.get(index).copied();
            self.collect_signature_parameter(
                tp,
                &mut surface,
                parameter,
                concrete_parameter,
                subst,
                &signature_subst,
            );
        }
        if let Some(concrete) = instantiated {
            // Prefer the concrete signature's represented return: the
            // instantiated return carries the compiler's actual outcome. When the
            // read is cut by a recursive application the raw declaration's
            // uninstantiated return annotation is used with the signature
            // substitution instead.
            let represented = self.return_type_safely(tp, Some(concrete), &signature_subst);
            if represented.is_some() {
                let s = self.type_surface(tp, represented, ApiStabilityInspection::Shallow, subst);
                surface.merge(&s);
            } else {
                self.collect_return_type_surface(tp, &mut surface, Some(raw), &signature_subst);
            }
        } else {
            self.collect_return_type_surface(tp, &mut surface, Some(raw), &signature_subst);
        }
        self.collect_signature_provenance(tp, &mut surface, raw, &signature_subst);
        surface
    }

    // go: internal/typeparser/api_stability.go:3672 apiStabilityAnalysis.collectSignatureSurface 739ce0473ae33dfd
    // Rust: Go's `collectParameter` closure.
    fn collect_signature_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        raw_parameter: P<Symbol>,
        concrete_parameter: Option<P<Symbol>>,
        subst: &ApiStabilitySubstitution,
        signature_subst: &ApiStabilitySubstitution,
    ) {
        // Prefer the concrete signature's represented parameter component: the
        // instantiated parameter carries the compiler's actual outcome. When
        // the read is cut by a recursive application the raw parameter's
        // represented component is used with the signature substitution, so a
        // bare type parameter still maps to its concrete argument without
        // instantiating a hidden recursive alias.
        if concrete_parameter.is_some()
            && let Some(concrete_type) = self.type_of_symbol_safely(tp, concrete_parameter)
        {
            let s =
                self.type_surface(tp, Some(concrete_type), ApiStabilityInspection::Shallow, subst);
            surface.merge(&s);
            return;
        }
        if let Some(parameter_type) = self.parameter_represented_type(tp, Some(raw_parameter)) {
            if parameter_type.is_some() {
                let s = self.type_surface(
                    tp,
                    parameter_type,
                    ApiStabilityInspection::Shallow,
                    signature_subst,
                );
                surface.merge(&s);
            }
        } else {
            surface.block();
        }
    }

    // go: internal/typeparser/api_stability.go:3756 apiStabilityAnalysis.collectReturnTypeSurface 4eddf806484421dc
    // collectReturnTypeSurface inspects the public return type of a signature. The
    // signature's materialized return type is preferred; otherwise it is
    // materialized lazily, with the already resolved annotation node, a primitive
    // keyword, or a bare type parameter binder as fallbacks. A genuinely
    // unavailable return leaves the surface incomplete.
    pub(crate) fn collect_return_type_surface(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        signature: Option<P<Signature>>,
        subst: &ApiStabilitySubstitution,
    ) {
        if signature.is_none() {
            return;
        }
        let Some(represented) = self.return_represented_type(tp, signature, subst) else {
            surface.block();
            return;
        };
        if represented.is_some() {
            let s = self.type_surface(tp, represented, ApiStabilityInspection::Shallow, subst);
            surface.merge(&s);
        }
    }

    // go: internal/typeparser/api_stability.go:3773 apiStabilityAnalysis.returnRepresentedType 203625563f91d125
    // returnRepresentedType returns the return type component of a signature. The
    // boolean is false only when the component is genuinely unavailable, which
    // blocks the surface; a primitive keyword is known and exposes nothing.
    //
    // Rust: Go's `(*Type, bool)`: None is Go's false, Some(None) a known component with no type.
    #[expect(clippy::option_option, reason = "Go's (*Type, bool): the outer None is Go's false")]
    pub(crate) fn return_represented_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        signature: Option<P<Signature>>,
        subst: &ApiStabilitySubstitution,
    ) -> Option<Option<P<Type>>> {
        signature?;
        if let Some(resolved) = self.return_type_safely(tp, signature, subst) {
            return Some(Some(resolved));
        }
        let node = signature_return_type_node(signature)?;
        if let Some(represented) = get_resolved_type_from_type_node(tp.checker, node) {
            return Some(Some(represented));
        }
        if api_stability_primitive_type_node(Some(node)) {
            return Some(None);
        }
        if let Some(binder) = self.type_parameter_from_annotation(tp, Some(node)) {
            return Some(Some(binder));
        }
        None
    }

    // go: internal/typeparser/api_stability.go:3799 apiStabilityAnalysis.parameterTypeOf 0e1f2ba15b47cb17
    // parameterTypeOf returns the represented type of a parameter, or nil when the
    // parameter exposes no type component of its own (a primitive keyword) or is
    // unavailable.
    pub(crate) fn parameter_type_of(
        &mut self,
        tp: &mut TypeParser<'_>,
        parameter: Option<P<Symbol>>,
    ) -> Option<P<Type>> {
        self.parameter_represented_type(tp, parameter).flatten()
    }

    // go: internal/typeparser/api_stability.go:3809 apiStabilityAnalysis.parameterRepresentedType b25b6d4ae1f80fe9
    // parameterRepresentedType returns the represented type component of a
    // parameter. The parameter's materialized type is preferred and otherwise
    // materialized lazily; the already resolved annotation node type and a bare
    // type parameter binder are fallbacks, while a primitive keyword is known and
    // exposes nothing.
    //
    // Rust: Go's `(*Type, bool)`, as in `return_represented_type`.
    #[expect(clippy::option_option, reason = "Go's (*Type, bool): the outer None is Go's false")]
    pub(crate) fn parameter_represented_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        parameter: Option<P<Symbol>>,
    ) -> Option<Option<P<Type>>> {
        let parameter = parameter?;
        if let Some(materialized) =
            get_resolved_type_of_symbol_if_materialized(tp.checker, Some(parameter))
        {
            return Some(Some(materialized));
        }
        if let Some(resolved) = self.type_of_symbol_safely(tp, Some(parameter)) {
            return Some(Some(resolved));
        }
        for &declaration in parameter.declarations() {
            if declaration.kind() != Kind::Parameter {
                continue;
            }
            let Some(annotation) = declaration.as_parameter_declaration().type_() else {
                continue;
            };
            if let Some(represented) = get_resolved_type_from_type_node(tp.checker, annotation) {
                return Some(Some(represented));
            }
            if api_stability_primitive_type_node(Some(annotation)) {
                return Some(None);
            }
            if let Some(binder) = self.type_parameter_from_annotation(tp, Some(annotation)) {
                return Some(Some(binder));
            }
        }
        None
    }

    // go: internal/typeparser/api_stability.go:3844 apiStabilityAnalysis.typeParameterFromAnnotation ce77bfbfd8f73410
    // typeParameterFromAnnotation returns the declared type parameter a type
    // annotation names, when the annotation is exactly a bare type parameter
    // reference. It reads the annotation symbol only; no annotation type is
    // resolved.
    pub(crate) fn type_parameter_from_annotation(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Option<P<Node>>,
    ) -> Option<P<Type>> {
        let node = node?;
        if node.kind() != Kind::TypeReference {
            return None;
        }
        let reference = node.as_type_reference_node();
        if let Some(arguments) = reference.type_arguments()
            && !arguments.nodes().is_empty()
        {
            return None;
        }
        let symbol = self.symbol_at_type_name_node(tp, Some(reference.type_name()))?;
        if !symbol.flags().intersects(SymbolFlags::TypeParameter) {
            return None;
        }
        self.note_materializing_symbol_read(
            ApiStabilityMaterializationReadKind::DeclaredType,
            Some(symbol),
        );
        Some(tp.checker.get_declared_type_of_symbol_exported(symbol))
    }

    // go: internal/typeparser/api_stability.go:3863 apiStabilityAnalysis.signatureArguments 24821093f18cd8a4
    // signatureArguments returns the checker-supplied type arguments of an
    // instantiated signature, or nil when the signature has no represented
    // substitution. The result is memoized for the analysis.
    pub(crate) fn signature_arguments(
        &mut self,
        tp: &mut TypeParser<'_>,
        signature: Option<P<Signature>>,
    ) -> Vec<P<Type>> {
        let Some(signature) = signature else {
            return Vec::new();
        };
        if let Some(cached) = self.signature_args.get(&signature) {
            return cached.clone();
        }
        let arguments = get_type_arguments_for_resolved_signature(tp.checker, Some(signature))
            .unwrap_or_default();
        self.signature_args.insert(signature, arguments.clone());
        arguments
    }

    // go: internal/typeparser/api_stability.go:3881 apiStabilityAnalysis.substitute 89bc84045da35012
    // substitute resolves a type parameter through the active substitution chain.
    // A compiler mapper maps the parameter directly; a concrete signature maps its
    // own parameters through the concrete type arguments or its instantiated
    // parameter mappers; a reference maps its target's parameters through the
    // represented arguments. A parameter replaced by an unrepresented argument
    // returns a nil mapping with ok set, which blocks the surface.
    //
    // Rust: Go's `(*Type, apiStabilitySubstitution, bool)`: None when the parameter is not replaced (Go returns
    // the input substitution with false), else the mapping (None for an unrepresented argument) and the parent.
    pub(crate) fn substitute(
        &mut self,
        tp: &mut TypeParser<'_>,
        subst: &ApiStabilitySubstitution,
        t: Option<P<Type>>,
    ) -> Option<(Option<P<Type>>, ApiStabilitySubstitution)> {
        let mut t = t?;
        if !t.flags().intersects(TypeFlags::TypeParameter) {
            return None;
        }
        // A distributive conditional check is the distributed clone of its declared
        // type parameter; its constraint points back at the declaration binder. All
        // frames map the declared binder, so match through it.
        if let Some(normalized) = get_non_distributed_type_parameter(Some(t))
            && normalized != t
        {
            t = normalized;
        }
        let mut frame = subst.frame.as_deref();
        while let Some(current) = frame {
            let parent = ApiStabilitySubstitution { frame: current.parent.clone() };
            if let Some(mapper) = current.mapper {
                let mapped = mapper.map(tp.checker, t);
                if mapped != t {
                    return Some((Some(mapped), parent));
                }
            }
            if let Some(signature) = current.signature
                && let Some(mapped) = self.signature_type_parameter(tp, signature, t)
                && mapped != Some(t)
            {
                return Some((mapped, parent));
            }
            if let Some(parameters) = current.parameters {
                for (index, &parameter) in parameters.iter().enumerate() {
                    if parameter != t || index >= current.arguments.len() {
                        continue;
                    }
                    let Some(mapped) = current.arguments[index] else {
                        // The argument was not represented. The parameter is replaced,
                        // but its value is unknown, so the component is unknown.
                        return Some((None, parent));
                    };
                    if mapped == t {
                        continue;
                    }
                    return Some((Some(mapped), parent));
                }
            }
            frame = current.parent.as_deref();
        }
        None
    }

    // go: internal/typeparser/api_stability.go:3924 apiStabilityAnalysis.signatureTypeParameter 775bfa36ecb76106
    // Rust: Go's `(*Type, bool)`: None is Go's false.
    #[expect(clippy::option_option, reason = "Go's (*Type, bool): the outer None is Go's false")]
    pub(crate) fn signature_type_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        signature: P<Signature>,
        t: P<Type>,
    ) -> Option<Option<P<Type>>> {
        let raw = raw_signature(Some(signature))?;
        let arguments = self.signature_arguments(tp, Some(signature));
        if !arguments.is_empty() {
            let parameters = raw.type_parameters();
            for (index, &parameter) in parameters.iter().enumerate() {
                if parameter == t && index < arguments.len() {
                    return Some(Some(arguments[index]));
                }
            }
        }
        if raw == signature {
            return None;
        }
        self.signature_bound_type_parameter(tp, Some(signature), Some(t)).map(Some)
    }

    // go: internal/typeparser/api_stability.go:3949 apiStabilityAnalysis.signatureBoundTypeParameter 0ee4a7802535bf52
    // signatureBoundTypeParameter resolves a bare type parameter through an
    // instantiated signature's own parameter and return types. It only reads a
    // concrete type when the corresponding target is itself a bare type parameter,
    // and it maps the binder forward with the instantiated symbol's own mapper
    // when the concrete type has not been materialized, so a recursive compound
    // member is never evaluated. The derived map is memoized per analysis.
    pub(crate) fn signature_bound_type_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        signature: Option<P<Signature>>,
        t: Option<P<Type>>,
    ) -> Option<P<Type>> {
        let signature = signature?;
        let t = t?;
        if !t.flags().intersects(TypeFlags::TypeParameter) {
            return None;
        }
        let raw = raw_signature(Some(signature))?;
        if raw == signature {
            return None;
        }
        if !self.signature_parameter_cache.contains_key(&signature) {
            let bound = self.compute_signature_parameter_map(tp, raw, signature);
            self.signature_parameter_cache.insert(signature, bound);
        }
        self.signature_parameter_cache.get(&signature)?.get(&t).copied()
    }

    // go: internal/typeparser/api_stability.go:3966 apiStabilityAnalysis.computeSignatureParameterMap 021174ee99427691
    pub(crate) fn compute_signature_parameter_map(
        &mut self,
        tp: &mut TypeParser<'_>,
        raw: P<Signature>,
        concrete: P<Signature>,
    ) -> FxHashMap<P<Type>, P<Type>> {
        let mut bound = FxHashMap::default();
        let raw_parameters = raw.parameters();
        let concrete_parameters = concrete.parameters();
        if raw_parameters.len() == concrete_parameters.len() {
            for index in 0..raw_parameters.len() {
                self.bind_signature_parameter(
                    tp,
                    &mut bound,
                    Some(raw_parameters[index]),
                    Some(concrete_parameters[index]),
                );
            }
        }
        self.bind_signature_parameter(
            tp,
            &mut bound,
            raw.this_parameter(),
            concrete.this_parameter(),
        );
        let raw_return =
            self.parameter_return_type(tp, Some(raw), &ApiStabilitySubstitution::default());
        if let Some(raw_return) = raw_return
            && raw_return.flags().intersects(TypeFlags::TypeParameter)
        {
            let concrete_return =
                get_resolved_return_type_of_signature_if_materialized(Some(concrete));
            if let Some(concrete_return) = concrete_return
                && concrete_return != raw_return
            {
                bound.insert(raw_return, concrete_return);
            }
        }
        bound
    }

    // go: internal/typeparser/api_stability.go:3966 apiStabilityAnalysis.computeSignatureParameterMap 021174ee99427691
    // Rust: Go's `bind` closure.
    fn bind_signature_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        bound: &mut FxHashMap<P<Type>, P<Type>>,
        raw_parameter: Option<P<Symbol>>,
        concrete_parameter: Option<P<Symbol>>,
    ) {
        let (Some(raw_parameter), Some(concrete_parameter)) = (raw_parameter, concrete_parameter)
        else {
            return;
        };
        let Some(raw_type) = self.parameter_type_of(tp, Some(raw_parameter)) else {
            return;
        };
        if !raw_type.flags().intersects(TypeFlags::TypeParameter) {
            return;
        }
        let mut concrete_type =
            get_resolved_type_of_symbol_if_materialized(tp.checker, Some(concrete_parameter));
        if concrete_type.is_none() {
            // Prefer the instantiated symbol's own mapper: it maps the binder
            // forward without instantiating a compound annotation.
            if let Some(mapper) =
                get_instantiated_symbol_mapper(tp.checker, Some(concrete_parameter))
            {
                concrete_type = Some(mapper.map(tp.checker, raw_type));
            }
        }
        if let Some(concrete_type) = concrete_type
            && concrete_type != raw_type
        {
            bound.insert(raw_type, concrete_type);
        }
    }

    // go: internal/typeparser/api_stability.go:4010 apiStabilityAnalysis.parameterReturnType 547ee3fa48f2ae60
    // parameterReturnType returns the represented return type of a signature: the
    // signature's materialized return type, the already resolved annotation node
    // type, or the declared binder when the return annotation is exactly a type
    // parameter reference.
    pub(crate) fn parameter_return_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        signature: Option<P<Signature>>,
        subst: &ApiStabilitySubstitution,
    ) -> Option<P<Type>> {
        self.return_represented_type(tp, signature, subst).flatten()
    }

    // go: internal/typeparser/api_stability.go:4021 apiStabilityAnalysis.collectDeclarationProvenance 2589ae172c9e4be8
    // collectDeclarationProvenance records the alias symbols written in a public
    // declaration's own type annotations. The checker erases an alias such as
    // `type ExperimentalText = string`, so symbol provenance is the only way to
    // name it. The traversal is driven by the represented checker type and never
    // descends into conditional branches, so a symbol that survives only in a
    // discarded branch is not reported.
    pub(crate) fn collect_declaration_provenance(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: Option<P<Node>>,
        represented: Option<P<Type>>,
        subst: &ApiStabilitySubstitution,
    ) {
        let Some(declaration) = declaration else {
            return;
        };
        if represented.is_none()
            || api_stability_property_declaration_is_internal(Some(declaration))
        {
            return;
        }
        if let Some(function_like) = declaration.function_like_data() {
            match declaration.kind() {
                Kind::GetAccessor => {
                    self.collect_type_node_provenance(
                        tp,
                        surface,
                        function_like.type_(),
                        represented,
                        subst,
                    );
                }
                Kind::SetAccessor => {
                    if let Some(parameters) = function_like.parameters() {
                        for &parameter in parameters.nodes() {
                            if parameter.kind() != Kind::Parameter {
                                continue;
                            }
                            self.collect_type_node_provenance(
                                tp,
                                surface,
                                parameter.as_parameter_declaration().type_(),
                                represented,
                                subst,
                            );
                        }
                    }
                }
                _ => {}
            }
            return;
        }
        match declaration.kind() {
            Kind::VariableDeclaration => self.collect_type_node_provenance(
                tp,
                surface,
                declaration.as_variable_declaration().type_(),
                represented,
                subst,
            ),
            Kind::PropertyDeclaration => self.collect_type_node_provenance(
                tp,
                surface,
                declaration.as_property_declaration().type_(),
                represented,
                subst,
            ),
            Kind::PropertySignature => self.collect_type_node_provenance(
                tp,
                surface,
                declaration.as_property_signature_declaration().type_(),
                represented,
                subst,
            ),
            Kind::TypeAliasDeclaration | Kind::JSTypeAliasDeclaration => self
                .collect_type_node_provenance(
                    tp,
                    surface,
                    declaration.type_node(),
                    represented,
                    subst,
                ),
            Kind::IndexSignature => self.collect_type_node_provenance(
                tp,
                surface,
                declaration.as_index_signature_declaration().type_(),
                represented,
                subst,
            ),
            _ => {}
        }
    }

    // go: internal/typeparser/api_stability.go:4059 apiStabilityAnalysis.collectSignatureProvenance c57404007fedb79e
    // collectSignatureProvenance records erased alias symbols in a signature's
    // public annotations. A type parameter replaced by a represented argument is
    // skipped together with its constraint and default, matching the represented
    // instantiated surface.
    pub(crate) fn collect_signature_provenance(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        raw: P<Signature>,
        signature_subst: &ApiStabilitySubstitution,
    ) {
        let Some(declaration) = raw.declaration() else {
            return;
        };
        let Some(function_like) = declaration.function_like_data() else {
            return;
        };
        if let Some(type_parameters) = function_like.type_parameters() {
            let raw_type_parameters = raw.type_parameters();
            for (index, &type_parameter) in type_parameters.nodes().iter().enumerate() {
                if type_parameter.kind() != Kind::TypeParameter
                    || index >= raw_type_parameters.len()
                {
                    continue;
                }
                if self.substitute(tp, signature_subst, Some(raw_type_parameters[index])).is_some()
                {
                    continue;
                }
                let type_parameter_declaration = type_parameter.as_type_parameter_declaration();
                let constraint = get_resolved_constraint_of_type_parameter_if_materialized(
                    tp.checker,
                    Some(raw_type_parameters[index]),
                );
                self.collect_type_node_provenance(
                    tp,
                    surface,
                    type_parameter_declaration.constraint(),
                    constraint,
                    signature_subst,
                );
                let default_type = get_resolved_default_from_type_parameter_if_materialized(
                    tp.checker,
                    Some(raw_type_parameters[index]),
                );
                self.collect_type_node_provenance(
                    tp,
                    surface,
                    type_parameter_declaration.default_type(),
                    default_type,
                    signature_subst,
                );
            }
        }
        let raw_parameters = raw.parameters();
        if let Some(parameters) = function_like.parameters() {
            let mut parameter_index = 0;
            for &parameter in parameters.nodes() {
                if parameter.kind() != Kind::Parameter {
                    continue;
                }
                let parameter_declaration = parameter.as_parameter_declaration();
                let mut represented = None;
                if parameter_index < raw_parameters.len() {
                    represented = self.parameter_type_of(tp, Some(raw_parameters[parameter_index]));
                }
                self.collect_type_node_provenance(
                    tp,
                    surface,
                    parameter_declaration.type_(),
                    represented,
                    signature_subst,
                );
                parameter_index += 1;
            }
        }
        let return_type = self.parameter_return_type(tp, Some(raw), signature_subst);
        self.collect_type_node_provenance(
            tp,
            surface,
            function_like.type_(),
            return_type,
            signature_subst,
        );
    }

    // go: internal/typeparser/api_stability.go:4128 apiStabilityAnalysis.collectTypeNodeProvenance 31d958901c5621eb
    // collectTypeNodeProvenance records alias symbols named by type annotation
    // nodes whose represented checker type is directly reachable. It is
    // deliberately structural and correlated with the represented types: unions,
    // intersections, tuples, arrays, reference arguments and literal members are
    // only descended when the corresponding represented component exists, and
    // conditional branches are never descended into. A union or intersection
    // constituent is correlated by concrete component identity, so a retained
    // constituent whose alias the compiler collapsed or reordered is still named.
    // An annotation whose represented type is unavailable records nothing, which
    // keeps the walk from becoming a syntax dependency parser.
    pub(crate) fn collect_type_node_provenance(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        node: Option<P<Node>>,
        represented: Option<P<Type>>,
        subst: &ApiStabilitySubstitution,
    ) {
        let (Some(node), Some(represented)) = (node, represented) else {
            return;
        };
        match node.kind() {
            Kind::ParenthesizedType => self.collect_type_node_provenance(
                tp,
                surface,
                Some(node.as_parenthesized_type_node().type_()),
                Some(represented),
                subst,
            ),
            Kind::TypeReference => {
                let reference = node.as_type_reference_node();
                let symbol = self.symbol_at_type_name_node(tp, Some(reference.type_name()));
                self.record_symbol(tp, surface, symbol);
                if api_stability_represented_surface_accepts_arguments(Some(represented)) {
                    self.collect_type_argument_provenance(
                        tp,
                        surface,
                        reference.type_arguments(),
                        represented,
                        subst,
                    );
                }
            }
            Kind::ExpressionWithTypeArguments => {
                let expression = node.as_expression_with_type_arguments();
                let symbol = self.symbol_at_type_name_node(tp, Some(expression.expression()));
                self.record_symbol(tp, surface, symbol);
                if api_stability_represented_surface_accepts_arguments(Some(represented)) {
                    self.collect_type_argument_provenance(
                        tp,
                        surface,
                        expression.type_arguments(),
                        represented,
                        subst,
                    );
                }
            }
            Kind::TypeQuery => {
                let symbol =
                    self.symbol_at_type_name_node(tp, Some(node.as_type_query_node().expr_name()));
                self.record_symbol(tp, surface, symbol);
            }
            Kind::ImportType => {
                let import_type = node.as_import_type_node();
                if let Some(qualifier) = import_type.qualifier() {
                    let symbol = self.symbol_at_type_name_node(tp, Some(qualifier));
                    self.record_symbol(tp, surface, symbol);
                }
                if api_stability_represented_surface_accepts_arguments(Some(represented)) {
                    self.collect_type_argument_provenance(
                        tp,
                        surface,
                        import_type.type_arguments(),
                        represented,
                        subst,
                    );
                }
            }
            Kind::UnionType => {
                let list = node.as_union_type_node().types();
                self.collect_aggregate_type_node_provenance(tp, surface, list, represented, subst);
            }
            Kind::IntersectionType => {
                let list = node.as_intersection_type_node().types();
                self.collect_aggregate_type_node_provenance(tp, surface, list, represented, subst);
            }
            Kind::ArrayType => {
                let element = node.as_array_type_node().element_type();
                let element_type = self.represented_element_type(Some(represented));
                self.collect_type_node_provenance(tp, surface, Some(element), element_type, subst);
            }
            Kind::TupleType => {
                let elements = node.as_tuple_type_node().elements();
                let represented_elements = self.represented_tuple_arguments(Some(represented));
                for (index, &element) in elements.nodes().iter().enumerate() {
                    let element_type = represented_elements.get(index).copied();
                    self.collect_type_node_provenance(
                        tp,
                        surface,
                        Some(element),
                        element_type,
                        subst,
                    );
                }
            }
            Kind::TypeOperator => self.collect_type_node_provenance(
                tp,
                surface,
                Some(node.as_type_operator_node().type_()),
                Some(represented),
                subst,
            ),
            Kind::IndexedAccessType => {
                let indexed = node.as_indexed_access_type_node();
                let (object_type, index_type) =
                    api_stability_represented_indexed_access(Some(represented));
                self.collect_type_node_provenance(
                    tp,
                    surface,
                    Some(indexed.object_type()),
                    object_type,
                    subst,
                );
                self.collect_type_node_provenance(
                    tp,
                    surface,
                    Some(indexed.index_type()),
                    index_type,
                    subst,
                );
            }
            Kind::ConditionalType => {
                let conditional = node.as_conditional_type_node();
                let (check_type, extends_type) =
                    api_stability_represented_conditional_parts(Some(represented));
                self.collect_type_node_provenance(
                    tp,
                    surface,
                    Some(conditional.check_type()),
                    check_type,
                    subst,
                );
                self.collect_type_node_provenance(
                    tp,
                    surface,
                    Some(conditional.extends_type()),
                    extends_type,
                    subst,
                );
            }
            Kind::FunctionType | Kind::ConstructorType => {
                self.collect_function_type_provenance(tp, surface, node, Some(represented), subst);
            }
            Kind::TypeLiteral => {
                self.collect_type_literal_provenance(tp, surface, node, Some(represented), subst);
            }
            Kind::MappedType => {
                let mapped = node.as_mapped_type_node();
                let (mut constraint, mut name_type, mut template_type) = (None, None, None);
                if represented.object_flags().intersects(ObjectFlags::Mapped) {
                    constraint = get_mapped_type_constraint_type(represented);
                    name_type = get_mapped_type_name_type(represented);
                    template_type = get_mapped_type_template_type(represented);
                }
                {
                    let type_parameter = mapped.type_parameter();
                    // The mapped parameter's constraint is the mapped constraint,
                    // already paired above; this pairing only names erased aliases.
                    self.collect_type_node_provenance(
                        tp,
                        surface,
                        type_parameter.as_type_parameter_declaration().constraint(),
                        constraint,
                        subst,
                    );
                }
                self.collect_type_node_provenance(
                    tp,
                    surface,
                    mapped.name_type(),
                    name_type,
                    subst,
                );
                self.collect_type_node_provenance(
                    tp,
                    surface,
                    mapped.type_(),
                    template_type,
                    subst,
                );
            }
            _ => {}
        }
    }
}

// go: internal/typeparser/api_stability.go:4218 apiStabilityRepresentedMembers 7be3d6d42e58b27c
pub(crate) fn api_stability_represented_members(t: Option<P<Type>>) -> Vec<P<Type>> {
    let Some(t) = t else {
        return Vec::new();
    };
    if t.flags().intersects(TypeFlags::UnionOrIntersection) {
        return t.types().to_vec();
    }
    // Compiler normalization collapsed the aggregate to a single constituent;
    // that constituent is the only retained component.
    vec![t]
}

// go: internal/typeparser/api_stability.go:4232 apiStabilityRepresentedContainsType d661541f527bde38
// apiStabilityRepresentedContainsType reports whether the normalized aggregate
// retained a component with the exact compiler identity of t.
pub(crate) fn api_stability_represented_contains_type(
    members: &[P<Type>],
    t: Option<P<Type>>,
) -> bool {
    let Some(t) = t else {
        return false;
    };
    members.contains(&t)
}

// go: internal/typeparser/api_stability.go:4244 apiStabilityRepresentedContainsAggregate e57d533ff3fea5d7
// apiStabilityRepresentedContainsAggregate reports whether every constituent of
// a written union or intersection was retained in the normalized aggregate.
// Compiler normalization flattens nested aggregates, so a written constituent
// such as `A = string | number` is retained as its individual members rather
// than as one aggregate component.
pub(crate) fn api_stability_represented_contains_aggregate(
    members: &[P<Type>],
    t: Option<P<Type>>,
) -> bool {
    let constituents = api_stability_represented_members(t);
    if constituents.is_empty() {
        return false;
    }
    for constituent in constituents {
        if !api_stability_represented_contains_type(members, Some(constituent)) {
            return false;
        }
    }
    true
}

impl ApiStabilityAnalysis {
    // go: internal/typeparser/api_stability.go:4267 apiStabilityAnalysis.collectAggregateTypeNodeProvenance 1caaaebc58a7a6c2
    // collectAggregateTypeNodeProvenance correlates the written constituents of a
    // union or intersection annotation with the represented aggregate by concrete
    // component identity. Compiler normalization can remove, collapse, reorder or
    // combine constituents, so pairing by position would attach the wrong
    // represented component. A constituent whose resolved type is retained in the
    // normalized aggregate is descended so its erased alias stays named; a
    // constituent eliminated by normalization records nothing. The node type is
    // read through the ordinary lazy accessor, correlated with a retained
    // component, and only then walked; the node itself never introduces a
    // dependency the represented type does not contain.
    pub(crate) fn collect_aggregate_type_node_provenance(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        list: P<NodeList>,
        represented: P<Type>,
        subst: &ApiStabilitySubstitution,
    ) {
        let members = api_stability_represented_members(Some(represented));
        for &member in list.nodes() {
            let member_type = self.represented_type_from_node(tp, Some(member), subst);
            if member_type.is_none()
                || (!api_stability_represented_contains_type(&members, member_type)
                    && !api_stability_represented_contains_aggregate(&members, member_type))
            {
                continue;
            }
            self.collect_type_node_provenance(tp, surface, Some(member), member_type, subst);
        }
    }
}

// go: internal/typeparser/api_stability.go:4282 apiStabilityRepresentedIndexedAccess 5c4a0f58b427d1b0
pub(crate) fn api_stability_represented_indexed_access(
    t: Option<P<Type>>,
) -> (Option<P<Type>>, Option<P<Type>>) {
    if let Some(t) = t
        && t.flags().intersects(TypeFlags::IndexedAccess)
    {
        let indexed = t.as_indexed_access_type();
        return (indexed.object_type(), indexed.index_type());
    }
    (None, None)
}

// go: internal/typeparser/api_stability.go:4291 apiStabilityRepresentedConditionalParts cd3e649dabfe6532
pub(crate) fn api_stability_represented_conditional_parts(
    t: Option<P<Type>>,
) -> (Option<P<Type>>, Option<P<Type>>) {
    if let Some(t) = t
        && t.flags().intersects(TypeFlags::Conditional)
    {
        let conditional = t.as_conditional_type();
        return (conditional.check_type(), conditional.extends_type());
    }
    (None, None)
}

// go: internal/typeparser/api_stability.go:4305 apiStabilityRepresentedSurfaceAcceptsArguments 61f9c40cd7bc8523
// apiStabilityRepresentedSurfaceAcceptsArguments reports whether the written
// type arguments of a named reference are part of the represented surface. An
// application the checker evaluated away (an eliminated conditional resolving
// to `never` or to a primitive) exposes none of its arguments, so recording
// them would report a symbol the surface cannot expose.
pub(crate) fn api_stability_represented_surface_accepts_arguments(
    represented: Option<P<Type>>,
) -> bool {
    let Some(represented) = represented else {
        return true;
    };
    !represented.flags().intersects(
        TypeFlags::Never
            | TypeFlags::String
            | TypeFlags::Number
            | TypeFlags::Boolean
            | TypeFlags::BigInt
            | TypeFlags::ESSymbol
            | TypeFlags::Void
            | TypeFlags::Undefined
            | TypeFlags::Null,
    )
}

impl ApiStabilityAnalysis {
    // go: internal/typeparser/api_stability.go:4315 apiStabilityAnalysis.collectTypeArgumentProvenance 1ab36b6573c6e37d
    pub(crate) fn collect_type_argument_provenance(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        arguments: Option<P<NodeList>>,
        represented: P<Type>,
        subst: &ApiStabilitySubstitution,
    ) {
        let Some(arguments) = arguments else {
            return;
        };
        if arguments.nodes().is_empty() {
            return;
        }
        let represented_arguments = self.represented_type_arguments(Some(represented));
        for (index, &argument) in arguments.nodes().iter().enumerate() {
            let argument_type = represented_arguments.get(index).copied();
            self.collect_type_node_provenance(tp, surface, Some(argument), argument_type, subst);
        }
    }

    // go: internal/typeparser/api_stability.go:4332 apiStabilityAnalysis.collectFunctionTypeProvenance 55a27437857f3460
    // collectFunctionTypeProvenance pairs a function-like annotation's parameter
    // and return nodes with the represented signature when one is available. When
    // it is not, the annotation records nothing: nodes are never resolved here.
    pub(crate) fn collect_function_type_provenance(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        node: P<Node>,
        represented: Option<P<Type>>,
        subst: &ApiStabilitySubstitution,
    ) {
        let mut kind = SignatureKind::Call;
        if node.kind() == Kind::ConstructorType {
            kind = SignatureKind::Construct;
        }
        let mut raw = None;
        if represented.is_some()
            && let Some(signatures) =
                get_resolved_signatures_of_type_if_materialized(represented, kind)
            && let Some(&first) = signatures.first()
        {
            raw = raw_signature(Some(first));
        }
        let Some(raw) = raw else {
            return;
        };
        if let Some(function_like) = node.function_like_data()
            && let Some(type_parameters) = function_like.type_parameters()
        {
            let raw_type_parameters = raw.type_parameters();
            for (index, &type_parameter) in type_parameters.nodes().iter().enumerate() {
                if type_parameter.kind() != Kind::TypeParameter
                    || index >= raw_type_parameters.len()
                {
                    continue;
                }
                let type_parameter_declaration = type_parameter.as_type_parameter_declaration();
                let constraint = get_resolved_constraint_of_type_parameter_if_materialized(
                    tp.checker,
                    Some(raw_type_parameters[index]),
                );
                self.collect_type_node_provenance(
                    tp,
                    surface,
                    type_parameter_declaration.constraint(),
                    constraint,
                    subst,
                );
                let default_type = get_resolved_default_from_type_parameter_if_materialized(
                    tp.checker,
                    Some(raw_type_parameters[index]),
                );
                self.collect_type_node_provenance(
                    tp,
                    surface,
                    type_parameter_declaration.default_type(),
                    default_type,
                    subst,
                );
            }
        }
        let raw_parameters = raw.parameters();
        for (index, &parameter) in node.parameters().iter().enumerate() {
            if parameter.kind() != Kind::Parameter {
                continue;
            }
            let mut parameter_type = None;
            if index < raw_parameters.len() {
                parameter_type = self.parameter_type_of(tp, Some(raw_parameters[index]));
            }
            self.collect_type_node_provenance(
                tp,
                surface,
                parameter.as_parameter_declaration().type_(),
                parameter_type,
                subst,
            );
        }
        let return_type = self.parameter_return_type(tp, Some(raw), subst);
        self.collect_type_node_provenance(tp, surface, node.type_node(), return_type, subst);
    }
}

// go: internal/typeparser/api_stability.go:4384 apiStabilityMemberSymbolForDeclaration 344c7d989292f2d0
// apiStabilityMemberSymbolForDeclaration returns the member symbol a
// declaration contributes to an already resolved member table. A simply named
// declaration is looked up by its member-table key. A computed declaration is
// never asked for its name text: the table is scanned for the symbol whose
// declarations include the member, which keeps the pairing with the concrete
// instantiated member (and its signature) the table belongs to. The scan runs
// only for a computed declaration, so simply named members keep the direct
// lookup.
pub(crate) fn api_stability_member_symbol_for_declaration(
    members: Option<P<SymbolTable>>,
    member: Option<P<Node>>,
) -> Option<P<Symbol>> {
    let member = member?;
    members?;
    if let Some(name) = ets_shim::ast::get_name_of_declaration(member)
        && let Some(simple) = api_stability_simple_name_text(Some(name))
    {
        return symbol_table_get(members, simple);
    }
    for key in sorted_symbol_table_keys(members) {
        let Some(candidate) = symbol_table_get(members, key) else {
            continue;
        };
        if candidate.declarations().contains(&member) {
            return Some(candidate);
        }
    }
    None
}

impl ApiStabilityAnalysis {
    // go: internal/typeparser/api_stability.go:4411 apiStabilityAnalysis.collectTypeLiteralProvenance 97aa19134a4331e8
    // collectTypeLiteralProvenance pairs a type literal's member annotations with
    // the represented member types when the literal's member surface has been
    // materialized. An unmaterialized member records nothing instead of being
    // resolved. A computed member is paired by declaration identity, so its
    // concrete instantiated member and signature are preserved without evaluating
    // the computed name.
    pub(crate) fn collect_type_literal_provenance(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        node: P<Node>,
        represented: Option<P<Type>>,
        subst: &ApiStabilitySubstitution,
    ) {
        let mut members = get_resolved_members_of_type_if_materialized(represented);
        if members.is_none()
            && let Some(represented) = represented
            && let Some(symbol) = represented.symbol()
        {
            let table = self.materialized_members_of_symbol(tp.checker, symbol);
            members = table.map(Some);
        }
        let Some(members) = members else {
            return;
        };
        for &member in node.members() {
            if api_stability_property_declaration_is_internal(Some(member)) {
                continue;
            }
            match member.kind() {
                Kind::PropertySignature => {
                    let property_signature = member.as_property_signature_declaration();
                    let mut property_type = None;
                    if let Some(property) =
                        api_stability_member_symbol_for_declaration(members, Some(member))
                    {
                        // The concrete instantiated member is read through the ordinary
                        // guarded accessor: a materialized component is used directly,
                        // an unresolved one is materialized only when the guard
                        // establishes that the read is bounded. This is the same lazy
                        // path a simply named member takes through memberSurface.
                        property_type = self.type_of_symbol_safely(tp, Some(property));
                    }
                    self.collect_type_node_provenance(
                        tp,
                        surface,
                        property_signature.type_(),
                        property_type,
                        subst,
                    );
                }
                Kind::MethodSignature => {
                    let Some(property) =
                        api_stability_member_symbol_for_declaration(members, Some(member))
                    else {
                        continue;
                    };
                    if let Some(property_type) = self.type_of_symbol_safely(tp, Some(property))
                        && let Some(signatures) = get_resolved_signatures_of_type_if_materialized(
                            Some(property_type),
                            SignatureKind::Call,
                        )
                        && let Some(&first) = signatures.first()
                    {
                        self.collect_function_signature_provenance(
                            tp,
                            surface,
                            Some(member),
                            raw_signature(Some(first)),
                            subst,
                        );
                        continue;
                    }
                    // A method's function type exists but its structured signatures have
                    // not been materialized. The raw declaration signature is created
                    // through the same guarded signature accessor an ordinary method
                    // member uses.
                    if !self.signature_member_resolution_is_safe(tp, Some(property), subst) {
                        continue;
                    }
                    let signatures = self.materialized_signatures_of_symbol(tp.checker, property);
                    if let Some(&first) = signatures.first() {
                        self.collect_function_signature_provenance(
                            tp,
                            surface,
                            Some(member),
                            raw_signature(Some(first)),
                            subst,
                        );
                    }
                }
                Kind::CallSignature | Kind::ConstructSignature => {
                    let mut kind = SignatureKind::Call;
                    if member.kind() == Kind::ConstructSignature {
                        kind = SignatureKind::Construct;
                    }
                    if let Some(signatures) =
                        get_resolved_signatures_of_type_if_materialized(represented, kind)
                        && let Some(&first) = signatures.first()
                    {
                        self.collect_function_signature_provenance(
                            tp,
                            surface,
                            Some(member),
                            raw_signature(Some(first)),
                            subst,
                        );
                    }
                }
                Kind::IndexSignature => {
                    if let Some(infos) =
                        get_resolved_index_infos_of_type_if_materialized(represented)
                    {
                        for &info in infos {
                            if info.declaration() == Some(member) {
                                self.collect_type_node_provenance(
                                    tp,
                                    surface,
                                    member.as_index_signature_declaration().type_(),
                                    info.value_type.get(),
                                    subst,
                                );
                                break;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // go: internal/typeparser/api_stability.go:4482 apiStabilityAnalysis.collectFunctionSignatureProvenance dcf607f34a2e50a1
    // collectFunctionSignatureProvenance pairs a function-like declaration's
    // annotation nodes with the represented signature it declares.
    pub(crate) fn collect_function_signature_provenance(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: Option<P<Node>>,
        raw: Option<P<Signature>>,
        subst: &ApiStabilitySubstitution,
    ) {
        let (Some(declaration), Some(raw)) = (declaration, raw) else {
            return;
        };
        let Some(function_like) = declaration.function_like_data() else {
            return;
        };
        let raw_parameters = raw.parameters();
        if let Some(parameters) = function_like.parameters() {
            let mut parameter_index = 0;
            for &parameter in parameters.nodes() {
                if parameter.kind() != Kind::Parameter {
                    continue;
                }
                let parameter_declaration = parameter.as_parameter_declaration();
                let mut parameter_type = None;
                if parameter_index < raw_parameters.len() {
                    parameter_type =
                        self.parameter_type_of(tp, Some(raw_parameters[parameter_index]));
                }
                self.collect_type_node_provenance(
                    tp,
                    surface,
                    parameter_declaration.type_(),
                    parameter_type,
                    subst,
                );
                parameter_index += 1;
            }
        }
        let return_type = self.parameter_return_type(tp, Some(raw), subst);
        self.collect_type_node_provenance(tp, surface, function_like.type_(), return_type, subst);
    }

    // go: internal/typeparser/api_stability.go:4515 apiStabilityAnalysis.representedTypeArguments a8868cf48f342ca1
    // representedTypeArguments returns the already recorded type arguments of a
    // reference. A deferred reference whose arguments the checker has not
    // materialized returns nil; nothing is resolved here.
    #[expect(clippy::unused_self, reason = "a method of Go's apiStabilityAnalysis")]
    pub(crate) fn represented_type_arguments(&self, t: Option<P<Type>>) -> &'static [P<Type>] {
        get_resolved_type_arguments(t)
    }

    // go: internal/typeparser/api_stability.go:4521 apiStabilityAnalysis.representedElementType cfeaadecbf4b9c97
    // representedElementType returns the represented element type of an array or
    // tuple reference.
    pub(crate) fn represented_element_type(&self, t: Option<P<Type>>) -> Option<P<Type>> {
        self.represented_type_arguments(t).first().copied()
    }

    // go: internal/typeparser/api_stability.go:4530 apiStabilityAnalysis.representedTupleArguments 05b132451e5398cf
    // representedTupleArguments returns the represented element types of a tuple
    // reference without instantiating anything.
    pub(crate) fn represented_tuple_arguments(&self, t: Option<P<Type>>) -> &'static [P<Type>] {
        let Some(t) = t else {
            return &[];
        };
        if !t.flags().intersects(TypeFlags::Object)
            || !t.object_flags().intersects(ObjectFlags::Reference)
        {
            return &[];
        }
        self.represented_type_arguments(Some(t))
    }

    // go: internal/typeparser/api_stability.go:4540 apiStabilityAnalysis.recordSymbol 726572a763c93382
    // recordSymbol records a symbol whose declared stability is above stable. A
    // stable symbol can never exceed any owner ceiling, so it is omitted to keep
    // each surface small.
    pub(crate) fn record_symbol(
        &self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        symbol: Option<P<Symbol>>,
    ) {
        let Some(symbol) = symbol else {
            return;
        };
        let declared = tp.declared_api_stability_of_symbol(symbol);
        if declared.level <= ApiStabilityLevel::Stable {
            return;
        }
        self.add_finding(
            surface,
            ApiStabilityFindingKey { symbol: Some(symbol), ..Default::default() },
            declared,
        );
    }

    // go: internal/typeparser/api_stability.go:4554 apiStabilityAnalysis.recordSignatureStability 741ba09502a5c0a3
    // recordSignatureStability records a public signature whose declaration carries
    // its own `@stability` tag. The raw (uninstantiated) signature keeps distinct
    // overload tags distinct.
    pub(crate) fn record_signature_stability(
        &self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        signature: Option<P<Signature>>,
    ) {
        let Some(signature) = signature else {
            return;
        };
        let declared = tp.declared_api_stability_of_signature(signature);
        if declared.level <= ApiStabilityLevel::Stable {
            return;
        }
        self.add_finding(
            surface,
            ApiStabilityFindingKey {
                signature: raw_signature(Some(signature)),
                ..Default::default()
            },
            declared,
        );
    }

    // go: internal/typeparser/api_stability.go:4571 apiStabilityAnalysis.typeChildBoundarySymbol 099fb86fbd67d5a4
    // typeChildBoundarySymbol returns the identity symbol whose explicit declared
    // tag bounds a type reached as a child component, or nil when the type carries
    // no explicit tag. An alias application's alias symbol takes precedence over
    // the represented target, so an explicit tag on the alias is the component's
    // own tag; otherwise the type's own symbol, or a reference's target symbol,
    // carries the tag. An absent tag (Declaration nil) is not a boundary.
    #[expect(clippy::unused_self, reason = "a method of Go's apiStabilityAnalysis")]
    pub(crate) fn type_child_boundary_symbol(
        &self,
        tp: &mut TypeParser<'_>,
        t: Option<P<Type>>,
    ) -> Option<P<Symbol>> {
        let t = t?;
        if let Some(alias) = t.alias()
            && let Some(alias_symbol) = alias.symbol()
            && tp.declared_api_stability_of_symbol(alias_symbol).declaration.is_some()
        {
            return Some(alias_symbol);
        }
        let mut symbol = t.symbol();
        if symbol.is_none()
            && t.object_flags().intersects(ObjectFlags::Reference)
            && let Some(target) = t.target()
        {
            symbol = target.symbol();
        }
        let symbol = symbol?;
        if tp.declared_api_stability_of_symbol(symbol).declaration.is_some() {
            return Some(symbol);
        }
        None
    }

    // go: internal/typeparser/api_stability.go:4597 apiStabilityAnalysis.heritageChildBoundarySymbol b5d48ee9a5324fdc
    // heritageChildBoundarySymbol returns the identity symbol whose explicit
    // declared tag bounds a heritage base reached as a child component, or nil.
    // As with a type edge, an alias application's alias symbol takes precedence
    // over the represented target.
    #[expect(clippy::unused_self, reason = "a method of Go's apiStabilityAnalysis")]
    pub(crate) fn heritage_child_boundary_symbol(
        &self,
        tp: &mut TypeParser<'_>,
        base: Option<P<Type>>,
        target: Option<P<Type>>,
    ) -> Option<P<Symbol>> {
        if let Some(base) = base
            && let Some(alias) = base.alias()
            && let Some(alias_symbol) = alias.symbol()
            && tp.declared_api_stability_of_symbol(alias_symbol).declaration.is_some()
        {
            return Some(alias_symbol);
        }
        let mut symbol = None;
        if let Some(target) = target {
            symbol = target.symbol();
        }
        if symbol.is_none()
            && let Some(base) = base
        {
            symbol = base.symbol();
        }
        let symbol = symbol?;
        if tp.declared_api_stability_of_symbol(symbol).declaration.is_some() {
            return Some(symbol);
        }
        None
    }

    // go: internal/typeparser/api_stability.go:4625 apiStabilityAnalysis.collectChildTypeArguments d5b46a5fec251ed3
    // collectChildTypeArguments merges the independently represented type arguments
    // a tagged child type still exposes. Alias application arguments and reference
    // arguments are operands of the application, not internals of the tagged
    // component; each argument is itself traversed through the child boundary rules.
    pub(crate) fn collect_child_type_arguments(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        t: Option<P<Type>>,
        subst: &ApiStabilitySubstitution,
    ) {
        let Some(t) = t else {
            return;
        };
        if let Some(alias) = t.alias()
            && api_stability_represented_surface_accepts_arguments(Some(t))
            && !self.alias_arguments_are_self(tp, Some(alias))
        {
            for &argument in alias.type_arguments() {
                let s =
                    self.type_surface(tp, Some(argument), ApiStabilityInspection::Shallow, subst);
                surface.merge(&s);
            }
        }
        if t.object_flags().intersects(ObjectFlags::Reference)
            && let Some(target) = t.target()
            && target != t
        {
            for argument in self.reference_arguments(tp, t, Some(target)) {
                if argument.is_none() {
                    continue;
                }
                let s = self.type_surface(tp, argument, ApiStabilityInspection::Shallow, subst);
                surface.merge(&s);
            }
        }
    }

    // go: internal/typeparser/api_stability.go:4654 apiStabilityAnalysis.collectTaggedHeritageArguments e7470ef9a89b9a58
    // collectTaggedHeritageArguments records every tagged class or interface
    // ancestor of a declaration as a child boundary and exposes its independently
    // represented type arguments. It is used where the compiler has already
    // flattened inherited members into a resolved member table and no per-base
    // declaration walk happens (an expanded reference). Untagged ancestors are
    // followed so a tagged ancestor deeper in the chain is still attributed with
    // the substitution context that reached it; an ancestor cycle is visited once.
    pub(crate) fn collect_tagged_heritage_arguments(
        &mut self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: Option<P<Type>>,
        subst: &ApiStabilitySubstitution,
        visited: &mut FxHashSet<P<Type>>,
    ) {
        let Some(declaration) = declaration else {
            return;
        };
        if visited.contains(&declaration) {
            return;
        }
        visited.insert(declaration);
        let Some(bases) = get_resolved_base_types_of_type_if_materialized(Some(declaration)) else {
            return;
        };
        for &base in bases {
            let mut target = base;
            if base.object_flags().intersects(ObjectFlags::Reference)
                && let Some(base_target) = base.target()
            {
                target = base_target;
            }
            if !target.object_flags().intersects(ObjectFlags::ClassOrInterface) {
                continue;
            }
            if self.optional_tagged_base(tp, target.symbol()) {
                continue;
            }
            if let Some(boundary) =
                self.heritage_child_boundary_symbol(tp, Some(base), Some(target))
            {
                self.record_symbol(tp, surface, Some(boundary));
                self.collect_child_type_arguments(tp, surface, Some(base), subst);
                continue;
            }
            let parameters = self.reference_type_parameters(Some(target));
            let arguments = self.reference_arguments(tp, base, Some(target));
            let base_subst =
                self.extend_parameters_substitution(subst, Some(base), parameters, arguments);
            self.collect_tagged_heritage_arguments(tp, surface, Some(target), &base_subst, visited);
        }
    }

    // go: internal/typeparser/api_stability.go:4692 apiStabilityAnalysis.declarationDeclaringSymbol f8f87cb50a9ddd2e
    // declarationDeclaringSymbol returns the class or interface symbol whose
    // declaration directly contains the given declaration, or nil for a
    // declaration of a type literal or any other anonymous surface. Only a direct
    // declaration names an owner: a declaration nested inside a type literal stays
    // anonymous even when that literal sits inside a class or interface.
    #[expect(clippy::unused_self, reason = "a method of Go's apiStabilityAnalysis")]
    pub(crate) fn declaration_declaring_symbol(
        &self,
        tp: &mut TypeParser<'_>,
        declaration: Option<P<Node>>,
    ) -> Option<P<Symbol>> {
        let parent = declaration?.parent()?;
        match parent.kind() {
            Kind::ClassDeclaration | Kind::ClassExpression | Kind::InterfaceDeclaration => {
                get_symbol_of_declaration(tp.checker, parent)
            }
            _ => None,
        }
    }

    // go: internal/typeparser/api_stability.go:4708 apiStabilityAnalysis.memberDeclaringSymbol dfe961be34afb211
    // memberDeclaringSymbol returns the class or interface symbol whose declaration
    // directly contains the member, or nil for a member of a type literal or any
    // other anonymous surface. Only a direct member declaration names an owner: a
    // member nested inside a type literal stays anonymous even when that literal
    // sits inside a class or interface.
    pub(crate) fn member_declaring_symbol(
        &self,
        tp: &mut TypeParser<'_>,
        member: Option<P<Symbol>>,
    ) -> Option<P<Symbol>> {
        let member = member?;
        for &declaration in member.declarations() {
            if let Some(declaring) = self.declaration_declaring_symbol(tp, Some(declaration)) {
                return Some(declaring);
            }
        }
        None
    }

    // go: internal/typeparser/api_stability.go:4728 apiStabilityAnalysis.inheritedChildBoundarySymbol 7df55813eff803e7
    // inheritedChildBoundarySymbol returns the class or interface symbol that
    // declares a member when the declaring symbol is not the member table's owner
    // and carries an explicit declared tag. A member declared by the owner is an own
    // member and is never gated; an untagged declaring symbol returns nil. This is
    // how a member the compiler flattened into a derived member table from a tagged
    // base is attributed to that base instead of being expanded as the derived
    // component's own member. A same-named own override keeps the owner as its
    // declaring symbol and stays inspected.
    pub(crate) fn inherited_child_boundary_symbol(
        &self,
        tp: &mut TypeParser<'_>,
        member: Option<P<Symbol>>,
        owner: Option<P<Symbol>>,
    ) -> Option<P<Symbol>> {
        member?;
        owner?;
        let declaring = self.member_declaring_symbol(tp, member)?;
        if Some(declaring) == owner {
            return None;
        }
        if tp.declared_api_stability_of_symbol(declaring).declaration.is_some() {
            return Some(declaring);
        }
        None
    }

    // go: internal/typeparser/api_stability.go:4753 apiStabilityAnalysis.inheritedChildBoundarySymbolForDeclaration c4a051b1da4df56d
    // inheritedChildBoundarySymbolForDeclaration returns the class or interface
    // symbol that directly declares a signature, constructor or index declaration
    // when that declaring symbol is not the member table's owner and carries an
    // explicit declared tag. The compiler flattens inherited call/construct
    // signatures and index infos into a derived structured type while keeping each
    // declaration's base provenance, so this attributes them to the tagged base
    // exactly like inheritedChildBoundarySymbol does for flattened members. A
    // declaration that belongs to the owner (an own call signature, own
    // constructor, own index signature) is never gated, and a declaration without
    // a class or interface parent (a type literal or a synthesized signature that
    // has no declaration) returns nil without resolving anything.
    pub(crate) fn inherited_child_boundary_symbol_for_declaration(
        &self,
        tp: &mut TypeParser<'_>,
        declaration: Option<P<Node>>,
        owner: Option<P<Symbol>>,
    ) -> Option<P<Symbol>> {
        let declaring = self.declaration_declaring_symbol(tp, declaration)?;
        if Some(declaring) == owner {
            return None;
        }
        if tp.declared_api_stability_of_symbol(declaring).declaration.is_some() {
            return Some(declaring);
        }
        None
    }

    // go: internal/typeparser/api_stability.go:4775 apiStabilityAnalysis.childSignatureBoundary 53d061d68bc6581a
    // childSignatureBoundary records an explicitly tagged signature reached as a
    // child component of owner and reports whether the signature is a boundary.
    // A signature whose declaration belongs to the owner symbol is that owner's own
    // signature (a root function's overload set) and stays walked; every other
    // tagged signature contributes its declared level and its parameter, return and
    // type-argument internals are not expanded. A signature the compiler flattened
    // into a derived structured type keeps its declaration's base provenance, so a
    // signature declared by a different, explicitly tagged class or interface is
    // attributed to that tagged base (its declared level contributes and its
    // internals are bounded). A synthesized default constructor signature without a
    // declaration has no internals to expand and is never gated.
    pub(crate) fn child_signature_boundary(
        &self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        signature: Option<P<Signature>>,
        owner: Option<P<Symbol>>,
    ) -> bool {
        let Some(raw) = raw_signature(signature) else {
            return false;
        };
        if let Some(declaration) = tp.declared_api_stability_of_signature(raw).declaration {
            if owner.is_some()
                && api_stability_declaration_belongs_to_symbol(Some(declaration), owner)
            {
                return false;
            }
            self.record_signature_stability(tp, surface, Some(raw));
            return true;
        }
        // A signature that is a declaration of the owner symbol itself (a method's
        // raw signature reached through its own function type) is the owner's own
        // exposed content, never a flattened inherited signature.
        if raw.declaration().is_some()
            && owner.is_some()
            && api_stability_declaration_belongs_to_symbol(raw.declaration(), owner)
        {
            return false;
        }
        if let Some(boundary) =
            self.inherited_child_boundary_symbol_for_declaration(tp, raw.declaration(), owner)
        {
            self.record_symbol(tp, surface, Some(boundary));
            return true;
        }
        false
    }

    // go: internal/typeparser/api_stability.go:4810 apiStabilityAnalysis.declarationChildBoundary c89c9e9ceae13796
    // declarationChildBoundary records an explicitly tagged declaration reached as
    // a child component and reports whether the declaration's internals are bounded.
    // It is used for index signatures, whose content annotations are the
    // component's internals. A declaration whose own `@stability` tag is explicit
    // contributes that tag. A declaration the compiler flattened into a derived
    // structured type is attributed to its declaring class or interface exactly
    // like a flattened signature: an explicitly tagged declaring symbol that is not
    // the owner contributes its declared level and bounds the declaration's
    // internals, so a tagged base's inherited index signature stays hidden while an
    // own index signature (declaring symbol == owner) keeps being walked.
    pub(crate) fn declaration_child_boundary(
        &self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: Option<P<Node>>,
        owner: Option<P<Symbol>>,
    ) -> bool {
        let Some(declaration) = declaration else {
            return false;
        };
        if tp.declared_api_stability_of_declaration(declaration).declaration.is_some() {
            self.record_declaration_stability(tp, surface, Some(declaration));
            return true;
        }
        if let Some(boundary) =
            self.inherited_child_boundary_symbol_for_declaration(tp, Some(declaration), owner)
        {
            self.record_symbol(tp, surface, Some(boundary));
            return true;
        }
        false
    }
}

// go: internal/typeparser/api_stability.go:4829 apiStabilityDeclarationBelongsToSymbol c1c481f990ac7112
// apiStabilityDeclarationBelongsToSymbol reports whether a declaration is one
// of a symbol's own declarations. It distinguishes a root function's overload
// signatures (declarations of the root symbol) from child signatures declared
// in another component.
pub(crate) fn api_stability_declaration_belongs_to_symbol(
    declaration: Option<P<Node>>,
    symbol: Option<P<Symbol>>,
) -> bool {
    let (Some(declaration), Some(symbol)) = (declaration, symbol) else {
        return false;
    };
    symbol.declarations().contains(&declaration)
}

impl ApiStabilityAnalysis {
    // go: internal/typeparser/api_stability.go:4841 apiStabilityAnalysis.recordDeclarationStability f419740ce3d88f9b
    // recordDeclarationStability records a `@stability` tag carried by a
    // declaration whose checker type has not been materialized. Declared member and
    // signature stability contributes to the surface independently of type
    // availability, so an annotation the checker has not resolved still reports its
    // tagged members.
    pub(crate) fn record_declaration_stability(
        &self,
        tp: &mut TypeParser<'_>,
        surface: &mut ApiStabilitySurface,
        declaration: Option<P<Node>>,
    ) {
        let Some(declaration) = declaration else {
            return;
        };
        let declared = tp.declared_api_stability_of_declaration(declaration);
        if declared.level <= ApiStabilityLevel::Stable {
            return;
        }
        self.add_finding(
            surface,
            ApiStabilityFindingKey { declaration: Some(declaration), ..Default::default() },
            declared,
        );
    }

    // go: internal/typeparser/api_stability.go:4852 apiStabilityAnalysis.addFinding 2cf948d3d07115dc
    #[expect(clippy::unused_self, reason = "a method of Go's apiStabilityAnalysis")]
    pub(crate) fn add_finding(
        &self,
        surface: &mut ApiStabilitySurface,
        key: ApiStabilityFindingKey,
        declared: ApiStabilityDeclaration,
    ) {
        surface.add(key, ApiStabilityFinding::from(declared));
    }

    // go: internal/typeparser/api_stability.go:4861 apiStabilityAnalysis.declarationRepresentedType 3c586d4f565e6efc
    // declarationRepresentedType returns the represented type of a symbol's own
    // declaration for annotation provenance. Type aliases use their declared type;
    // values use the symbol's value type when it exists, otherwise the
    // declaration's own annotation. An already computed component is preferred and
    // a missing one is materialized lazily.
    pub(crate) fn declaration_represented_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: P<Symbol>,
        declaration: P<Node>,
    ) -> Option<P<Type>> {
        let empty = ApiStabilitySubstitution::default();
        if matches!(declaration.kind(), Kind::TypeAliasDeclaration | Kind::JSTypeAliasDeclaration) {
            if let Some(declared) = self.declared_type_of_symbol(tp, Some(symbol)) {
                return Some(declared);
            }
            return self.represented_type_from_node(tp, declaration.type_node(), &empty);
        }
        if let Some(materialized) =
            get_resolved_type_of_symbol_if_materialized(tp.checker, Some(symbol))
        {
            return Some(materialized);
        }
        match declaration.kind() {
            Kind::VariableDeclaration => self.represented_type_from_node(
                tp,
                declaration.as_variable_declaration().type_(),
                &empty,
            ),
            Kind::PropertyDeclaration => self.represented_type_from_node(
                tp,
                declaration.as_property_declaration().type_(),
                &empty,
            ),
            Kind::PropertySignature => self.represented_type_from_node(
                tp,
                declaration.as_property_signature_declaration().type_(),
                &empty,
            ),
            _ => None,
        }
    }

    // go: internal/typeparser/api_stability.go:4886 apiStabilityAnalysis.declarationAnnotationNode 75a380c9f84bf1f9
    // declarationAnnotationNode returns the public type annotation a declaration
    // exposes for metadata reading, or nil.
    #[expect(clippy::unused_self, reason = "a method of Go's apiStabilityAnalysis")]
    pub(crate) fn declaration_annotation_node(
        &self,
        declaration: Option<P<Node>>,
    ) -> Option<P<Node>> {
        let declaration = declaration?;
        if let Some(function_like) = declaration.function_like_data() {
            return function_like.type_();
        }
        match declaration.kind() {
            Kind::VariableDeclaration => declaration.as_variable_declaration().type_(),
            Kind::PropertyDeclaration => declaration.as_property_declaration().type_(),
            Kind::PropertySignature => declaration.as_property_signature_declaration().type_(),
            Kind::TypeAliasDeclaration | Kind::JSTypeAliasDeclaration => declaration.type_node(),
            _ => None,
        }
    }

    // go: internal/typeparser/api_stability.go:4911 apiStabilityAnalysis.conditionalDeclarationIsDeferred bc6fbe7b46e861a8
    // conditionalDeclarationIsDeferred reports whether a conditional written at a
    // declaration site is symbolic: one of its operands mentions an unbound type
    // parameter or an infer binder. The check is a narrow binder scan; it resolves
    // nothing. A fully concrete conditional has a compiler outcome that the
    // declaration does not represent and is never inspected through its branches.
    pub(crate) fn conditional_declaration_is_deferred(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: P<Node>,
    ) -> bool {
        let conditional = node.as_conditional_type_node();
        let mut visiting = FxHashSet::default();
        self.type_node_mentions_type_parameter(tp, Some(conditional.check_type()), &mut visiting)
            || self.type_node_mentions_type_parameter(
                tp,
                Some(conditional.extends_type()),
                &mut visiting,
            )
    }

    // go: internal/typeparser/api_stability.go:4924 apiStabilityAnalysis.typeNodeMentionsTypeParameter 2f1f647bed171444
    // typeNodeMentionsTypeParameter reports whether a type node names an unbound
    // type parameter (including an `infer` binder). It only looks up annotation
    // symbols; no annotation type is resolved.
    pub(crate) fn type_node_mentions_type_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Option<P<Node>>,
        visiting: &mut FxHashSet<P<Node>>,
    ) -> bool {
        let Some(node) = node else {
            return false;
        };
        if visiting.contains(&node) {
            return false;
        }
        visiting.insert(node);
        let result = self.type_node_mentions_type_parameter_inner(tp, node, visiting);
        // Rust: Go's `defer delete(visiting, node)`.
        visiting.remove(&node);
        result
    }

    // go: internal/typeparser/api_stability.go:4924 apiStabilityAnalysis.typeNodeMentionsTypeParameter 2f1f647bed171444
    // Rust: the body after the cycle guard, split out so the deferred release runs after it.
    fn type_node_mentions_type_parameter_inner(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: P<Node>,
        visiting: &mut FxHashSet<P<Node>>,
    ) -> bool {
        match node.kind() {
            Kind::InferType => true,
            Kind::TypeReference => {
                let reference = node.as_type_reference_node();
                if let Some(symbol) = self.symbol_at_type_name_node(tp, Some(reference.type_name()))
                    && symbol.flags().intersects(SymbolFlags::TypeParameter)
                {
                    return true;
                }
                if let Some(arguments) = reference.type_arguments() {
                    for &argument in arguments.nodes() {
                        if self.type_node_mentions_type_parameter(tp, Some(argument), visiting) {
                            return true;
                        }
                    }
                }
                false
            }
            Kind::ArrayType => self.type_node_mentions_type_parameter(
                tp,
                Some(node.as_array_type_node().element_type()),
                visiting,
            ),
            Kind::TupleType => {
                for &element in node.as_tuple_type_node().elements().nodes() {
                    if self.type_node_mentions_type_parameter(tp, Some(element), visiting) {
                        return true;
                    }
                }
                false
            }
            Kind::NamedTupleMember => self.type_node_mentions_type_parameter(
                tp,
                Some(node.as_named_tuple_member().type_()),
                visiting,
            ),
            Kind::OptionalType => self.type_node_mentions_type_parameter(
                tp,
                Some(node.as_optional_type_node().type_()),
                visiting,
            ),
            Kind::RestType => self.type_node_mentions_type_parameter(
                tp,
                Some(node.as_rest_type_node().type_()),
                visiting,
            ),
            Kind::ParenthesizedType => self.type_node_mentions_type_parameter(
                tp,
                Some(node.as_parenthesized_type_node().type_()),
                visiting,
            ),
            Kind::TypeOperator => self.type_node_mentions_type_parameter(
                tp,
                Some(node.as_type_operator_node().type_()),
                visiting,
            ),
            Kind::IndexedAccessType => {
                let indexed = node.as_indexed_access_type_node();
                self.type_node_mentions_type_parameter(tp, Some(indexed.object_type()), visiting)
                    || self.type_node_mentions_type_parameter(
                        tp,
                        Some(indexed.index_type()),
                        visiting,
                    )
            }
            Kind::UnionType => {
                for &member in node.as_union_type_node().types().nodes() {
                    if self.type_node_mentions_type_parameter(tp, Some(member), visiting) {
                        return true;
                    }
                }
                false
            }
            Kind::IntersectionType => {
                for &member in node.as_intersection_type_node().types().nodes() {
                    if self.type_node_mentions_type_parameter(tp, Some(member), visiting) {
                        return true;
                    }
                }
                false
            }
            Kind::TemplateLiteralType => {
                for &span in node.as_template_literal_type_node().template_spans().nodes() {
                    if span.kind() != Kind::TemplateLiteralTypeSpan {
                        continue;
                    }
                    if self.type_node_mentions_type_parameter(
                        tp,
                        Some(span.as_template_literal_type_span().type_()),
                        visiting,
                    ) {
                        return true;
                    }
                }
                false
            }
            Kind::ConditionalType => {
                let conditional = node.as_conditional_type_node();
                self.type_node_mentions_type_parameter(tp, Some(conditional.check_type()), visiting)
                    || self.type_node_mentions_type_parameter(
                        tp,
                        Some(conditional.extends_type()),
                        visiting,
                    )
            }
            _ => false,
        }
    }

    // go: internal/typeparser/api_stability.go:5007 apiStabilityAnalysis.shouldInspectSymbol 5518f25656de8d27
    // shouldInspectSymbol reports whether a symbol has at least one declaration
    // outside the default library. The standard library is never stability-tagged
    // and expanding it would pull a large, irrelevant surface into every reference.
    #[expect(clippy::unused_self, reason = "a method of Go's apiStabilityAnalysis")]
    pub(crate) fn should_inspect_symbol(
        &self,
        tp: &TypeParser<'_>,
        symbol: Option<P<Symbol>>,
    ) -> bool {
        let Some(symbol) = symbol else {
            return false;
        };
        if symbol.declarations().is_empty() {
            return false;
        }
        for &declaration in symbol.declarations() {
            match ets_shim::ast::get_source_file_of_node(declaration) {
                None => return true,
                Some(source_file) => {
                    if !tp.program.is_source_file_default_library(source_file.path()) {
                        return true;
                    }
                }
            }
        }
        false
    }

    // go: internal/typeparser/api_stability.go:5024 apiStabilityAnalysis.shouldInspectMemberDeclaration 08a5338397a2f9ce
    // shouldInspectMemberDeclaration reports whether a member declaration's public
    // annotation should be inspected for erased alias provenance. Function-like
    // declarations are covered by their public signatures, except accessors whose
    // value annotation is the only place an erased alias survives.
    #[expect(clippy::unused_self, reason = "a method of Go's apiStabilityAnalysis")]
    pub(crate) fn should_inspect_member_declaration(&self, declaration: Option<P<Node>>) -> bool {
        let Some(declaration) = declaration else {
            return false;
        };
        if declaration.function_like_data().is_none() {
            return true;
        }
        declaration.kind() == Kind::GetAccessor || declaration.kind() == Kind::SetAccessor
    }
}

// go: internal/typeparser/api_stability.go:5036 typeParameterAnnotationNode 54769fd1a0bd0421
// typeParameterAnnotationNode returns the declared constraint or default
// annotation of a type parameter.
pub(crate) fn type_parameter_annotation_node(
    t: Option<P<Type>>,
    constraint: bool,
) -> Option<P<Node>> {
    let symbol = t?.symbol()?;
    for &declaration in symbol.declarations() {
        if declaration.kind() != Kind::TypeParameter {
            continue;
        }
        let type_parameter_declaration = declaration.as_type_parameter_declaration();
        if constraint {
            return type_parameter_declaration.constraint();
        }
        return type_parameter_declaration.default_type();
    }
    None
}

// go: internal/typeparser/api_stability.go:5062 signatureReturnTypeNode a066dbaee803b1ef
// signatureReturnTypeNode returns the declared return type annotation of a
// signature, or nil when the return type is inferred.
pub(crate) fn signature_return_type_node(signature: Option<P<Signature>>) -> Option<P<Node>> {
    let declaration = signature?.declaration()?;
    declaration.function_like_data()?.type_()
}

// go: internal/typeparser/api_stability.go:5079 apiStabilityIndexSignatureKeyNode c82c15e0eb93ea7d
// apiStabilityIndexSignatureKeyNode returns the key type node of an index
// signature declaration.
pub(crate) fn api_stability_index_signature_key_node(
    declaration: Option<P<Node>>,
) -> Option<P<Node>> {
    let declaration = declaration?;
    if declaration.kind() != Kind::IndexSignature {
        return None;
    }
    let parameters = declaration.parameters();
    if parameters.len() != 1 || parameters[0].kind() != Kind::Parameter {
        return None;
    }
    parameters[0].as_parameter_declaration().type_()
}

// go: internal/typeparser/api_stability.go:5092 apiStabilityIndexSignatureValueNode 78373ec8871340ad
// apiStabilityIndexSignatureValueNode returns the value type node of an index
// signature declaration.
pub(crate) fn api_stability_index_signature_value_node(
    declaration: Option<P<Node>>,
) -> Option<P<Node>> {
    let declaration = declaration?;
    if declaration.kind() != Kind::IndexSignature {
        return None;
    }
    declaration.as_index_signature_declaration().type_()
}

// go: internal/typeparser/api_stability.go:5101 apiStabilityMappedDeclarationHasName 77e7082570b6e19f
// apiStabilityMappedDeclarationHasName reports whether a mapped type declared a
// `as` clause whose name type the checker has not materialized.
pub(crate) fn api_stability_mapped_declaration_has_name(t: Option<P<Type>>) -> bool {
    let Some(symbol) = t.and_then(|t| t.symbol()) else {
        return false;
    };
    for &declaration in symbol.declarations() {
        if declaration.kind() != Kind::MappedType {
            continue;
        }
        if declaration.as_mapped_type_node().name_type().is_some() {
            return true;
        }
    }
    false
}

// go: internal/typeparser/api_stability.go:5122 apiStabilityTypeAliasDeclaration 60da93774f0e22c1
// apiStabilityTypeAliasDeclaration returns the type alias declaration of a
// symbol.
pub(crate) fn api_stability_type_alias_declaration(symbol: Option<P<Symbol>>) -> Option<P<Node>> {
    let symbol = symbol?;
    symbol.declarations().iter().copied().find(|declaration| {
        matches!(declaration.kind(), Kind::TypeAliasDeclaration | Kind::JSTypeAliasDeclaration)
    })
}

// go: internal/typeparser/api_stability.go:5138 apiStabilitySymbolDeclaresHeritage b025b83b39281ab3
// apiStabilitySymbolDeclaresHeritage reports whether a class or interface
// symbol declares heritage clauses. It reads declaration structure only and is
// used to distinguish an unmaterialized heritage surface from one with nothing
// to resolve.
pub(crate) fn api_stability_symbol_declares_heritage(symbol: Option<P<Symbol>>) -> bool {
    let Some(symbol) = symbol else {
        return false;
    };
    for &declaration in symbol.declarations() {
        let clauses = match declaration.kind() {
            Kind::ClassDeclaration | Kind::ClassExpression => {
                declaration.class_like_data().and_then(ClassLikeBase::heritage_clauses)
            }
            Kind::InterfaceDeclaration => declaration.as_interface_declaration().heritage_clauses(),
            _ => None,
        };
        let Some(clauses) = clauses else {
            continue;
        };
        for &clause_node in clauses.nodes() {
            if !clause_node.as_heritage_clause().types().nodes().is_empty() {
                return true;
            }
        }
    }
    false
}

// go: internal/typeparser/api_stability.go:5171 sortedSymbolTableKeys d2a94f2cbc2a9a54
// Rust: Go sorts the keys bytewise (`sort.Strings`). tsrs spells the internal-name prefix `\u{7f}` where Go uses
// byte `0xFE`; both sort after every ASCII character, but Go's sorts after non-ASCII UTF-8 too, so the prefix byte
// is compared as `0xFE` to keep Go's order.
pub(crate) fn sorted_symbol_table_keys(table: Option<P<SymbolTable>>) -> Vec<&'static str> {
    let Some(table) = table else {
        return Vec::new();
    };
    let mut keys = table.keys();
    keys.sort_by(|left, right| {
        let go_bytes = |s: &'static str| s.bytes().map(|b| if b == 0x7f { 0xFE_u8 } else { b });
        go_bytes(left).cmp(go_bytes(right))
    });
    keys
}

/// Rust-only: Go's `table[name]` on a possibly nil `ast.SymbolTable`.
pub(crate) fn symbol_table_get(table: Option<P<SymbolTable>>, name: &str) -> Option<P<Symbol>> {
    let table = table?;
    SymbolTable::get(&table, name)
}

// go: internal/typeparser/api_stability.go:5191 apiStabilityMemberTableNameIsVisible 63c79ce7a570535f
// apiStabilityMemberTableNameIsVisible reports whether a member-table entry is
// a candidate public member. Entries whose names use the checker's documented
// internal symbol-name prefix are reserved implementation spellings (`__call`,
// `__new`, `__index`, `__export`, the binder's `__computed` placeholder, ambient
// module patterns, ...) and are never public members. A late-bound computed
// member is the exception: the checker names a computed declaration by the
// property name its literal or unique-symbol value resolves to, and
// `checker.IsKnownSymbol` identifies exactly those spellings. Its accessibility
// is still decided by its declaration provenance (`apiStabilitySymbolIsNonPublic`)
// at the call site, and an unresolved dynamic name keeps the `__computed`
// placeholder, which stays invisible.
pub(crate) fn api_stability_member_table_name_is_visible(
    name: &str,
    member: Option<P<Symbol>>,
) -> bool {
    let Some(member) = member else {
        return false;
    };
    if !name.starts_with(INTERNAL_SYMBOL_NAME_PREFIX) {
        return true;
    }
    ets_shim::checker::is_known_symbol(member)
}

// go: internal/typeparser/api_stability.go:5205 apiStabilitySymbolIsNonPublic 1657b92f290f588b
// apiStabilitySymbolIsNonPublic classifies a member symbol by its declaration
// accessibility. Private identifiers, private/protected members and members
// whose declarations are all internal are excluded; a leading `__` name is
// not treated as private.
pub(crate) fn api_stability_symbol_is_non_public(symbol: Option<P<Symbol>>) -> bool {
    let Some(symbol) = symbol else {
        return true;
    };
    if ets_shim::checker::is_private_identifier_symbol(Some(symbol))
        || api_stability_symbol_has_only_internal_declarations(Some(symbol))
    {
        return true;
    }
    let flags = ets_shim::checker::get_declaration_modifier_flags_from_symbol(symbol);
    flags.intersects(ModifierFlags::Private | ModifierFlags::Protected)
}

// go: internal/typeparser/api_stability.go:5219 apiStabilitySymbolHasOnlyInternalDeclarations e70f7a4003c0767a
// A public declaration keeps a merged property visible even when another
// declaration marks that property internal. Declaration-less synthetic members
// also stay visible rather than inheriting the visibility of a referenced type.
pub(crate) fn api_stability_symbol_has_only_internal_declarations(
    symbol: Option<P<Symbol>>,
) -> bool {
    let Some(symbol) = symbol else {
        return false;
    };
    if symbol.declarations().is_empty() {
        return false;
    }
    symbol
        .declarations()
        .iter()
        .all(|&declaration| api_stability_property_declaration_is_internal(Some(declaration)))
}

// go: internal/typeparser/api_stability.go:5231 apiStabilityPropertyDeclarationIsInternal b248090991f03443
pub(crate) fn api_stability_property_declaration_is_internal(declaration: Option<P<Node>>) -> bool {
    let Some(declaration) = declaration else {
        return false;
    };
    match declaration.kind() {
        Kind::PropertySignature
        | Kind::PropertyDeclaration
        | Kind::MethodSignature
        | Kind::MethodDeclaration
        | Kind::GetAccessor
        | Kind::SetAccessor
        | Kind::Parameter => internal_tag_of_declaration(declaration),
        _ => false,
    }
}

// go: internal/typeparser/api_stability.go:5245 apiStabilitySignatureIsNonPublic b01ebe12deb1a6ce
// apiStabilitySignatureIsNonPublic filters construct and method signatures whose
// declaration is private or protected.
pub(crate) fn api_stability_signature_is_non_public(signature: P<Signature>) -> bool {
    let Some(declaration) = signature.declaration() else {
        return false;
    };
    if let Some(name) = ets_shim::ast::get_name_of_declaration(declaration)
        && ets_shim::ast::is_private_identifier(name)
    {
        return true;
    }
    let flags = ets_shim::ast::get_combined_modifier_flags(declaration);
    flags.intersects(ModifierFlags::Private | ModifierFlags::Protected)
}
