use ets_fixable::Context;
use ets_rewriter::Tracker;
use ets_shim::P;
use ets_shim::ast::{self, Kind, Node, Symbol, SymbolFlags};
use ets_shim::checker::Type;
use ets_typeparser::{ModuleReference, TypeParser};

/// Rust-only: Go's `func(node *ast.Node) bool` closures over `ctx.TypeParser`; the parser is passed in, since it
/// cannot be captured next to the context that owns it.
pub(crate) type ModuleMemberPredicate = Box<dyn Fn(&mut TypeParser<'_>, P<Node>) -> bool>;

// go: internal/fixables/module_member_reference.go:15 moduleMember b5c85a37ff4aa953
// moduleMember describes a module export that a fix wants to reference.
pub(crate) struct ModuleMember {
    pub(crate) name: &'static str,
    pub(crate) is_module: ModuleMemberPredicate, // node references the module namespace
    pub(crate) is_member: ModuleMemberPredicate, // node references the export itself
}

// go: internal/fixables/module_member_reference.go:22 moduleMemberReference cc0f87f0e0dc9067
// moduleMemberReference is a path of names, visible at a location, that reaches a moduleMember.
#[derive(Clone)]
pub(crate) struct ModuleMemberReference {
    pub(crate) path: ModuleReference,
    pub(crate) via_module: bool, // path is the module namespace, so the member is accessed on it
}

// go: internal/fixables/module_member_reference.go:37 resolveModuleMember 9cb4b3955c797837
// resolveModuleMember finds a reference that resolves at location to member:
// preferred, the file's import of the module, when it reaches member,
// otherwise an import binding in scope. It returns nil when nothing reaches
// the export, so the fix never introduces an unbound, shadowed or
// uninitialized name.
//
// Local aliases are never used: a declaration that precedes location in the
// text may still be uninitialized when location runs (a sibling switch clause,
// or a hoisted function called before the declaration). Import bindings are
// initialized before any module code runs.
//
// Rust: `GetSymbolsInScope` returns one symbol per name (the checker collects them in a symbol table keyed by
// name), so the keys of Go's unstable sort are unique and a stable sort orders them the same way.
pub(crate) fn resolve_module_member(
    ctx: &mut Context<'_, '_>,
    location: P<Node>,
    preferred: &ModuleReference,
    member: &ModuleMember,
) -> Option<ModuleMemberReference> {
    if let Some(reference) = imported_module_member(ctx, location, preferred, member) {
        return Some(reference);
    }
    let mut symbols = ctx
        .checker()
        .get_symbols_in_scope_exported(location, SymbolFlags::Value | SymbolFlags::Alias);
    symbols.sort_by(|left, right| left.name().cmp(right.name()));
    for symbol in symbols {
        let Some(name) = import_binding_name(ctx, location, Some(symbol)) else {
            continue;
        };
        if (member.is_member)(&mut ctx.type_parser, name) {
            return Some(ModuleMemberReference {
                path: ModuleReference(vec![name.text().to_string()]),
                via_module: false,
            });
        }
        if (member.is_module)(&mut ctx.type_parser, name) {
            let name_type = ctx.type_parser.get_type_at_location(name);
            if type_has_member(ctx, name_type, member) {
                return Some(ModuleMemberReference {
                    path: ModuleReference(vec![name.text().to_string()]),
                    via_module: true,
                });
            }
        }
    }
    None
}

// go: internal/fixables/module_member_reference.go:63 importedModuleMember 3d7ca1ae927a514b
// importedModuleMember returns module as a reference to member when its first
// name resolves at location to an import binding and the value reached by
// following the rest of the path has member, or nil otherwise.
pub(crate) fn imported_module_member(
    ctx: &mut Context<'_, '_>,
    location: P<Node>,
    module: &ModuleReference,
    member: &ModuleMember,
) -> Option<ModuleMemberReference> {
    if module.is_empty() {
        return None;
    }
    let symbol = ctx.checker().resolve_name_exported(
        &module[0],
        location,
        SymbolFlags::Value | SymbolFlags::Alias,
        false,
    );
    let name = import_binding_name(ctx, location, symbol)?;
    let mut t = ctx.type_parser.get_type_at_location(name);
    for property in &module[1..] {
        let symbol = ctx.checker().get_property_of_type_exported(t?, property)?;
        t = Some(ctx.checker().get_type_of_symbol_exported(symbol));
    }
    if !type_has_member(ctx, t, member) {
        return None;
    }
    Some(ModuleMemberReference { path: module.clone(), via_module: true })
}

// go: internal/fixables/module_member_reference.go:89 importBindingName 1c8729d820eccea1
// importBindingName returns the name of symbol's value import declaration
// when that name resolves to symbol at location, so a generated identifier
// reaches the import and not a local that shadows it. Bindings that are
// type-only, directly or through a type-only re-export, are not values.
pub(crate) fn import_binding_name(
    ctx: &mut Context<'_, '_>,
    location: P<Node>,
    symbol: Option<P<Symbol>>,
) -> Option<P<Node>> {
    let symbol = symbol?;
    if symbol.declarations().len() != 1
        || ctx.checker().get_type_only_alias_declaration(symbol).is_some()
    {
        return None;
    }
    let declaration = symbol.declarations()[0];
    if declaration.kind() != Kind::ImportSpecifier && declaration.kind() != Kind::NamespaceImport
        || ast::is_part_of_type_only_import_or_export_declaration(declaration)
    {
        return None;
    }
    let name = declaration.name()?;
    if name.kind() != Kind::Identifier
        || ctx.checker().resolve_name_exported(
            name.text(),
            location,
            SymbolFlags::Value | SymbolFlags::Alias,
            false,
        ) != Some(symbol)
    {
        return None;
    }
    Some(name)
}

// go: internal/fixables/module_member_reference.go:106 typeHasMember a56dd1633ca84926
pub(crate) fn type_has_member(
    ctx: &mut Context<'_, '_>,
    t: Option<P<Type>>,
    member: &ModuleMember,
) -> bool {
    let Some(t) = t else {
        return false;
    };
    let property = ctx.checker().get_property_of_type_exported(t, member.name);
    let Some(property) = property else {
        return false;
    };
    let mut declaration = property.value_declaration();
    if declaration.is_none()
        && property.flags().intersects(SymbolFlags::Alias)
        && property.declarations().len() == 1
    {
        // Aliased exports such as `export { undefined_ as undefined }` are declared by their export specifier.
        declaration = Some(property.declarations()[0]);
    }
    let Some(declaration) = declaration else {
        return false;
    };
    declaration.name().is_some_and(|name| (member.is_member)(&mut ctx.type_parser, name))
}

impl ModuleMemberReference {
    // go: internal/fixables/module_member_reference.go:123 moduleMemberReference.expression 62cc0fa23fcc2f21
    // expression builds the reference: module.member, or the direct binding.
    pub(crate) fn expression(&self, tracker: &Tracker, member: &ModuleMember) -> P<Node> {
        if self.via_module {
            return tracker.new_property_access_path(&self.path.append(&[member.name]));
        }
        tracker.new_property_access_path(&self.path)
    }
}

// go: internal/fixables/module_member_reference.go:135 calleeModuleMemberReference 189854cf13ee3100
// calleeModuleMemberReference keeps the qualifier a callee is written with:
// Q.f gives Q.<member>, and a named import of f gives a bare <member> from the
// same import. When the bare name has to be imported, namedImports is the list
// to add it to. Otherwise it falls back to module, the file's reference to the
// member's module.
pub(crate) fn callee_module_member_reference(
    ctx: &mut Context<'_, '_>,
    callee: P<Node>,
    module: &ModuleReference,
    member: &ModuleMember,
) -> (Option<ModuleMemberReference>, Option<P<Node>>) {
    if let Some(qualifier) = callee_module_qualifier(ctx, callee, member)
        && let Some(reference) = imported_module_member(ctx, callee, &qualifier, member)
    {
        return (Some(reference), None);
    }
    if let (Some(reference), named_imports) = named_import_module_member(ctx, callee, member) {
        return (Some(reference), named_imports);
    }
    (resolve_module_member(ctx, callee, module, member), None)
}

// go: internal/fixables/module_member_reference.go:149 calleeModuleQualifier 81e14d2672dd2fd6
// calleeModuleQualifier returns the module path of a Q.f callee whose Q is
// member's module.
//
// Rust: Go's nil `ModuleReference` is `None`; the context is passed for the predicate's parser.
fn callee_module_qualifier(
    ctx: &mut Context<'_, '_>,
    callee: P<Node>,
    member: &ModuleMember,
) -> Option<ModuleReference> {
    let callee = ast::skip_parentheses(callee);
    if !ast::is_property_access_expression(callee) {
        return None;
    }
    let qualifier = callee.expression()?;
    if !(member.is_module)(&mut ctx.type_parser, qualifier) {
        return None;
    }
    identifier_path(qualifier)
}

// go: internal/fixables/module_member_reference.go:163 identifierPath 63de0494736b1d00
// identifierPath returns the names of an identifier or a chain of property
// accesses on identifiers, or nil for any other expression.
//
// Rust: Go's nil `ModuleReference` is `None`.
fn identifier_path(node: P<Node>) -> Option<ModuleReference> {
    if ast::is_identifier(node) {
        return Some(ModuleReference(vec![node.text().to_string()]));
    } else if ast::is_property_access_expression(node)
        && let Some(name) = node.name()
        && ast::is_identifier(name)
        && let Some(path) = node.expression().and_then(identifier_path)
    {
        return Some(path.append(&[name.text()]));
    }
    None
}

// go: internal/fixables/module_member_reference.go:179 namedImportModuleMember bebf88d4055c2fc9
// namedImportModuleMember handles a callee that is a named import binding of
// member's module: it reuses a bare member binding already in scope, or returns
// the bare member name with the callee's NamedImports to add it to when the
// name is free and the imported module exports it.
fn named_import_module_member(
    ctx: &mut Context<'_, '_>,
    callee: P<Node>,
    member: &ModuleMember,
) -> (Option<ModuleMemberReference>, Option<P<Node>>) {
    let callee = ast::skip_parentheses(callee);
    if !ast::is_identifier(callee) {
        return (None, None);
    }
    let symbol = ctx.checker().resolve_name_exported(
        callee.text(),
        callee,
        SymbolFlags::Value | SymbolFlags::Alias,
        false,
    );
    let binding = import_binding_name(ctx, callee, symbol);
    let Some(binding_parent) = binding.and_then(|binding| binding.parent()) else {
        return (None, None);
    };
    if !ast::is_import_specifier(binding_parent) {
        return (None, None);
    }
    let Some(named_imports) = binding_parent.parent() else {
        return (None, None);
    };
    let Some(import_declaration) = named_imports.parent().and_then(|clause| clause.parent()) else {
        return (None, None);
    };
    if !ast::is_named_imports(named_imports) || !ast::is_import_declaration(import_declaration) {
        return (None, None);
    }
    let module_symbol = ctx
        .type_parser
        .get_symbol_at_location(import_declaration.as_import_declaration().module_specifier());
    let Some(module_symbol) = module_symbol else {
        return (None, None);
    };
    let export = ctx.checker().try_get_member_in_module_exports(member.name, module_symbol);
    let Some(mut export) = export else {
        return (None, None);
    };
    if export.flags().intersects(SymbolFlags::Alias) {
        export = ctx.checker().get_aliased_symbol(export);
    }
    let export_name = export.value_declaration().and_then(|declaration| declaration.name());
    let Some(export_name) = export_name else {
        return (None, None);
    };
    if !(member.is_member)(&mut ctx.type_parser, export_name) {
        return (None, None);
    }

    let existing = ctx.checker().resolve_name_exported(
        member.name,
        callee,
        SymbolFlags::Value | SymbolFlags::Alias,
        false,
    );
    if existing.is_none() {
        return (
            Some(ModuleMemberReference {
                path: ModuleReference(vec![member.name.to_string()]),
                via_module: false,
            }),
            Some(named_imports),
        );
    }
    if let Some(name) = import_binding_name(ctx, callee, existing)
        && (member.is_member)(&mut ctx.type_parser, name)
    {
        return (
            Some(ModuleMemberReference {
                path: ModuleReference(vec![name.text().to_string()]),
                via_module: false,
            }),
            None,
        );
    }
    (None, None)
}
