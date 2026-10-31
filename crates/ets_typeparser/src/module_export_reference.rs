use std::sync::Arc;

use ets_shim::ast::{Kind, Node, NodeFlags, SourceFile, Symbol};
use ets_shim::core::P;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::*;

/// Rust-only: Go's `func(*TypeParser, *checker.Checker, *ast.SourceFile) bool`; the checker argument is always
/// the parser's own checker, which the parser already lends (`tp.checker`). A shared reference to a function or a
/// closure, as Go's func value (the package-level descriptors hold functions; tests hold counting closures).
pub(crate) type MatchesSourceFile =
    &'static (dyn Fn(&mut TypeParser<'_>, P<SourceFile>) -> bool + Sync);

// go: internal/typeparser/module_export_reference.go:11 packageSourceFileDescriptor c63ec9fcb5774be9
pub(crate) struct PackageSourceFileDescriptor {
    pub(crate) id: ModuleDescriptorId,
    pub(crate) package_name: &'static str,
    pub(crate) matches_source_file: Option<MatchesSourceFile>,
}

// go: internal/typeparser/module_export_reference.go:17 moduleDescriptorID c0c57ce60025df85
// go: internal/typeparser/module_export_reference.go:19 moduleDescriptorEffect db4c89bc296667e0
// Rust: a newtype over Go's `uint8`, since ids past `moduleDescriptorCount` are valid (Go's tests use them).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct ModuleDescriptorId(pub(crate) u8);

// go: internal/typeparser/module_export_reference.go:19 moduleDescriptorEffect db4c89bc296667e0
impl ModuleDescriptorId {
    pub(crate) const Effect: ModuleDescriptorId = ModuleDescriptorId(0);
    pub(crate) const EffectPackage: ModuleDescriptorId = ModuleDescriptorId(1);
    pub(crate) const Layer: ModuleDescriptorId = ModuleDescriptorId(2);
    pub(crate) const Schema: ModuleDescriptorId = ModuleDescriptorId(3);
    pub(crate) const ParseResult: ModuleDescriptorId = ModuleDescriptorId(4);
    pub(crate) const SchemaParser: ModuleDescriptorId = ModuleDescriptorId(5);
    pub(crate) const Predicate: ModuleDescriptorId = ModuleDescriptorId(6);
    pub(crate) const Option: ModuleDescriptorId = ModuleDescriptorId(7);
    pub(crate) const Duration: ModuleDescriptorId = ModuleDescriptorId(8);
    pub(crate) const EffectModel: ModuleDescriptorId = ModuleDescriptorId(9);
    pub(crate) const SqlModel: ModuleDescriptorId = ModuleDescriptorId(10);
    pub(crate) const Context: ModuleDescriptorId = ModuleDescriptorId(11);
    pub(crate) const Cause: ModuleDescriptorId = ModuleDescriptorId(12);
    pub(crate) const Data: ModuleDescriptorId = ModuleDescriptorId(13);
    pub(crate) const VitestRunner: ModuleDescriptorId = ModuleDescriptorId(14);
    pub(crate) const Vitest: ModuleDescriptorId = ModuleDescriptorId(15);
    pub(crate) const EffectVitest: ModuleDescriptorId = ModuleDescriptorId(16);
    pub(crate) const Function: ModuleDescriptorId = ModuleDescriptorId(17);
    pub(crate) const Metric: ModuleDescriptorId = ModuleDescriptorId(18);
    pub(crate) const PlatformNode: ModuleDescriptorId = ModuleDescriptorId(19);
    pub(crate) const PlatformBun: ModuleDescriptorId = ModuleDescriptorId(20);
    pub(crate) const PlatformBrowser: ModuleDescriptorId = ModuleDescriptorId(21);
    pub(crate) const AiTool: ModuleDescriptorId = ModuleDescriptorId(22);
    #[cfg_attr(not(test), expect(dead_code, reason = "only the tests read it, as in Go"))]
    pub(crate) const Count: ModuleDescriptorId = ModuleDescriptorId(23);
}

// go: internal/typeparser/module_export_reference.go:46 moduleDescriptorMask 2ec1f65c138b974c
pub(crate) type ModuleDescriptorMask = u32;

// go: internal/typeparser/module_export_reference.go:48 maxModuleDescriptors 83aab3d89b4b8823
pub(crate) const MAX_MODULE_DESCRIPTORS: u8 = 32;

// go: internal/typeparser/module_export_reference.go:50 moduleExportFilter b15675d64f1625f8
#[derive(Clone, Copy, Default)]
pub(crate) struct ModuleExportFilter {
    checked: ModuleDescriptorMask,
    has_declaration: ModuleDescriptorMask,
}

// go: internal/typeparser/module_export_reference.go:57 moduleExportIdentity c49ea6a711fe42f8
// moduleExportIdentity is one module file declaring a symbol, with the names
// that module exports the symbol under.
#[derive(Clone)]
pub(crate) struct ModuleExportIdentity {
    file: P<SourceFile>,
    names: Vec<String>,
    // exportEquals marks an `export =` module. Members of its exported value
    // count as exports too, so it is answered by looking the member up.
    export_equals: bool,
}

// go: internal/typeparser/module_export_reference.go:66 moduleExportNames e7c3e4e16385b040
// moduleExportNames indexes a module's export names by resolved target.
//
// Rust: the map is shared between the cache and its readers, as Go's map is when the struct is copied.
#[derive(Clone, Default)]
pub(crate) struct ModuleExportNames {
    by_target: Arc<FxHashMap<P<Symbol>, Vec<String>>>,
    export_equals: bool,
}

// go: internal/typeparser/module_export_reference.go:71 newPackageSourceFileDescriptor 032990b1c4133a90
pub(crate) const fn new_package_source_file_descriptor(
    id: ModuleDescriptorId,
    package_name: &'static str,
    matches_source_file: Option<MatchesSourceFile>,
) -> PackageSourceFileDescriptor {
    assert!(
        id.0 < MAX_MODULE_DESCRIPTORS,
        "typeparser: package source file descriptor id does not fit moduleDescriptorMask"
    );
    PackageSourceFileDescriptor { id, package_name, matches_source_file }
}

impl TypeParser<'_> {
    // go: internal/typeparser/module_export_reference.go:86 TypeParser.ReferenceSymbolAtNode 6d4ef47fdf0bf00e
    pub fn reference_symbol_at_node(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> Option<P<Symbol>> {
        let node = node.into()?;
        if !node.flags().intersects(NodeFlags::JSDoc) && !can_have_symbol_at_location(node.kind()) {
            return None;
        }

        cached_node(
            self,
            |l| &mut l.reference_symbol,
            node,
            |tp| {
                let symbol = tp.get_symbol_at_location(node);
                tp.reference_symbol_from(node, symbol)
            },
        )
    }
}

// go: internal/typeparser/module_export_reference.go:104 canHaveSymbolAtLocation e32e68707bed4abc
// canHaveSymbolAtLocation reports whether the checker's getSymbolAtLocation
// can return a symbol for a node of this kind: the source file, the
// declaration names it resolves before its switch, and the kinds its switch
// handles. Every other kind falls through to its nil default. JSDoc nodes can
// be swapped for their reparsed clones, so callers must not gate them.
pub(crate) fn can_have_symbol_at_location(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::SourceFile
            | Kind::Identifier
            | Kind::PrivateIdentifier
            | Kind::StringLiteral
            | Kind::NumericLiteral
            | Kind::BigIntLiteral
            | Kind::NoSubstitutionTemplateLiteral
            | Kind::ComputedPropertyName
            | Kind::PropertyAccessExpression
            | Kind::ElementAccessExpression
            | Kind::QualifiedName
            | Kind::ThisKeyword
            | Kind::ThisType
            | Kind::SuperKeyword
            | Kind::ConstructorKeyword
            | Kind::DefaultKeyword
            | Kind::FunctionKeyword
            | Kind::EqualsGreaterThanToken
            | Kind::ClassKeyword
            | Kind::ImportType
            | Kind::ExportKeyword
            | Kind::ImportKeyword
            | Kind::NewKeyword
            | Kind::InstanceOfKeyword
            | Kind::MetaProperty
            | Kind::JsxNamespacedName
    )
}

impl TypeParser<'_> {
    // go: internal/typeparser/module_export_reference.go:121 TypeParser.SymbolAndReferenceSymbolAtNode f7dce8e903e7c864
    pub fn symbol_and_reference_symbol_at_node(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> (Option<P<Symbol>>, Option<P<Symbol>>) {
        let node = node.into();
        let symbol = self.get_symbol_at_location(node);
        let Some(node) = node else {
            return (symbol, None);
        };

        (
            symbol,
            cached_node(
                self,
                |l| &mut l.reference_symbol,
                node,
                |tp| tp.reference_symbol_from(node, symbol),
            ),
        )
    }

    // go: internal/typeparser/module_export_reference.go:132 TypeParser.referenceSymbolFrom 1388caa3f5e9db4c
    pub(crate) fn reference_symbol_from(
        &mut self,
        node: P<Node>,
        sym: Option<P<Symbol>>,
    ) -> Option<P<Symbol>> {
        let mut sym = sym;
        if sym.is_none() && node.kind() == Kind::PropertyAccessExpression {
            let prop = node.as_property_access_expression();
            sym = self.get_symbol_at_location(prop.name());
        }

        sym = self.resolve_aliased_symbol(sym);
        if node.kind() == Kind::Identifier && !ets_shim::ast::is_declaration_name(node) {
            return self.resolve_constant_alias_symbol(sym);
        }
        sym
    }

    // go: internal/typeparser/module_export_reference.go:146 TypeParser.resolveConstantAliasSymbol 2c981f2a8e17f156
    pub(crate) fn resolve_constant_alias_symbol(
        &mut self,
        sym: Option<P<Symbol>>,
    ) -> Option<P<Symbol>> {
        let mut seen: FxHashSet<P<Symbol>> = FxHashSet::default();
        let mut sym = sym;
        while let Some(s) = sym {
            if seen.contains(&s) {
                return Some(s);
            }
            seen.insert(s);

            let Some(declaration_node) = s.value_declaration() else {
                return Some(s);
            };
            if declaration_node.kind() != Kind::VariableDeclaration
                || declaration_node
                    .parent()
                    .is_none_or(|parent| parent.kind() != Kind::VariableDeclarationList)
                || !declaration_node
                    .parent()
                    .is_some_and(|parent| parent.flags().intersects(NodeFlags::Const))
            {
                return Some(s);
            }
            let Some(initializer) = declaration_node.as_variable_declaration().initializer() else {
                return Some(s);
            };

            let Some(next) =
                self.get_symbol_at_location(ets_shim::ast::skip_parentheses(initializer))
            else {
                return Some(s);
            };
            sym = self.resolve_aliased_symbol(Some(next));
        }
        None
    }

    // go: internal/typeparser/module_export_reference.go:174 TypeParser.IsSourceFileInPackage e80222d19be62dbb
    pub fn is_source_file_in_package(
        &mut self,
        sf: impl Into<Option<P<SourceFile>>>,
        package_name: &str,
    ) -> bool {
        let Some(sf) = sf.into() else {
            return false;
        };
        let Some(pkg) = self.package_json_for_source_file(sf) else {
            return false;
        };
        // Rust: Go's strings.EqualFold. The package names in scope ("effect") are ASCII without a letter that has a
        // non-ASCII case-fold partner (k, s), so ASCII folding gives the same answer for every input.
        pkg.name.get_value().is_some_and(|name| name.eq_ignore_ascii_case(package_name))
    }

    // go: internal/typeparser/module_export_reference.go:186 TypeParser.IsNodeReferenceToModuleExport 5c8dfbb75b08b11f
    pub(crate) fn is_node_reference_to_module_export(
        &mut self,
        node: P<Node>,
        desc: &PackageSourceFileDescriptor,
        member_name: &str,
    ) -> bool {
        let Some(sym) = self.reference_symbol_at_node(node) else {
            return false;
        };
        if !self.symbol_has_declaration_in_module_cached(sym, desc) {
            return false;
        }

        for identity in self.module_export_identities(sym).iter() {
            if !self.source_file_matches_module_cached(identity.file, desc) {
                continue;
            }
            if identity.export_equals {
                if self.is_symbol_exported_from_module_file(sym, identity.file, member_name) {
                    return true;
                }
            } else if identity.names.iter().any(|name| name == member_name) {
                return true;
            }
        }
        false
    }

    // go: internal/typeparser/module_export_reference.go:213 TypeParser.moduleExportIdentities 17415ec68022e5e7
    // moduleExportIdentities lists, once per symbol, the modules declaring sym
    // and the names each exports it under. Only files inside a package can match a
    // module descriptor, so other declarations are skipped.
    pub(crate) fn module_export_identities(
        &mut self,
        sym: P<Symbol>,
    ) -> Arc<[ModuleExportIdentity]> {
        if let Some(identities) = self.links().module_export_identities.get(&sym) {
            return Arc::clone(identities);
        }

        let mut identities: Vec<ModuleExportIdentity> = Vec::new();
        let target = self.resolved_target(Some(sym));
        for &decl in sym.declarations() {
            let Some(sf) = ets_shim::ast::get_source_file_of_node(decl) else {
                continue;
            };
            if self.package_json_for_source_file(sf).is_none()
                || identities.iter().any(|identity| identity.file == sf)
            {
                continue;
            }
            let exports = self.module_export_names_of(sf);
            if exports.export_equals {
                identities.push(ModuleExportIdentity {
                    file: sf,
                    names: Vec::new(),
                    export_equals: true,
                });
            } else if let Some(names) = target.and_then(|target| exports.by_target.get(&target))
                && !names.is_empty()
            {
                identities.push(ModuleExportIdentity {
                    file: sf,
                    names: names.clone(),
                    export_equals: false,
                });
            }
        }
        let identities: Arc<[ModuleExportIdentity]> = identities.into();
        self.links().module_export_identities.insert(sym, Arc::clone(&identities));
        identities
    }

    // go: internal/typeparser/module_export_reference.go:242 TypeParser.moduleExportNamesOf 38895268011dbf2f
    // moduleExportNamesOf groups a module's export names by the symbol each
    // resolves to, comparing symbols the way getSymbolIfSameReference does.
    pub(crate) fn module_export_names_of(&mut self, sf: P<SourceFile>) -> ModuleExportNames {
        cached(
            self,
            |l| &mut l.module_export_names,
            sf,
            |tp| {
                let Some(module_sym) = get_symbol_of_declaration(tp.checker, sf.as_node()) else {
                    return ModuleExportNames::default();
                };
                if tp.checker.resolve_external_module_symbol_exported(module_sym) != module_sym {
                    return ModuleExportNames { by_target: Arc::default(), export_equals: true };
                }
                // Rust: Go resolves each export inside the checker's callback; tsrs's callback cannot reach the parser,
                // so the exports are collected first, then resolved in the same order. The callback only reads the
                // module's export table, which resolving an alias does not change.
                let mut exports: Vec<(P<Symbol>, String)> = Vec::new();
                tp.checker.for_each_export_and_property_of_module(
                    module_sym,
                    |_, export_sym, name| {
                        exports.push((export_sym, name.to_string()));
                    },
                );
                let mut by_target: FxHashMap<P<Symbol>, Vec<String>> = FxHashMap::default();
                for (export_sym, name) in exports {
                    let resolved = tp.resolve_aliased_symbol(Some(export_sym));
                    if let Some(target) = tp.resolved_target(resolved) {
                        by_target.entry(target).or_default().push(name);
                    }
                }
                ModuleExportNames { by_target: Arc::new(by_target), export_equals: false }
            },
        )
    }

    // go: internal/typeparser/module_export_reference.go:263 TypeParser.resolvedTarget 8ece49eeeffa4da9
    // resolvedTarget mirrors the checker's getResolvedTarget, the key
    // getSymbolIfSameReference compares.
    pub(crate) fn resolved_target(&mut self, sym: Option<P<Symbol>>) -> Option<P<Symbol>> {
        get_resolved_target(self.checker, sym)
    }

    // go: internal/typeparser/module_export_reference.go:270 TypeParser.sourceFileMatchesModuleCached 30688fc60e854f15
    pub(crate) fn source_file_matches_module_cached(
        &mut self,
        sf: P<SourceFile>,
        desc: &PackageSourceFileDescriptor,
    ) -> bool {
        let bit: ModuleDescriptorMask = 1 << desc.id.0;
        // The checked bit is set before the computation, so a recursive query for the same file and descriptor
        // answers false (Go writes through the store's pointer the same way).
        let checked = {
            let mut links = self.links();
            let filter = links.module_file_filter.entry(sf).or_default();
            let checked = filter.checked & bit != 0;
            if !checked {
                filter.checked |= bit;
            }
            checked
        };
        if !checked && self.source_file_matches_module(sf, desc) {
            self.links().module_file_filter.entry(sf).or_default().has_declaration |= bit;
        }
        self.links().module_file_filter.entry(sf).or_default().has_declaration & bit != 0
    }

    // go: internal/typeparser/module_export_reference.go:282 TypeParser.sourceFileMatchesModule cb2f50ce455060c1
    pub(crate) fn source_file_matches_module(
        &mut self,
        sf: P<SourceFile>,
        desc: &PackageSourceFileDescriptor,
    ) -> bool {
        self.is_source_file_in_package(sf, desc.package_name)
            && desc
                .matches_source_file
                .is_none_or(|matches_source_file| matches_source_file(self, sf))
    }

    // go: internal/typeparser/module_export_reference.go:287 TypeParser.symbolHasDeclarationInModuleCached e4f37277548be2d4
    pub(crate) fn symbol_has_declaration_in_module_cached(
        &mut self,
        sym: P<Symbol>,
        desc: &PackageSourceFileDescriptor,
    ) -> bool {
        // With one declaration the per-file filter already answers in one lookup.
        if sym.declarations().len() == 1 {
            return self.symbol_has_declaration_in_module(sym, desc);
        }
        let bit: ModuleDescriptorMask = 1 << desc.id.0;
        // The checked bit is set before the computation, so a recursive query for the same symbol and descriptor
        // answers false (Go writes through the store's pointer the same way).
        let checked = {
            let mut links = self.links();
            let filter = links.module_export_filter.entry(sym).or_default();
            let checked = filter.checked & bit != 0;
            if !checked {
                filter.checked |= bit;
            }
            checked
        };
        if !checked && self.symbol_has_declaration_in_module(sym, desc) {
            self.links().module_export_filter.entry(sym).or_default().has_declaration |= bit;
        }
        self.links().module_export_filter.entry(sym).or_default().has_declaration & bit != 0
    }

    // go: internal/typeparser/module_export_reference.go:303 TypeParser.symbolHasDeclarationInModule 2d9a56ae98119695
    pub(crate) fn symbol_has_declaration_in_module(
        &mut self,
        sym: P<Symbol>,
        desc: &PackageSourceFileDescriptor,
    ) -> bool {
        for &decl in sym.declarations() {
            if let Some(sf) = ets_shim::ast::get_source_file_of_node(decl)
                && self.source_file_matches_module_cached(sf, desc)
            {
                return true;
            }
        }
        false
    }

    // go: internal/typeparser/module_export_reference.go:317 TypeParser.isSymbolExportedFromModuleFile fba029652f1112eb
    // isSymbolExportedFromModuleFile looks memberName up in one module, including
    // the members of an `export =` value.
    pub(crate) fn is_symbol_exported_from_module_file(
        &mut self,
        sym: P<Symbol>,
        sf: P<SourceFile>,
        member_name: &str,
    ) -> bool {
        let Some(module_sym) = get_symbol_of_declaration(self.checker, sf.as_node()) else {
            return false;
        };
        let export_sym =
            self.checker.try_get_member_in_module_exports_and_properties(member_name, module_sym);
        let export_sym = self.resolve_aliased_symbol(export_sym);
        get_symbol_if_same_reference(self.checker, export_sym, Some(sym)).is_some()
    }

    // go: internal/typeparser/module_export_reference.go:326 TypeParser.IsNodeReferenceToModule c6800facbd61272b
    pub(crate) fn is_node_reference_to_module(
        &mut self,
        node: impl Into<Option<P<Node>>>,
        desc: &PackageSourceFileDescriptor,
    ) -> bool {
        let Some(sym) = self.reference_symbol_at_node(node) else {
            return false;
        };
        self.symbol_has_declaration_in_module_cached(sym, desc)
    }
}
