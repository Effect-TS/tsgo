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
