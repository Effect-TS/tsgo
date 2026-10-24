use std::sync::Arc;

use ets_etscore::{ResolvedEffectPluginOptions, TopLevelNamedReexportsMode};
use ets_importstyle::{BarrelReader, Policy, ProgramFiles, Style};
use ets_shim::ast::SymbolFlags;
use ets_shim::core::stringutil::{equal_fold, go_strings_to_lower};
use ets_shim::ls::autoimport::{Export, ExportSyntax, Fix, FixTransformer, ModuleID};
use ets_shim::lsproto;
use ets_shim::modulespecifiers::ResultKind;
use parking_lot::Mutex;
use rustc_hash::FxHashMap;

/// Rust-only: the Go provider's `resolveTarget` callback type.
pub type ResolveTarget = Box<dyn Fn(&Export) -> (String, ResultKind) + Send + Sync>;

/// Rust-only: the shim's `autoimport.ModuleIDString`. tsrs's `ModuleID` is the module id string itself (as in
/// the typescript-go provider's compatibility shim).
pub(crate) fn module_id_string(id: &ModuleID) -> &str {
    &id.0
}

/// Rust-only: the shim's `autoimport.ModuleIDFileName` (the typescript-go provider's: the id string itself).
pub(crate) fn module_id_file_name(id: &ModuleID) -> String {
    id.0.clone()
}

// go: internal/autoimportstyle/stylepolicy.go:15 stylePolicy
// stylePolicy applies auto-import style rewrites based on configured preferences.
//
// Rust: `BarrelReader.Find` fills a cache, and the policy lives in a shared `Fn` transformer, so the reader sits
// behind a lock (Go: "not safe for concurrent use; keep one per checker or per request").
#[derive(Default)]
pub(crate) struct StylePolicy {
    pub(crate) styles: Policy,
    pub(crate) aliases: FxHashMap<String, String>,
    pub(crate) follow_reexports: bool,
    pub(crate) barrels: Option<Mutex<BarrelReader>>,
    pub(crate) resolve_target: Option<ResolveTarget>,
}

// go: internal/autoimportstyle/stylepolicy.go:25 NewFixTransformer
// NewFixTransformer creates a FixTransformer from the given resolved Effect options.
// Returns nil if the preferences are empty (no packages configured).
pub fn new_fix_transformer(
    resolved: Option<&ResolvedEffectPluginOptions>,
    program: Option<&'static dyn ProgramFiles>,
    resolve_target: Option<ResolveTarget>,
) -> Option<FixTransformer> {
    let mut sp = new_style_policy(resolved);
    if let Some(program) = program {
        sp.barrels = Some(Mutex::new(ets_importstyle::new_program_barrel_reader(program)));
    }
    sp.resolve_target = resolve_target;
    if StylePolicy::is_empty(Some(&sp)) {
        return None;
    }
    Some(Box::new(move |export: Option<&Export>, fixes: Vec<Arc<Fix>>| -> Vec<Arc<Fix>> {
        let mut rewritten: Vec<Arc<Fix>> = Vec::with_capacity(fixes.len());
        let mut has_use_namespace: FxHashMap<String, bool> = FxHashMap::default();
        for fix in fixes {
            let adjusted = sp.apply(export, Some(fix));
            if let Some(adjusted) = adjusted {
                if adjusted.kind == lsproto::AutoImportFixKind::UseNamespace {
                    has_use_namespace.insert(namespace_fix_key(Some(&adjusted)), true);
                }
                rewritten.push(adjusted);
            }
        }
        if !has_use_namespace.is_empty() {
            let mut filtered = Vec::with_capacity(rewritten.len());
            for fix in rewritten {
                if fix.kind == lsproto::AutoImportFixKind::AddNew
                    && fix.import_kind == lsproto::ImportKind::Namespace
                    && has_use_namespace
                        .get(&namespace_fix_key(Some(&fix)))
                        .copied()
                        .unwrap_or_default()
                {
                    continue;
                }
                filtered.push(fix);
            }
            rewritten = filtered;
        }
        if rewritten.is_empty() {
            return Vec::new();
        }
        rewritten
    }))
}

// go: internal/autoimportstyle/stylepolicy.go:67 namespaceFixKey
pub(crate) fn namespace_fix_key(fix: Option<&Fix>) -> String {
    let Some(fix) = fix else {
        return String::new();
    };
    format!("{}\x00{}", fix.module_specifier, fix.namespace_prefix)
}

// go: internal/autoimportstyle/stylepolicy.go:76 newStylePolicy
// newStylePolicy creates a stylePolicy from the given resolved options.
// Alias keys are lowercased for case-insensitive matching.
pub(crate) fn new_style_policy(resolved: Option<&ResolvedEffectPluginOptions>) -> StylePolicy {
    let Some(resolved) = resolved else {
        return StylePolicy::default();
    };
    let mut sp = StylePolicy {
        styles: ets_importstyle::new_policy(Some(resolved)),
        aliases: FxHashMap::with_capacity_and_hasher(
            resolved.import_aliases.len(),
            rustc_hash::FxBuildHasher,
        ),
        follow_reexports: resolved.top_level_named_reexports == TopLevelNamedReexportsMode::Follow,
        ..Default::default()
    };
    for (pkg, alias) in &resolved.import_aliases {
        sp.aliases.insert(go_strings_to_lower(pkg), alias.clone());
    }
    sp
}

impl StylePolicy {
    // go: internal/autoimportstyle/stylepolicy.go:92 stylePolicy.isEmpty
    // isEmpty returns true if no style preferences are configured.
    pub(crate) fn is_empty(sp: Option<&Self>) -> bool {
        sp.is_none_or(|sp| sp.styles.is_empty())
    }

    // go: internal/autoimportstyle/stylepolicy.go:99 stylePolicy.Apply
    // Apply transforms a fix based on the export's package and the configured style.
    // Returns nil if the fix should be suppressed (e.g., ignored top-level reexport).
    // Returns the fix unchanged if no style policy applies.
    pub(crate) fn apply(&self, export: Option<&Export>, fix: Option<Arc<Fix>>) -> Option<Arc<Fix>> {
        let (Some(export), Some(fix)) = (export, fix.as_ref()) else {
            return fix;
        };
        if Self::is_empty(Some(self)) {
            return Some(Arc::clone(fix));
        }

        // Only rewrite new imports or additions to existing imports.
        if fix.kind != lsproto::AutoImportFixKind::AddNew
            && fix.kind != lsproto::AutoImportFixKind::AddToExisting
        {
            return Some(Arc::clone(fix));
        }

        let style = self.styles.package_style(&export.package_name);
        if style == Style::None {
            return Some(Arc::clone(fix));
        }
        if equal_fold(&export.package_name, "effect") && is_effect_internal(&fix.module_specifier) {
            return None;
        }
        if style == Style::Namespace {
            return self.apply_namespace_rewrite(export, fix);
        }
        Some(self.apply_barrel_rewrite(export, fix))
    }

    // go: internal/autoimportstyle/stylepolicy.go:123 stylePolicy.applyNamespaceRewrite
    // applyNamespaceRewrite rewrites a named-import fix to a namespace-import fix.
    pub(crate) fn apply_namespace_rewrite(
        &self,
        export: &Export,
        fix: &Arc<Fix>,
    ) -> Option<Arc<Fix>> {
        // Only rewrite named imports
        if fix.import_kind != lsproto::ImportKind::Named {
            return Some(Arc::clone(fix));
        }
        // Rewrites require usage qualification (e.g. `succeed` -> `Effect.succeed`).
        // If no usage site is available, keep the original named-import fix.
        if fix.usage_position.is_none() {
            return Some(Arc::clone(fix));
        }
        if is_namespace_reexport(Some(export)) {
            let Some(resolve_target) = &self.resolve_target else {
                return Some(Arc::clone(fix));
            };
            let (module_specifier, module_specifier_kind) = resolve_target(export);
            if module_specifier.is_empty() {
                return Some(Arc::clone(fix));
            }
            return Some(Arc::new(Fix {
                auto_import_fix: lsproto::AutoImportFix {
                    kind: lsproto::AutoImportFixKind::AddNew,
                    import_kind: lsproto::ImportKind::Namespace,
                    module_specifier,
                    name: export.name().to_string(),
                    use_require: fix.use_require,
                    add_as_type_only: fix.add_as_type_only,
                    usage_position: fix.usage_position,
                    ..Default::default()
                },
                module_specifier_kind,
                module_file_name: module_id_file_name(&export.target.module_id),
                ..Default::default()
            }));
        }

        // Check if this is a top-level named reexport
        let is_reexport = !module_id_string(&export.target.module_id).is_empty()
            && export.target.module_id != export.module_id;
        if is_reexport && !self.follow_reexports {
            // When topLevelNamedReexports is "ignore", skip rewriting top-level reexports
            return Some(Arc::clone(fix));
        }
        if is_reexport && self.follow_reexports {
            // When topLevelNamedReexports is "follow", suppress reexport fixes
            // so the direct submodule namespace import wins instead
            return None;
        }

        let namespace_name = ets_importstyle::namespace_name(&fix.module_specifier);
        if namespace_name.is_empty() {
            return Some(Arc::clone(fix));
        }

        // Apply alias if configured
        let pkg_name = go_strings_to_lower(&export.package_name);
        let alias = self.resolve_alias(&pkg_name, namespace_name);

        let result = Fix {
            auto_import_fix: lsproto::AutoImportFix {
                kind: lsproto::AutoImportFixKind::AddNew,
                import_kind: lsproto::ImportKind::Namespace,
                module_specifier: fix.module_specifier.clone(),
                name: fix.name.clone(),
                use_require: fix.use_require,
                add_as_type_only: fix.add_as_type_only,
                usage_position: fix.usage_position,
                namespace_prefix: alias,
                ..Default::default()
            },
            module_specifier_kind: fix.module_specifier_kind,
            is_re_export: fix.is_re_export,
            module_file_name: fix.module_file_name.clone(),
            ..Default::default()
        };
        Some(Arc::new(result))
    }
}

// go: internal/autoimportstyle/stylepolicy.go:195 isNamespaceReexport
pub(crate) fn is_namespace_reexport(export: Option<&Export>) -> bool {
    export.is_some_and(|export| {
        export.syntax == ExportSyntax::Modifier
            && export.flags.intersects(SymbolFlags::Module)
            && !module_id_string(&export.target.module_id).is_empty()
            && export.target.module_id != export.module_id
    })
}

impl StylePolicy {
    // go: internal/autoimportstyle/stylepolicy.go:205 stylePolicy.applyBarrelRewrite
    // applyBarrelRewrite rewrites a fix to a named import from the barrel that
    // forwards the fix's module, keeping the fix unchanged when no barrel is found.
    // Rust: never nil, so the result is not an `Option`.
    pub(crate) fn apply_barrel_rewrite(&self, export: &Export, fix: &Arc<Fix>) -> Arc<Fix> {
        // Barrel rewrites require usage qualification (e.g. `request` -> `HttpClient.request`).
        // If no usage site is available, keep the original named-import fix.
        if fix.usage_position.is_none() {
            return Arc::clone(fix);
        }
        let mut barrels = self.barrels.as_ref().map(Mutex::lock);
        let Some(barrel) = BarrelReader::find(
            barrels.as_deref_mut(),
            &fix.module_specifier,
            &fix.module_file_name,
        ) else {
            return Arc::clone(fix);
        };
        drop(barrels);

        let pkg_name = go_strings_to_lower(&export.package_name);
        let alias = self.resolve_alias(&pkg_name, &barrel.export_name);

        let result = Fix {
            auto_import_fix: lsproto::AutoImportFix {
                kind: lsproto::AutoImportFixKind::AddNew,
                import_kind: lsproto::ImportKind::Named,
                module_specifier: barrel.specifier,
                name: fix.name.clone(),
                use_require: fix.use_require,
                add_as_type_only: fix.add_as_type_only,
                usage_position: fix.usage_position,
                namespace_prefix: alias,
                ..Default::default()
            },
            module_specifier_kind: fix.module_specifier_kind,
            is_re_export: fix.is_re_export,
            module_file_name: fix.module_file_name.clone(),
            ..Default::default()
        };
        Arc::new(result)
    }
}

// go: internal/autoimportstyle/stylepolicy.go:239 isEffectInternal
// isEffectInternal reports whether a specifier points into an internal
// directory of the effect package, which auto-import never suggests.
pub(crate) fn is_effect_internal(specifier: &str) -> bool {
    specifier.starts_with("effect/internal/") || specifier.contains("/internal/")
}

impl StylePolicy {
    // go: internal/autoimportstyle/stylepolicy.go:245 stylePolicy.resolveAlias
    // resolveAlias returns the configured alias for the given namespace name under
    // the given package, or the namespace name itself if no alias is configured.
    pub(crate) fn resolve_alias(&self, pkg_name_lower: &str, namespace_name: &str) -> String {
        if let Some(alias) = self.aliases.get(pkg_name_lower) {
            return alias.clone();
        }
        namespace_name.to_string()
    }
}
