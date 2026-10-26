use std::sync::LazyLock;

use ets_etscore::Severity;
use ets_rule::{Context, Rule};
use ets_shim::P;
use ets_shim::ast::{self, Diagnostic, Kind, Node, SourceFile, Symbol, SymbolFlags};
use ets_shim::checker::Signature;
use ets_shim::diagnostics as tsdiag;
use ets_typeparser::{
    ApiStabilityDependency, ApiStabilityLevel, ApiStabilitySession, api_stability_dependency_name,
    api_stability_immediate_aliased_symbol, api_stability_level_from_tag,
    get_symbol_if_same_reference, get_symbol_of_declaration, internal_tag_of_declaration,
    new_api_stability_dependency, stability_tag_of_declaration,
};
use rustc_hash::FxHashSet;

use crate::*;

// go: internal/rules/api_stability_leak.go:19 ApiStabilityLeak
// ApiStabilityLeak reports exported APIs whose public surface exposes a type
// that is marked with a lower stability than the export itself. A stable export
// may not expose unstable or experimental types, and an unstable export may not
// expose experimental types. Experimental exports impose no restriction.
pub static API_STABILITY_LEAK: LazyLock<Rule> = LazyLock::new(|| Rule {
    name: "apiStabilityLeak",
    group: "maintainers",
    description: "Reports exported APIs whose public surface exposes a less stable type",
    default_severity: Severity::Off,
    supported_effect: vec!["v4"],
    codes: vec![
        tsdiag::X_0_EXPOSES_1_AN_2_API_EFFECT_API_STABILITY_LEAK.code(),
        tsdiag::X_0_IS_DECLARED_1_HERE_EFFECT_API_STABILITY_LEAK.code(),
    ],
    run: check_api_stability_leaks,
});

// go: internal/rules/api_stability_leak.go:40 checkApiStabilityLeaks
// checkApiStabilityLeaks compares each export's declared stability ceiling with
// the actual stability of its public surface. Every export is inspected in the
// file's ApiStabilitySession, reset before each export, so the export's type
// and signatures share one traversal and unpublishable results stay local to
// it. Declared stability lookups persist per checker, and a complete, settled,
// context-free concrete type or signature surface is reused by later exports,
// files and TypeParsers over the same checker. The ceiling and the diagnostic
// location are per export.
fn check_api_stability_leaks(ctx: &mut Context<'_, '_>) -> Vec<P<Diagnostic>> {
    let source_file = ctx.source_file;
    let Some(module_symbol) = get_symbol_of_declaration(ctx.checker(), source_file.as_node())
    else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    let mut session = ctx.type_parser.new_api_stability_session();
    for export_symbol in sorted_api_stability_exports(ctx, module_symbol) {
        if api_stability_export_is_internal(ctx, Some(module_symbol), Some(export_symbol)) {
            continue;
        }
        let ceiling = api_stability_ceiling(ctx, export_symbol);
        if ceiling >= ApiStabilityLevel::Experimental {
            continue;
        }
        let Some(target) = resolve_stability_alias(ctx.checker(), Some(export_symbol)) else {
            continue;
        };
        let Some(location) = api_stability_export_location(ctx, export_symbol) else {
            continue;
        };

        session.reset();
        let mut walker =
            new_api_stability_walker(ctx, &mut session, export_symbol, target, location, ceiling);
        walker.inspect_export();
        diagnostics.append(&mut walker.diagnostics);
    }
    diagnostics
}

// go: internal/rules/api_stability_leak.go:83 sortedApiStabilityExports
// sortedApiStabilityExports returns a module's exports in a deterministic
// order. The checker returns them in Go map order; each export is analyzed
// independently from its own session, so visit order only decides the order in
// which bounded per-export analyses consume their own budgets. Sorting by name
// (the unique export-table key) makes the reported findings reproducible across
// runs and processes. Diagnostics are sorted later, so this only removes
// analysis-order nondeterminism.
//
// Rust: tsrs returns `Vec<P<Symbol>>`, which holds no nil, so Go's nil ordering drops.
pub(crate) fn sorted_api_stability_exports(
    ctx: &mut Context<'_, '_>,
    module: P<Symbol>,
) -> Vec<P<Symbol>> {
    let mut exports = ctx.checker().get_exports_of_module_exported(module);
    exports.sort_by(|left, right| left.name().cmp(right.name()));
    exports
}

// go: internal/rules/api_stability_leak.go:112 apiStabilityCeiling
// apiStabilityCeiling returns the stability ceiling that governs an export. A
// forwarding tag written on a re-export declaration in the selected file takes
// precedence over the forwarded symbol's own stability, including when it is
// stricter; presence is therefore returned separately from the declared level.
// The tag is source-specific, so it is read here rather than through the
// per-checker symbol cache that only sees the forwarded symbol's declarations.
fn api_stability_ceiling(ctx: &mut Context<'_, '_>, export_symbol: P<Symbol>) -> ApiStabilityLevel {
    let mut ceiling = ctx.type_parser.declared_api_stability(Some(export_symbol));
    if let Some(forwarded) = api_stability_forwarding_stability(ctx, Some(export_symbol)) {
        ceiling = forwarded;
    }
    ceiling
}

// go: internal/rules/api_stability_leak.go:128 apiStabilityWalker
// apiStabilityWalker walks one export's public surface through the export's
// analysis-local ApiStabilitySession and reports every dependency whose
// declared stability exceeds the export's ceiling. Namespace re-exports are
// enumerated through their module symbol so type-only exports are not lost. The
// walker holds no type-graph state: cycle safety and memoization belong to the
// session; when the export is inspected, its unpublishable results are
// discarded and its complete, context-free surfaces are reused through the
// per-checker stores.
struct ApiStabilityWalker<'w, 'a, 'c> {
    ctx: &'w mut Context<'a, 'c>,
    session: &'w mut ApiStabilitySession,
    ceiling: ApiStabilityLevel,
    export_symbol: P<Symbol>,
    export_target: P<Symbol>,
    location: P<Node>,

    // inspectedModules breaks namespace re-export cycles by resolved module
    // identity. offenders is keyed by offender declaration, symbol or signature,
    // so the same type is reported once per export regardless of how many paths
    // reach it.
    inspected_modules: FxHashSet<P<Symbol>>,
    offenders: FxHashSet<ApiStabilityOffenderKey>,

    diagnostics: Vec<P<Diagnostic>>,
}

// go: internal/rules/api_stability_leak.go:146 apiStabilityOffenderKey
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ApiStabilityOffenderKey {
    symbol: Option<P<Symbol>>,
    signature: Option<P<Signature>>,
    declaration: Option<P<Node>>,
}

// go: internal/rules/api_stability_leak.go:152 newApiStabilityWalker
fn new_api_stability_walker<'w, 'a, 'c>(
    ctx: &'w mut Context<'a, 'c>,
    session: &'w mut ApiStabilitySession,
    export_symbol: P<Symbol>,
    export_target: P<Symbol>,
    location: P<Node>,
    ceiling: ApiStabilityLevel,
) -> ApiStabilityWalker<'w, 'a, 'c> {
    ApiStabilityWalker {
        ctx,
        session,
        ceiling,
        export_symbol,
        export_target,
        location,
        inspected_modules: FxHashSet::default(),
        offenders: FxHashSet::default(),
        diagnostics: Vec::new(),
    }
}

impl ApiStabilityWalker<'_, '_, '_> {
    // go: internal/rules/api_stability_leak.go:174 apiStabilityWalker.inspectExport
    // inspectExport seeds the walk. Namespace re-exports expose their members'
    // surfaces, so modules are enumerated explicitly.
    //
    // Rust: the export target is never nil (the caller skips a nil one), so Go's nil check drops.
    fn inspect_export(&mut self) {
        self.inspect_symbol(Some(self.export_target));
    }

    // go: internal/rules/api_stability_leak.go:181 apiStabilityWalker.inspectSymbol
    fn inspect_symbol(&mut self, symbol: Option<P<Symbol>>) {
        let Some(symbol) = symbol else {
            return;
        };
        if symbol.flags().intersects(SymbolFlags::Module) {
            self.inspect_module(Some(symbol));
            return;
        }
        let used = self.session.used_by_symbol(self.ctx.type_parser, Some(symbol));
        for (_, &dependency) in used.dependencies().all() {
            self.record_dependency(dependency);
        }
    }

    // go: internal/rules/api_stability_leak.go:198 apiStabilityWalker.inspectModule
    // inspectModule enumerates a namespace module's exports and inspects each
    // member. Modules are guarded by resolved identity so namespace re-export cycles
    // terminate. Type-only exports (interfaces and aliases) are not value properties
    // of the namespace, so they are enumerated through the module symbol.
    fn inspect_module(&mut self, module: Option<P<Symbol>>) {
        let Some(module) = module else {
            return;
        };
        if self.inspected_modules.contains(&module) {
            return;
        }
        self.inspected_modules.insert(module);
        for member in sorted_api_stability_exports(self.ctx, module) {
            // An `@internal` member is not part of the namespace's public API, so
            // neither its declared stability nor its surface is inspected.
            if api_stability_export_is_internal(self.ctx, Some(module), Some(member)) {
                continue;
            }
            // A namespace exposes each member's own declared stability as part of the
            // surface, so a tagged member is reported even when it is not reachable
            // through another member's dependencies.
            self.record_declared(Some(member));
            let resolved = resolve_stability_alias(self.ctx.checker(), Some(member));
            self.inspect_symbol(resolved);
        }
    }

    // go: internal/rules/api_stability_leak.go:223 apiStabilityWalker.recordDeclared
    // recordDeclared reports a namespace member whose own declared stability is
    // above the namespace owner's ceiling. The member's surface is inspected
    // separately for dependencies that themselves exceed the ceiling.
    fn record_declared(&mut self, symbol: Option<P<Symbol>>) {
        let Some(symbol) = symbol else {
            return;
        };
        let declared = self.ctx.type_parser.declared_api_stability_of_symbol(Some(symbol));
        self.report(new_api_stability_dependency(
            Some(symbol),
            None,
            declared.declaration(),
            declared.level(),
        ));
    }

    // go: internal/rules/api_stability_leak.go:235 apiStabilityWalker.recordDependency
    // recordDependency reports a dependency whose declared stability is above the
    // export's ceiling. The owner's ceiling governs the entire surface; a member's
    // own `@stability` tag never loosens it, and no separate stricter-member
    // requirement is imposed.
    fn record_dependency(&mut self, dependency: ApiStabilityDependency) {
        self.report(dependency);
    }

    // go: internal/rules/api_stability_leak.go:239 apiStabilityWalker.report
    fn report(&mut self, dependency: ApiStabilityDependency) {
        if dependency.level() <= self.ceiling {
            return;
        }
        if dependency.symbol().is_none()
            && dependency.signature().is_none()
            && dependency.declaration().is_none()
        {
            return;
        }
        let key = ApiStabilityOffenderKey {
            symbol: dependency.symbol(),
            signature: dependency.signature(),
            declaration: dependency.declaration(),
        };
        if self.offenders.contains(&key) {
            return;
        }
        if self.same_as_export(&dependency) {
            return;
        }
        self.offenders.insert(key);
        let mut related_information = Vec::new();
        if let Some(declaration) = dependency.declaration()
            && let Some(source_file) = ast::get_source_file_of_node(declaration)
        {
            related_information = vec![self.ctx.new_diagnostic(
                source_file,
                ets_shim::scanner::get_error_range_for_node(source_file, declaration),
                &tsdiag::X_0_IS_DECLARED_1_HERE_EFFECT_API_STABILITY_LEAK,
                &[],
                &[
                    &api_stability_dependency_name(&dependency),
                    api_stability_level_name(dependency.level()),
                ],
            )];
        }
        let display_name = api_stability_export_display_name(self.ctx, self.export_symbol);
        let diagnostic = self.ctx.new_diagnostic(
            self.ctx.source_file,
            self.ctx.get_error_range(self.location),
            &tsdiag::X_0_EXPOSES_1_AN_2_API_EFFECT_API_STABILITY_LEAK,
            &related_information,
            &[
                &display_name,
                &api_stability_dependency_name(&dependency),
                api_stability_level_name(dependency.level()),
            ],
        );
        self.diagnostics.push(diagnostic);
    }

    // go: internal/rules/api_stability_leak.go:285 apiStabilityWalker.sameAsExport
    // sameAsExport filters the export's own symbol from its dependency list. A
    // signature or distinct declaration finding is never the export itself, so it
    // is still reported.
    fn same_as_export(&mut self, dependency: &ApiStabilityDependency) -> bool {
        if dependency.symbol().is_none() || dependency.signature().is_some() {
            return false;
        }
        let (export_symbol, export_target) = (self.export_symbol, self.export_target);
        let c = self.ctx.checker();
        get_symbol_if_same_reference(c, dependency.symbol(), Some(export_symbol)).is_some()
            || get_symbol_if_same_reference(c, dependency.symbol(), Some(export_target)).is_some()
    }
}

// go: internal/rules/api_stability_leak.go:294 apiStabilityLevelName
fn api_stability_level_name(level: ApiStabilityLevel) -> &'static str {
    if level == ApiStabilityLevel::Experimental {
        return "experimental";
    }
    "unstable"
}

// go: internal/rules/api_stability_leak.go:301 apiStabilityExportDisplayName
fn api_stability_export_display_name(
    ctx: &mut Context<'_, '_>,
    export_symbol: P<Symbol>,
) -> String {
    let name = export_symbol.name();
    let Some(pkg) = ctx.type_parser.package_json_for_source_file(ctx.source_file) else {
        return name.to_string();
    };
    let Some(package_name) = pkg.name.get_value() else {
        return name.to_string();
    };
    if package_name.is_empty() {
        return name.to_string();
    }
    let directory = get_package_json_directory(ctx.program, ctx.source_file);
    let module_name =
        stability_api_module_name(package_name, &directory, ctx.source_file.file_name());
    if !module_name.is_empty() {
        return format!("{module_name}#{name}");
    }
    name.to_string()
}

// go: internal/rules/api_stability_leak.go:321 apiStabilityExportLocation
// apiStabilityExportLocation returns a node in the selected file that names the
// export, preferring a local declaration and falling back to the `export * from`
// declaration that forwards it.
fn api_stability_export_location(
    ctx: &mut Context<'_, '_>,
    export_symbol: P<Symbol>,
) -> Option<P<Node>> {
    for &declaration in export_symbol.declarations() {
        if ast::get_source_file_of_node(declaration) != Some(ctx.source_file) {
            continue;
        }
        if let Some(name) = ast::get_name_of_declaration(declaration) {
            return Some(name);
        }
        return Some(declaration);
    }
    api_stability_reexport_location(ctx, export_symbol)
}

// go: internal/rules/api_stability_leak.go:339 apiStabilityForwardingStability
// apiStabilityForwardingStability returns the stability declared by a re-export
// declaration in the selected file that forwards this export: a star, a named
// specifier or a namespace export. The boolean reports whether any forwarding
// declaration carries a tag, which lets the caller give an explicit forwarding
// tag precedence even when it tightens the inherited ceiling.
//
// Rust: Go's `(level, present)` is `Option<level>` (`None` when no forwarding declaration carries a tag).
fn api_stability_forwarding_stability(
    ctx: &mut Context<'_, '_>,
    export_symbol: Option<P<Symbol>>,
) -> Option<ApiStabilityLevel> {
    let export_symbol = export_symbol?;
    let mut best = ApiStabilityLevel::Stable;
    let mut present = false;
    for &statement in ctx.source_file.statements().nodes() {
        if statement.kind() != Kind::ExportDeclaration {
            continue;
        }
        let stability = stability_tag_of_declaration(statement);
        if stability.is_empty() {
            continue;
        }
        if !api_stability_export_declaration_forwards(ctx, statement, export_symbol) {
            continue;
        }
        present = true;
        let level = api_stability_level_from_tag(stability);
        if level > best {
            best = level;
        }
    }
    present.then_some(best)
}

// go: internal/rules/api_stability_leak.go:370 apiStabilityExportDeclarationForwards
// apiStabilityExportDeclarationForwards reports whether an export declaration in
// the selected file forwards the given export symbol.
//
// Rust: the declaration is the `ExportDeclaration` node (Go passes its `AsExportDeclaration()`).
fn api_stability_export_declaration_forwards(
    ctx: &mut Context<'_, '_>,
    declaration: P<Node>,
    export_symbol: P<Symbol>,
) -> bool {
    let export_declaration = declaration.as_export_declaration();
    let Some(export_clause) = export_declaration.export_clause() else {
        let Some(module_specifier) = export_declaration.module_specifier() else {
            return false;
        };
        let mut module_symbol = ctx.type_parser.get_symbol_at_location(module_specifier);
        if let Some(symbol) = module_symbol
            && symbol.flags().intersects(SymbolFlags::Alias)
        {
            module_symbol = Some(ctx.checker().get_aliased_symbol(symbol));
        }
        let Some(module_symbol) = module_symbol else {
            return false;
        };
        let exported = ctx
            .checker()
            .try_get_member_in_module_exports_and_properties(export_symbol.name(), module_symbol);
        return exported.is_some()
            && get_symbol_if_same_reference(ctx.checker(), exported, Some(export_symbol))
                .is_some();
    };
    match export_clause.kind() {
        Kind::NamespaceExport => {
            let namespace = export_clause.as_namespace_export();
            return namespace.name().text() == export_symbol.name();
        }
        Kind::NamedExports => {
            let named = export_clause.as_named_exports();
            for &element in named.elements().nodes() {
                if element.kind() != Kind::ExportSpecifier {
                    continue;
                }
                let specifier = element.as_export_specifier();
                if specifier.name().text() == export_symbol.name() {
                    return true;
                }
            }
        }
        _ => {}
    }
    false
}

// go: internal/rules/api_stability_leak.go:410 apiStabilityInternalVisit
// apiStabilityInternalVisit identifies one module-scoped visibility question:
// whether a symbol is internal as exported from a module. Recursion cycles are
// broken by this identity.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ApiStabilityInternalVisit {
    module: P<Symbol>,
    symbol: P<Symbol>,
}

// go: internal/rules/api_stability_leak.go:430 apiStabilityExportIsInternal
// apiStabilityExportIsInternal reports whether an export is marked `@internal`
// and must not be checked. Native TypeScript recognizes the same tag for
// `stripInternal` declaration emit: a tagged declaration is omitted from the
// emitted declarations, so it is not part of the public API.
//
// An export is internal only when every path that exposes it from the module
// is internal: a declaration without the tag is public, a named specifier,
// star or namespace forwarding without the tag is public, and an untagged
// forwarding inherits the visibility of the target it forwards. A merged
// symbol with at least one non-internal declaration therefore stays public, so
// a public overload is still checked when only the implementation is tagged,
// while an untagged re-export of an internal target is skipped because the
// forwarded target is not public API. The check reads metadata only: parsed
// JSDoc tags, declarations and re-export statements, never types, so it never
// touches the dependency caches.
//
// Rust: the context is a reference, so Go's nil-context check drops.
fn api_stability_export_is_internal(
    ctx: &mut Context<'_, '_>,
    module: Option<P<Symbol>>,
    export_symbol: Option<P<Symbol>>,
) -> bool {
    let (Some(module), Some(export_symbol)) = (module, export_symbol) else {
        return false;
    };
    api_stability_symbol_is_internal(
        ctx,
        Some(module),
        Some(export_symbol),
        &mut FxHashSet::default(),
    )
}

// go: internal/rules/api_stability_leak.go:452 apiStabilitySymbolIsInternal
// apiStabilitySymbolIsInternal reports whether a symbol is internal as exported
// from one module. Declarations of the symbol that live in the module are its
// paths; a star re-export is inspected separately because it is not a
// declaration of the forwarded symbol. A path that revisits an in-progress
// visibility question reports no new public declaration: public declarations
// are always collected when their module is entered, before that module's
// forwards are followed, so a cycle can only be reached after every reachable
// declaration path has already decided its visibility.
//
// When the module's export table selects the symbol for its own name and the
// symbol has a declaration in the module, that explicit declaration is the
// compiler's export winner: a redundant `export *` forward of the same name
// must not defeat a tag written on the explicit declaration. Star paths are
// still followed when the symbol is only reachable through them, or when the
// selected export resolves to a different symbol.
//
// Rust: Go's `defer delete(visiting, visit)` is the removal before each return after the insert.
fn api_stability_symbol_is_internal(
    ctx: &mut Context<'_, '_>,
    module: Option<P<Symbol>>,
    symbol: Option<P<Symbol>>,
    visiting: &mut FxHashSet<ApiStabilityInternalVisit>,
) -> bool {
    let (Some(module), Some(symbol)) = (module, symbol) else {
        return false;
    };
    let visit = ApiStabilityInternalVisit { module, symbol };
    if visiting.contains(&visit) {
        return true;
    }
    visiting.insert(visit);
    let result = api_stability_symbol_is_internal_visit(ctx, module, symbol, visiting);
    visiting.remove(&visit);
    result
}

/// Rust-only: the body of `apiStabilitySymbolIsInternal` after `visit` is marked (see there).
fn api_stability_symbol_is_internal_visit(
    ctx: &mut Context<'_, '_>,
    module: P<Symbol>,
    symbol: P<Symbol>,
    visiting: &mut FxHashSet<ApiStabilityInternalVisit>,
) -> bool {
    let files = api_stability_module_source_files(Some(module));
    if files.is_empty() {
        return false;
    }
    let mut found = false;
    for &declaration in symbol.declarations() {
        if !api_stability_declaration_in_files(declaration, &files) {
            continue;
        }
        found = true;
        if !api_stability_declaration_path_is_internal(ctx, module, symbol, declaration, visiting) {
            return false;
        }
    }
    if found && api_stability_symbol_is_module_export(ctx, Some(module), Some(symbol)) {
        return true;
    }
    for statement in api_stability_module_export_statements(Some(module)) {
        if statement.kind() != Kind::ExportDeclaration {
            continue;
        }
        let declaration = statement.as_export_declaration();
        // Named and namespace forwards are declarations of the forwarded
        // symbol; only a star forward is not, so it is inspected here.
        if declaration.export_clause().is_some() {
            continue;
        }
        if !api_stability_export_declaration_forwards(ctx, statement, symbol) {
            continue;
        }
        found = true;
        if !api_stability_star_path_is_internal(ctx, symbol, statement, visiting) {
            return false;
        }
    }
    found
}

// go: internal/rules/api_stability_leak.go:505 apiStabilitySymbolIsModuleExport
// apiStabilitySymbolIsModuleExport reports whether a module's own export table
// selects exactly this symbol for its name. It is the metadata-only identity
// test that lets an explicit export declaration take precedence over redundant
// star forwards; it resolves aliases but never reads a type.
fn api_stability_symbol_is_module_export(
    ctx: &mut Context<'_, '_>,
    module: Option<P<Symbol>>,
    symbol: Option<P<Symbol>>,
) -> bool {
    let (Some(module), Some(symbol)) = (module, symbol) else {
        return false;
    };
    let exported =
        ctx.checker().try_get_member_in_module_exports_and_properties(symbol.name(), module);
    get_symbol_if_same_reference(ctx.checker(), exported, Some(symbol)).is_some()
}

// go: internal/rules/api_stability_leak.go:522 apiStabilityDeclarationPathIsInternal
// apiStabilityDeclarationPathIsInternal reports whether one declaration of an
// exported symbol is an internal path. A tag on the declaration or, for a named
// or namespace forward, on its enclosing export declaration marks the path
// internal; an untagged named forward stays internal only when the target it
// forwards is itself internal. An import alias follows the imported symbol to
// the module it was exported from, so `export { X }` after `import { X }`
// inherits the original declaration's tag; an `export import` alias follows the
// target of its module reference the same way. A namespace forward without a tag
// is public: its container is public, and its members are filtered individually.
fn api_stability_declaration_path_is_internal(
    ctx: &mut Context<'_, '_>,
    module: P<Symbol>,
    symbol: P<Symbol>,
    declaration: P<Node>,
    visiting: &mut FxHashSet<ApiStabilityInternalVisit>,
) -> bool {
    if internal_tag_of_declaration(declaration) {
        return true;
    }
    let statement = api_stability_forwarding_statement_of(Some(declaration));
    if statement.is_some_and(internal_tag_of_declaration) {
        return true;
    }
    match declaration.kind() {
        Kind::ExportSpecifier => {
            let mut target_module = Some(module);
            if let Some(statement) = statement
                && let Some(module_specifier) = statement.as_export_declaration().module_specifier()
            {
                target_module = api_stability_module_of_specifier(ctx, Some(module_specifier));
            }
            api_stability_aliased_target_is_internal(ctx, target_module, Some(symbol), visiting)
        }
        Kind::ExportAssignment => {
            api_stability_aliased_target_is_internal(ctx, Some(module), Some(symbol), visiting)
        }
        Kind::ImportSpecifier | Kind::ImportClause => {
            api_stability_import_target_is_internal(ctx, declaration, Some(symbol), visiting)
        }
        Kind::ImportEqualsDeclaration => {
            api_stability_import_equals_target_is_internal(ctx, Some(symbol), declaration, visiting)
        }
        _ => false,
    }
}

// go: internal/rules/api_stability_leak.go:556 apiStabilityImportTargetIsInternal
// apiStabilityImportTargetIsInternal follows an untagged import alias to the
// symbol it imports and reports whether that symbol is internal as exported
// from the module it was imported from. An `export { X }` or
// `export default X` of an imported identifier re-exposes the same declaration,
// so it is internal exactly when the imported declaration is. Only the alias
// target named by the import declaration is followed; no other identifier is
// inferred and no type is read.
fn api_stability_import_target_is_internal(
    ctx: &mut Context<'_, '_>,
    declaration: P<Node>,
    symbol: Option<P<Symbol>>,
    visiting: &mut FxHashSet<ApiStabilityInternalVisit>,
) -> bool {
    let Some(symbol) = symbol else {
        return false;
    };
    if !symbol.flags().intersects(SymbolFlags::Alias) {
        return false;
    }
    if !symbol.declarations().iter().any(|&d| ast::is_alias_symbol_declaration(d)) {
        return false;
    }
    let Some(target_module) = api_stability_import_target_module(ctx, declaration) else {
        return false;
    };
    let target = api_stability_immediate_aliased_symbol(ctx.checker(), Some(symbol));
    if target.is_none() || target == Some(symbol) {
        return false;
    }
    api_stability_symbol_is_internal(ctx, Some(target_module), target, visiting)
}

// go: internal/rules/api_stability_leak.go:577 apiStabilityImportTargetModule
// apiStabilityImportTargetModule resolves the module an import alias targets
// through the import declaration that contains the alias. An unresolvable
// specifier reports no module, which keeps the import path public.
fn api_stability_import_target_module(
    ctx: &mut Context<'_, '_>,
    declaration: P<Node>,
) -> Option<P<Symbol>> {
    let mut node = Some(declaration);
    while let Some(current) = node {
        match current.kind() {
            Kind::ImportDeclaration => {
                return api_stability_module_of_specifier(
                    ctx,
                    Some(current.as_import_declaration().module_specifier()),
                );
            }
            Kind::SourceFile | Kind::ModuleBlock => return None,
            _ => {}
        }
        node = current.parent();
    }
    None
}

// go: internal/rules/api_stability_leak.go:600 apiStabilityImportEqualsTargetIsInternal
// apiStabilityImportEqualsTargetIsInternal follows an untagged `import X = ...`
// alias, including the `export import` form, to the target it names and reports
// whether that target is internal as exported from its own module. The checker's
// native immediate alias target is preferred; when the checker left it
// unresolved (a namespace import qualifier, a chained alias or an unknown
// require), the target is read from the qualifier module's export table or from
// the referenced module itself, which is the same declaration metadata the
// checker's own resolution uses. Only the written module reference is followed
// and no type is read, so a tag on the alias declaration or on the target's
// declaration keeps exactly the precedence the named and default import paths
// give it.
fn api_stability_import_equals_target_is_internal(
    ctx: &mut Context<'_, '_>,
    symbol: Option<P<Symbol>>,
    declaration: P<Node>,
    visiting: &mut FxHashSet<ApiStabilityInternalVisit>,
) -> bool {
    let Some(symbol) = symbol else {
        return false;
    };
    if !symbol.flags().intersects(SymbolFlags::Alias) {
        return false;
    }
    if !symbol.declarations().iter().any(|&d| ast::is_alias_symbol_declaration(d)) {
        return false;
    }
    let import_equals = declaration.as_import_equals_declaration();
    let reference = import_equals.module_reference();
    let mut target = api_stability_immediate_aliased_symbol(ctx.checker(), Some(symbol));
    if target.is_none() {
        let name = api_stability_import_equals_target_name(Some(reference));
        if !name.is_empty()
            && let Some(qualifier) = api_stability_module_of_entity(
                ctx,
                api_stability_import_equals_qualifier(Some(reference)),
            )
        {
            target = ctx.checker().try_get_member_in_module_exports_and_properties(name, qualifier);
        }
    }
    if target.is_none() && reference.kind() == Kind::ExternalModuleReference {
        // A native `require` alias stands for the module itself; the module
        // reference is that target when the checker has not resolved it.
        target = api_stability_module_of_entity(
            ctx,
            api_stability_import_equals_qualifier(Some(reference)),
        );
    }
    let Some(target) = target else {
        return false;
    };
    if target == symbol {
        return false;
    }
    let modules = api_stability_declaration_modules(ctx, Some(target));
    if modules.is_empty() {
        return false;
    }
    // The alias forwards one symbol; it is internal only when every module
    // that declares that symbol exposes it internally, like any merged path.
    for module in modules {
        if !api_stability_symbol_is_internal(ctx, Some(module), Some(target), visiting) {
            return false;
        }
    }
    true
}

// go: internal/rules/api_stability_leak.go:646 apiStabilityImportEqualsQualifier
// apiStabilityImportEqualsQualifier returns the entity name that qualifies the
// target of an import-equals reference: the left side of a qualified name (the
// namespace the named member belongs to), the require argument of an external
// module reference, or nil for a bare identifier target.
fn api_stability_import_equals_qualifier(reference: Option<P<Node>>) -> Option<P<Node>> {
    let reference = reference?;
    match reference.kind() {
        Kind::QualifiedName => Some(reference.as_qualified_name().left()),
        Kind::ExternalModuleReference => {
            Some(reference.as_external_module_reference().expression())
        }
        _ => None,
    }
}

// go: internal/rules/api_stability_leak.go:666 apiStabilityImportEqualsTargetName
// apiStabilityImportEqualsTargetName returns the member name an import-equals
// reference names inside its qualifier module. A bare identifier names itself
// and an external `require` names the module, so both report no member name.
fn api_stability_import_equals_target_name(reference: Option<P<Node>>) -> &'static str {
    let Some(reference) = reference else {
        return "";
    };
    if reference.kind() != Kind::QualifiedName {
        return "";
    }
    reference.as_qualified_name().right().text()
}

// go: internal/rules/api_stability_leak.go:682 apiStabilityDeclarationModule
// apiStabilityDeclarationModule returns the module symbol that directly exports
// a declaration: the nearest enclosing declared namespace, or the source file's
// module for a top-level declaration. It is the metadata-only fallback for an
// alias target whose module is not named by a module reference, such as a bare
// identifier target or a chained alias.
fn api_stability_declaration_module(
    ctx: &mut Context<'_, '_>,
    declaration: Option<P<Node>>,
) -> Option<P<Symbol>> {
    let mut node = declaration;
    while let Some(current) = node {
        if matches!(current.kind(), Kind::ModuleDeclaration | Kind::SourceFile) {
            return get_symbol_of_declaration(ctx.checker(), current);
        }
        node = current.parent();
    }
    None
}

// go: internal/rules/api_stability_leak.go:698 apiStabilityDeclarationModules
// apiStabilityDeclarationModules returns the distinct modules that declare a
// symbol, in declaration order. A symbol merged from several modules has one
// visibility path per module.
fn api_stability_declaration_modules(
    ctx: &mut Context<'_, '_>,
    symbol: Option<P<Symbol>>,
) -> Vec<P<Symbol>> {
    let Some(symbol) = symbol else {
        return Vec::new();
    };
    let mut modules = Vec::new();
    for &declaration in symbol.declarations() {
        let Some(module) = api_stability_declaration_module(ctx, Some(declaration)) else {
            continue;
        };
        if modules.contains(&module) {
            continue;
        }
        modules.push(module);
    }
    modules
}

// go: internal/rules/api_stability_leak.go:720 apiStabilityStarPathIsInternal
// apiStabilityStarPathIsInternal reports whether one star re-export path is
// internal. The tag on the star declaration marks the whole path internal;
// otherwise the path inherits the visibility of the symbol in the module it is
// forwarded from.
//
// Rust: Go passes the statement and its `AsExportDeclaration()`; the statement carries both.
fn api_stability_star_path_is_internal(
    ctx: &mut Context<'_, '_>,
    symbol: P<Symbol>,
    statement: P<Node>,
    visiting: &mut FxHashSet<ApiStabilityInternalVisit>,
) -> bool {
    if internal_tag_of_declaration(statement) {
        return true;
    }
    let module_specifier = statement.as_export_declaration().module_specifier();
    let module = api_stability_module_of_specifier(ctx, module_specifier);
    api_stability_symbol_is_internal(ctx, module, Some(symbol), visiting)
}

// go: internal/rules/api_stability_leak.go:731 apiStabilityAliasedTargetIsInternal
// apiStabilityAliasedTargetIsInternal follows an untagged alias declaration to
// its immediate target and reports whether the target is internal. The checker
// synthesizes declaration-less aliases for `export =` and JSON modules and
// panics when asked for their immediate target, so those stay public.
fn api_stability_aliased_target_is_internal(
    ctx: &mut Context<'_, '_>,
    module: Option<P<Symbol>>,
    symbol: Option<P<Symbol>>,
    visiting: &mut FxHashSet<ApiStabilityInternalVisit>,
) -> bool {
    let Some(symbol) = symbol else {
        return false;
    };
    if !symbol.flags().intersects(SymbolFlags::Alias) {
        return false;
    }
    if !symbol.declarations().iter().any(|&d| ast::is_alias_symbol_declaration(d)) {
        return false;
    }
    let target = api_stability_immediate_aliased_symbol(ctx.checker(), Some(symbol));
    if target.is_none() || target == Some(symbol) {
        return false;
    }
    api_stability_symbol_is_internal(ctx, module, target, visiting)
}

// go: internal/rules/api_stability_leak.go:749 apiStabilityForwardingStatementOf
// apiStabilityForwardingStatementOf returns the export declaration statement a
// named specifier or namespace export belongs to. A forwarding tag is written
// above the whole statement, so the statement carries the tag for every
// specifier inside it.
fn api_stability_forwarding_statement_of(declaration: Option<P<Node>>) -> Option<P<Node>> {
    let parent = declaration?.parent()?;
    if parent.kind() == Kind::ExportDeclaration {
        return Some(parent);
    }
    if let Some(grandparent) = parent.parent()
        && grandparent.kind() == Kind::ExportDeclaration
    {
        return Some(grandparent);
    }
    None
}

// go: internal/rules/api_stability_leak.go:767 apiStabilityModuleSourceFiles
// apiStabilityModuleSourceFiles returns the source files a module symbol is
// declared in. A source file module is declared by the file itself; a declared
// namespace or ambient module is declared by its module declaration, and its
// members live in the file containing that declaration. Merged namespace
// declarations therefore contribute every file they appear in.
fn api_stability_module_source_files(module: Option<P<Symbol>>) -> Vec<P<SourceFile>> {
    let Some(module) = module else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for &declaration in module.declarations() {
        let file = match declaration.kind() {
            Kind::SourceFile => Some(declaration.as_source_file_p()),
            Kind::ModuleDeclaration => ast::get_source_file_of_node(declaration),
            _ => None,
        };
        let Some(file) = file else {
            continue;
        };
        if files.contains(&file) {
            continue;
        }
        files.push(file);
    }
    files
}

// go: internal/rules/api_stability_leak.go:798 apiStabilityModuleExportStatements
// apiStabilityModuleExportStatements returns the statements that can forward
// exports out of a module with `export *`: the top-level statements of a source
// file, or the statements of the module blocks of its declared namespaces and
// ambient modules. Named and namespace forwards are declarations of the
// forwarded symbol itself and are inspected through symbol.Declarations; only
// star forwards need this separate enumeration. A dotted namespace declaration
// nests module declarations, so the body chain is followed to its module block.
fn api_stability_module_export_statements(module: Option<P<Symbol>>) -> Vec<P<Node>> {
    let Some(module) = module else {
        return Vec::new();
    };
    let mut statements = Vec::new();
    for &declaration in module.declarations() {
        match declaration.kind() {
            Kind::SourceFile => {
                statements.extend_from_slice(declaration.as_source_file().statements().nodes());
            }
            Kind::ModuleDeclaration => {
                if let Some(block) = api_stability_module_block_of(declaration) {
                    statements.extend_from_slice(block.as_module_block().statements().nodes());
                }
            }
            _ => {}
        }
    }
    statements
}

// go: internal/rules/api_stability_leak.go:824 apiStabilityModuleBlockOf
// apiStabilityModuleBlockOf returns the module block at the end of a module
// declaration's body chain. A namespace declared without a body (an ambient
// declaration) has no block and forwards nothing.
//
// Rust: returns the `ModuleBlock` node (Go returns its `AsModuleBlock()`).
fn api_stability_module_block_of(declaration: P<Node>) -> Option<P<Node>> {
    let mut node = declaration;
    while node.kind() == Kind::ModuleDeclaration {
        let body = node.body()?;
        if body.kind() == Kind::ModuleBlock {
            return Some(body);
        }
        node = body;
    }
    None
}

// go: internal/rules/api_stability_leak.go:838 apiStabilityDeclarationInFiles
fn api_stability_declaration_in_files(declaration: P<Node>, files: &[P<SourceFile>]) -> bool {
    ast::get_source_file_of_node(declaration).is_some_and(|file| files.contains(&file))
}

// go: internal/rules/api_stability_leak.go:846 apiStabilityModuleOfSpecifier
// apiStabilityModuleOfSpecifier resolves the module symbol a module specifier
// refers to. An unresolved specifier reports no module, which keeps the
// forwarding path public.
fn api_stability_module_of_specifier(
    ctx: &mut Context<'_, '_>,
    module_specifier: Option<P<Node>>,
) -> Option<P<Symbol>> {
    api_stability_module_of_entity(ctx, module_specifier)
}

// go: internal/rules/api_stability_leak.go:854 apiStabilityModuleOfEntity
// apiStabilityModuleOfEntity resolves the module symbol an entity name or module
// specifier refers to, following an import alias to the module it names. An
// unresolved entity or a symbol that declares no module reports no module, which
// keeps the forwarding path public.
fn api_stability_module_of_entity(
    ctx: &mut Context<'_, '_>,
    entity: Option<P<Node>>,
) -> Option<P<Symbol>> {
    let entity = entity?;
    let mut module = ctx.type_parser.get_symbol_at_location(entity)?;
    if module.flags().intersects(SymbolFlags::Alias) {
        module = ctx.checker().get_aliased_symbol(module);
    }
    if api_stability_module_source_files(Some(module)).is_empty() {
        return None;
    }
    Some(module)
}

// go: internal/rules/api_stability_leak.go:871 apiStabilityReexportLocation
fn api_stability_reexport_location(
    ctx: &mut Context<'_, '_>,
    export_symbol: P<Symbol>,
) -> Option<P<Node>> {
    let mut fallback = None;
    for &statement in ctx.source_file.statements().nodes() {
        if statement.kind() != Kind::ExportDeclaration {
            continue;
        }
        let declaration = statement.as_export_declaration();
        let Some(module_specifier) = declaration.module_specifier() else {
            continue;
        };
        if declaration.export_clause().is_some() {
            continue;
        }
        if fallback.is_none() {
            fallback = Some(statement);
        }
        let Some(mut module_symbol) = ctx.type_parser.get_symbol_at_location(module_specifier)
        else {
            continue;
        };
        if module_symbol.flags().intersects(SymbolFlags::Alias) {
            module_symbol = ctx.checker().get_aliased_symbol(module_symbol);
        }
        let exported = ctx
            .checker()
            .try_get_member_in_module_exports_and_properties(export_symbol.name(), module_symbol);
        if exported.is_some()
            && get_symbol_if_same_reference(ctx.checker(), exported, Some(export_symbol)).is_some()
        {
            return Some(statement);
        }
    }
    fallback
}
