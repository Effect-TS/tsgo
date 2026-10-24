//! Rust-only: the effect-tsgo shim accessors and TypeScript-patch APIs the type parser calls that tsrs does not make
//! public (`docs/TSRS_API.md`, "Gaps"). Each helper computes what the Go function computes, from public tsrs building
//! blocks; none reimplements checker logic beyond the few lines of the Go function it stands for.

use ets_shim::ast::{
    InternalSymbolNameComputed, ModifierFlags, Node, NodeFlags, SourceFile, Symbol, SymbolFlags,
};
use ets_shim::checker::{
    AliasArg, Checker, MappedType, Signature, Type, TypeFlags, UnionReduction,
};
use ets_shim::core::P;

// Rust-only: checker.Checker_getSymbolOfDeclaration is not public in tsrs.
// Go: `getMergedSymbol(getLateBoundSymbol(node.Symbol()))`, nil for a node without a symbol. getLateBoundSymbol is
// the identity except for a class member whose name is late-bound (`__computed`); for those the checker's own
// public `GetSymbolAtLocation` of the declaration name returns `getSymbolOfDeclaration(declaration)` (its
// declaration-name branch), which forces the late binding as Go does.
pub fn get_symbol_of_declaration(c: &mut Checker, node: P<Node>) -> Option<P<Symbol>> {
    let symbol = node.symbol()?;
    if symbol.flags().intersects(SymbolFlags::ClassMember)
        && symbol.name() == InternalSymbolNameComputed
        && let Some(name) = node.name()
        && ets_shim::ast::is_declaration_name(name)
        && !name.flags().intersects(NodeFlags::InWithStatement)
    {
        return c.get_symbol_at_location_exported(name);
    }
    Some(c.get_merged_symbol(symbol))
}

// Rust-only: checker.Checker_getSymbolIfSameReference is not public in tsrs.
// Go: `if c.getResolvedTarget(s1) == c.getResolvedTarget(s2) { return s1 }`, where getResolvedTarget is
// `getMergedSymbol(resolveSymbol(getMergedSymbol(s)))` and resolveSymbol resolves a non-local alias
// (`resolveAlias`, public as `GetAliasedSymbol`). A nil symbol stays nil through all three.
pub fn get_symbol_if_same_reference(
    c: &mut Checker,
    s1: Option<P<Symbol>>,
    s2: Option<P<Symbol>>,
) -> Option<P<Symbol>> {
    if get_resolved_target(c, s1) == get_resolved_target(c, s2) {
        return s1;
    }
    None
}

// Rust-only: Go's `(*Checker).getResolvedTarget` (see `get_symbol_if_same_reference`).
pub(crate) fn get_resolved_target(c: &mut Checker, symbol: Option<P<Symbol>>) -> Option<P<Symbol>> {
    let merged = c.get_merged_symbol(symbol?);
    let resolved = if ets_shim::ast::is_non_local_alias(
        merged,
        SymbolFlags::Value | SymbolFlags::Type | SymbolFlags::Namespace,
    ) {
        c.get_aliased_symbol(merged)
    } else {
        merged
    };
    Some(c.get_merged_symbol(resolved))
}

// Rust-only: Checker.GetTypeArgumentsForResolvedSignature (effect-tsgo patch 003-checker-exports) is not in tsrs.
// Go: nil without a mapper or type parameters, else `instantiateTypes(typeParams, sig.mapper)`. Instantiating a
// type parameter is applying the mapper to it (`instantiateType` -> `getMappedType`), which tsrs's public
// `TypeMapper::map` does.
pub(crate) fn get_type_arguments_for_resolved_signature(
    c: &mut Checker,
    sig: Option<P<Signature>>,
) -> Option<Vec<P<Type>>> {
    let sig = sig?;
    let mapper = sig.mapper.get()?;
    let type_params = match sig.target() {
        Some(target) => target.type_parameters(),
        None => sig.type_parameters(),
    };
    if type_params.is_empty() {
        return None;
    }
    Some(type_params.iter().map(|&t| mapper.map(c, t)).collect())
}

// Rust-only: checker.GetNonDistributedTypeParameter (shim integration.go; Go's getNonDistributedTypeParameter) is
// not public in tsrs: a distributed type parameter's constraint is its declaration binder.
pub(crate) fn get_non_distributed_type_parameter(t: Option<P<Type>>) -> Option<P<Type>> {
    let t = t?;
    if t.flags().intersects(TypeFlags::TypeParameter) && t.as_type_parameter().is_distributed.get()
    {
        return t.as_type_parameter().constraint.get();
    }
    Some(t)
}

// Rust-only: checker.Checker_isNoInferType is not public in tsrs.
// A NoInfer<T> type is represented as a substitution type with a TypeFlags.Unknown constraint.
pub(crate) fn is_no_infer_type(t: P<Type>) -> bool {
    t.flags().intersects(TypeFlags::Substitution)
        && t.as_substitution_type()
            .constraint
            .get()
            .is_some_and(|constraint| constraint.flags().intersects(TypeFlags::Unknown))
}

// Rust-only: checker.Checker_getTypeParameterModifiers is not public in tsrs.
pub(crate) fn get_type_parameter_modifiers(tp: P<Type>) -> ModifierFlags {
    let mut flags = ModifierFlags::None;
    if let Some(symbol) = tp.symbol() {
        for d in symbol.declarations() {
            flags |= d.modifier_flags();
        }
    }
    flags & (ModifierFlags::In | ModifierFlags::Out | ModifierFlags::Const)
}

// Rust-only: checker.Checker_getTypeParameterFromMappedType is not public in tsrs.
// Go: the cached `typeParameter`, else `getDeclaredTypeOfTypeParameter(getSymbolOfDeclaration(declaration.TypeParameter))`
// (public as `GetDeclaredTypeOfSymbol` of the type parameter's symbol), stored in the cache.
pub(crate) fn get_type_parameter_from_mapped_type(c: &mut Checker, t: P<Type>) -> P<Type> {
    let m = t.as_mapped_type();
    if m.type_parameter.get().is_none() {
        let declaration = m.declaration.get().expect("mapped type declaration");
        let symbol = get_symbol_of_declaration(c, declaration.as_mapped_type_node().type_parameter)
            .expect("mapped type parameter symbol");
        m.type_parameter.set(Some(c.get_declared_type_of_symbol_exported(symbol)));
    }
    m.type_parameter.get().expect("set above")
}

// Rust-only: checker.Checker_getConstraintTypeFromMappedType is not public in tsrs.
// Go: the cached `constraintType`, else the constraint of the type parameter or errorType, stored in the cache.
pub(crate) fn get_constraint_type_from_mapped_type(c: &mut Checker, t: P<Type>) -> P<Type> {
    let m = t.as_mapped_type();
    if m.constraint_type.get().is_none() {
        let type_parameter = get_type_parameter_from_mapped_type(c, t);
        let constraint = c
            .get_constraint_of_type_parameter_exported(type_parameter)
            .unwrap_or_else(|| c.get_error_type());
        m.constraint_type.set(Some(constraint));
    }
    m.constraint_type.get().expect("set above")
}

// Rust-only: checker.Checker_getNameTypeFromMappedType is not public in tsrs.
// Go instantiates the `as` clause's type with the mapped type's mapper (`instantiateType`, not public in tsrs) and
// caches it. tsrs's public `MappedType::resolve_components` runs the four component getters of Go (type parameter,
// constraint, name type, template type) in that order; the first two are already cached here, so it computes this
// one and the template type, which Go computes on the next call of `compareMappedType` anyway.
pub(crate) fn get_name_type_from_mapped_type(c: &mut Checker, t: P<Type>) -> Option<P<Type>> {
    let m = t.as_mapped_type();
    m.declaration.get().expect("mapped type declaration").as_mapped_type_node().name_type?;
    if m.name_type().is_none() {
        MappedType::resolve_components(c, t);
    }
    m.name_type()
}

// Rust-only: checker.Checker_getTemplateTypeFromMappedType is not public in tsrs.
// Go: the declared template type with the mapped optionality, instantiated with the mapper, cached; computed
// through `MappedType::resolve_components` as in `get_name_type_from_mapped_type`.
pub(crate) fn get_template_type_from_mapped_type(c: &mut Checker, t: P<Type>) -> P<Type> {
    let m = t.as_mapped_type();
    if m.template_type().is_none() {
        MappedType::resolve_components(c, t);
    }
    m.template_type().expect("resolved above")
}

// Rust-only: checker.IsSourceFileTypeChecked (shim integration.go) is not in tsrs: a state peek at the file's
// `typeChecked` link, which tsrs's `source_file_links` keeps public. It never checks the file.
pub(crate) fn is_source_file_type_checked(c: &Checker, source_file: P<SourceFile>) -> bool {
    c.source_file_links.try_get(source_file).is_some_and(|links| links.type_checked.get())
}

// Rust-only: checker.GetResolvedTypeFromTypeNode (shim integration.go) is not in tsrs: the type already in the
// node's `typeNodeLinks` (`checkExpressionCached`'s cache for an expression, the resolved type of a type node), or
// nil. tsrs keeps the store public; reading it never resolves anything.
pub fn get_resolved_type_from_type_node(c: &Checker, node: P<Node>) -> Option<P<Type>> {
    c.type_node_links.try_get(node)?.resolved_type.get()
}

// Rust-only: checker.Checker_getRegularTypeOfLiteralType is not public in tsrs.
// Go: a freshable literal's `regularType`; for a union, the cached `regularType`, else `mapType(t,
// getRegularTypeOfLiteralType)` stored on the union; any other type unchanged.
pub(crate) fn get_regular_type_of_literal_type(c: &mut Checker, t: P<Type>) -> P<Type> {
    if t.flags().intersects(TypeFlags::Freshable) {
        return t
            .as_literal_type()
            .regular_type
            .get()
            .expect("a freshable literal has a regular type");
    }
    if t.flags().intersects(TypeFlags::Union) {
        let u = t.as_union_type();
        if u.regular_type().is_none() {
            u.set_regular_type(map_type(c, t, &mut |c, t| {
                Some(get_regular_type_of_literal_type(c, t))
            }));
        }
        return u.regular_type().expect("set above");
    }
    t
}

// Rust-only: Go's `(*Checker).mapType` (`mapTypeEx` with reductions), for `get_regular_type_of_literal_type`;
// tsrs's is `pub(crate)`.
fn map_type(
    c: &mut Checker,
    t: P<Type>,
    f: &mut dyn FnMut(&mut Checker, P<Type>) -> Option<P<Type>>,
) -> Option<P<Type>> {
    if t.flags().intersects(TypeFlags::Never) {
        return Some(t);
    }
    if !t.flags().intersects(TypeFlags::Union) {
        return f(c, t);
    }
    let u = t.as_union_type();
    let mut types = u.types.get();
    if let Some(origin) = u.origin()
        && origin.flags().intersects(TypeFlags::Union)
    {
        types = origin.types();
    }
    let mut mapped_types = Vec::new();
    let mut changed = false;
    for &s in types {
        let mapped =
            if s.flags().intersects(TypeFlags::Union) { map_type(c, s, f) } else { f(c, s) };
        if mapped != Some(s) {
            changed = true;
        }
        if let Some(mapped) = mapped {
            mapped_types.push(mapped);
        }
    }
    if !changed {
        return Some(t);
    }
    if mapped_types.is_empty() {
        return None;
    }
    Some(c.get_union_type_ex(&mapped_types, UnionReduction::Literal, AliasArg::None, None))
}
