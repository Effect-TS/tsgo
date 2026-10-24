//! Rust-only: the effect-tsgo shim accessors the API-stability analysis calls that tsrs does not offer
//! (`_tools/gen_shims/providers/typescript/checker/integration.go`). Each helper is a cached-field peek over public
//! tsrs state, as the Go function is; none resolves or instantiates anything (`docs/TSRS_API.md`, "Gaps").

use ets_shim::ast::{CheckFlags, Kind, Node, Symbol, SymbolFlags, SymbolTable};
use ets_shim::checker::{
    Checker, IndexInfo, ObjectFlags, Signature, SignatureKind, Type, TypeFlags, TypeMapper,
};
use ets_shim::core::P;

use crate::*;

// Rust-only: checker.GetNonPrimitiveType (shim integration.go): the checker's canonical non-primitive object type.
pub(crate) fn get_non_primitive_type(c: &Checker) -> P<Type> {
    c.non_primitive_type
}

// Rust-only: checker.GetMappedTypeConstraintType (shim integration.go): only the already-resolved constraint.
pub(crate) fn get_mapped_type_constraint_type(t: P<Type>) -> Option<P<Type>> {
    t.as_mapped_type().constraint_type.get()
}

// Rust-only: checker.GetMappedTypeNameType (shim integration.go): only the already-resolved remapped key type.
pub(crate) fn get_mapped_type_name_type(t: P<Type>) -> Option<P<Type>> {
    t.as_mapped_type().name_type.get()
}

// Rust-only: checker.GetMappedTypeTemplateType (shim integration.go): only the already-resolved template type.
pub(crate) fn get_mapped_type_template_type(t: P<Type>) -> Option<P<Type>> {
    t.as_mapped_type().template_type.get()
}

// Rust-only: checker.GetResolvedConditionalTypeBranch (shim integration.go): an already-resolved true or false
// branch of a conditional type, or nil; never instantiates or evaluates the conditional.
pub fn get_resolved_conditional_type_branch(
    t: Option<P<Type>>,
    true_branch: bool,
) -> Option<P<Type>> {
    let t = t?;
    if !t.flags().intersects(TypeFlags::Conditional) {
        return None;
    }
    let d = t.as_conditional_type();
    if true_branch {
        return d.resolved_true_type.get();
    }
    d.resolved_false_type.get()
}

// Rust-only: checker.GetConditionalTypeBranchNode (shim integration.go): the declared true or false branch node of
// a conditional type, without reading the branch type.
pub fn get_conditional_type_branch_node(t: Option<P<Type>>, true_branch: bool) -> Option<P<Node>> {
    let t = t?;
    if !t.flags().intersects(TypeFlags::Conditional) {
        return None;
    }
    let d = t.as_conditional_type();
    let root = d.root.get()?;
    let node = root.node.get()?;
    let node = node.as_conditional_type_node();
    if true_branch {
        return Some(node.true_type());
    }
    Some(node.false_type())
}

// Rust-only: checker.GetResolvedTypeArguments (shim integration.go): the type arguments already recorded on a
// reference type, without resolving them; a deferred reference returns nil.
pub(crate) fn get_resolved_type_arguments(t: Option<P<Type>>) -> &'static [P<Type>] {
    let Some(t) = t else {
        return &[];
    };
    if !t.flags().intersects(TypeFlags::Object)
        || !t.object_flags().intersects(ObjectFlags::Reference)
    {
        return &[];
    }
    t.as_type_reference().resolved_type_arguments.get().unwrap_or_default()
}

// Rust-only: checker.GetResolvedTypeOfSymbolIfMaterialized (shim integration.go): the type already computed for a
// value symbol, or nil.
pub fn get_resolved_type_of_symbol_if_materialized(
    c: &Checker,
    symbol: Option<P<Symbol>>,
) -> Option<P<Type>> {
    let symbol = symbol?;
    c.value_symbol_links.try_get(symbol)?.resolved_type.get()
}

// Rust-only: checker.GetResolvedDeclaredTypeOfSymbolIfMaterialized (shim integration.go): the declared type already
// recorded for a symbol, or nil.
pub fn get_resolved_declared_type_of_symbol_if_materialized(
    c: &Checker,
    symbol: Option<P<Symbol>>,
) -> Option<P<Type>> {
    let symbol = symbol?;
    let flags = symbol.flags();
    if flags.intersects(SymbolFlags::Class | SymbolFlags::Interface | SymbolFlags::Alias) {
        if let Some(links) = c.declared_type_links.try_get(symbol) {
            return links.declared_type.get();
        }
    } else if flags.intersects(SymbolFlags::TypeAlias)
        && let Some(links) = c.type_alias_links.try_get(symbol)
    {
        return links.declared_type.get();
    }
    None
}

/// Rust-only: one entry of `get_declared_index_infos_of_symbol`.
///
/// Go builds a transient `*IndexInfo` with
/// `Checker_newIndexInfo` (not public in tsrs); its callers only read the declaration and the key and value types,
/// which may be nil here (an unresolved annotation), so a plain record stands for it.
#[derive(Clone, Copy, Debug)]
pub struct DeclaredIndexInfo {
    pub(crate) key_type: Option<P<Type>>,
    pub(crate) value_type: Option<P<Type>>,
    pub(crate) declaration: Option<P<Node>>,
}

impl DeclaredIndexInfo {
    /// Rust-only: Go's `IndexInfo.KeyType()`.
    pub fn key_type(&self) -> Option<P<Type>> {
        self.key_type
    }

    /// Rust-only: Go's `IndexInfo.ValueType()`.
    pub fn value_type(&self) -> Option<P<Type>> {
        self.value_type
    }

    /// Rust-only: Go's `IndexInfo.Declaration()`.
    pub fn declaration(&self) -> Option<P<Node>> {
        self.declaration
    }
}

// Rust-only: checker.GetDeclaredIndexInfosOfSymbol (shim integration.go): the index infos declared by a symbol
// without resolving its member table or its key and value annotations; a key or value type is returned only when
// the checker already materialized the annotation.
pub fn get_declared_index_infos_of_symbol(
    c: &Checker,
    symbol: Option<P<Symbol>>,
) -> Vec<DeclaredIndexInfo> {
    let Some(symbol) = symbol else {
        return Vec::new();
    };
    let mut infos = Vec::new();
    for &symbol_declaration in symbol.declarations() {
        let mut declarations: Vec<P<Node>> = Vec::new();
        match symbol_declaration.kind() {
            Kind::ClassDeclaration
            | Kind::ClassExpression
            | Kind::InterfaceDeclaration
            | Kind::EnumDeclaration
            | Kind::TypeLiteral
            | Kind::MappedType => {
                declarations.extend_from_slice(symbol_declaration.members());
            }
            _ => {}
        }
        if ets_shim::ast::is_index_signature_declaration(symbol_declaration) {
            declarations.push(symbol_declaration);
        }
        for declaration in declarations {
            if !ets_shim::ast::is_index_signature_declaration(declaration) {
                continue;
            }
            if ets_shim::ast::has_static_modifier(declaration) {
                // Instance members only, matching getIndexSymbol.
                continue;
            }
            let parameters = declaration.parameters();
            if parameters.len() != 1 {
                continue;
            }
            let Some(key_node) = parameters[0].type_node() else {
                continue;
            };
            let key_type = get_resolved_type_from_type_node(c, key_node);
            let value_type =
                declaration.type_node().and_then(|node| get_resolved_type_from_type_node(c, node));
            infos.push(DeclaredIndexInfo { key_type, value_type, declaration: Some(declaration) });
        }
    }
    infos
}

// Rust-only: checker.GetResolvedReturnTypeOfSignatureIfMaterialized (shim integration.go): the return type the
// checker already computed for a signature, or nil.
pub(crate) fn get_resolved_return_type_of_signature_if_materialized(
    signature: Option<P<Signature>>,
) -> Option<P<Type>> {
    signature?.resolved_return_type.get()
}

// Rust-only: checker.GetResolvedConstraintOfTypeParameterIfMaterialized (shim integration.go): the constraint the
// checker already computed for a type parameter, or nil when unresolved or recorded as absent.
pub(crate) fn get_resolved_constraint_of_type_parameter_if_materialized(
    c: &Checker,
    t: Option<P<Type>>,
) -> Option<P<Type>> {
    let t = t?;
    if !t.flags().intersects(TypeFlags::TypeParameter) {
        return None;
    }
    let constraint = t.as_type_parameter().constraint.get()?;
    if constraint == c.no_constraint_type || constraint == c.circular_constraint_type {
        return None;
    }
    Some(constraint)
}

// Rust-only: checker.GetResolvedDefaultFromTypeParameterIfMaterialized (shim integration.go): the default type the
// checker already computed for a type parameter, or nil when unresolved or recorded as absent.
pub(crate) fn get_resolved_default_from_type_parameter_if_materialized(
    c: &Checker,
    t: Option<P<Type>>,
) -> Option<P<Type>> {
    let t = t?;
    if !t.flags().intersects(TypeFlags::TypeParameter) {
        return None;
    }
    let default_type = t.as_type_parameter().resolved_default_type.get()?;
    if default_type == c.no_constraint_type
        || default_type == c.circular_constraint_type
        || default_type == c.resolving_default_type
    {
        return None;
    }
    Some(default_type)
}

// Rust-only: checker.GetResolvedBaseTypesOfTypeIfMaterialized (shim integration.go): the base types the checker
// already resolved for a class or interface; None when inheritance has not been materialized.
pub(crate) fn get_resolved_base_types_of_type_if_materialized(
    t: Option<P<Type>>,
) -> Option<&'static [P<Type>]> {
    let t = t?;
    if !t.object_flags().intersects(ObjectFlags::ClassOrInterface | ObjectFlags::Tuple) {
        return None;
    }
    let data = t.as_interface_type();
    if !data.base_types_resolved.get() {
        return None;
    }
    Some(data.resolved_base_types.get())
}

/// Rust-only: Go's `(*Type).Flags()&TypeFlagsObject != 0 && ObjectFlags()&ObjectFlagsMembersResolved != 0`, the
/// common guard of the `*IfMaterialized` member accessors.
fn members_resolved(t: P<Type>) -> bool {
    t.flags().intersects(TypeFlags::Object)
        && t.object_flags().intersects(ObjectFlags::MembersResolved)
}

// Rust-only: checker.GetResolvedMembersOfTypeIfMaterialized (shim integration.go): the member table the checker
// already resolved for a structured type; None when the member surface was not materialized.
#[expect(
    clippy::option_option,
    reason = "Go's (SymbolTable, bool): a resolved table may itself be nil"
)]
pub(crate) fn get_resolved_members_of_type_if_materialized(
    t: Option<P<Type>>,
) -> Option<Option<P<SymbolTable>>> {
    let t = t?;
    if !members_resolved(t) {
        return None;
    }
    Some(t.as_structured_type().members())
}

// Rust-only: checker.GetResolvedSignaturesOfTypeIfMaterialized (shim integration.go): the call or construct
// signatures the checker already resolved for a structured type; None when not materialized.
pub(crate) fn get_resolved_signatures_of_type_if_materialized(
    t: Option<P<Type>>,
    kind: SignatureKind,
) -> Option<&'static [P<Signature>]> {
    let t = t?;
    if !members_resolved(t) {
        return None;
    }
    let data = t.as_structured_type();
    if kind == SignatureKind::Call {
        return Some(data.call_signatures());
    }
    Some(data.construct_signatures())
}

// Rust-only: checker.GetResolvedIndexInfosOfTypeIfMaterialized (shim integration.go): the index infos the checker
// already computed for a structured type; None when not materialized.
pub(crate) fn get_resolved_index_infos_of_type_if_materialized(
    t: Option<P<Type>>,
) -> Option<&'static [P<IndexInfo>]> {
    let t = t?;
    if !members_resolved(t) {
        return None;
    }
    Some(t.as_structured_type().index_infos())
}

// Rust-only: checker.GetInstantiatedSymbolMapper (shim integration.go): the mapper already recorded on an
// instantiated symbol, or nil.
pub(crate) fn get_instantiated_symbol_mapper(
    c: &Checker,
    symbol: Option<P<Symbol>>,
) -> Option<P<TypeMapper>> {
    let symbol = symbol?;
    if !symbol.check_flags().intersects(CheckFlags::Instantiated) {
        return None;
    }
    c.value_symbol_links.try_get(symbol)?.mapper()
}

// Rust-only: checker.IsCheckingSourceFile (shim integration.go): whether the checker is inside its ordinary
// source-file check lifecycle (Go: `c.ctx != nil`).
pub(crate) fn is_checking_source_file(c: &Checker) -> bool {
    c.ctx.is_some()
}

// Rust-only: checker.IsAliasResolutionFailed (shim integration.go): only an already-cached alias resolution
// failure; never resolves the alias.
pub(crate) fn is_alias_resolution_failed(c: &Checker, symbol: Option<P<Symbol>>) -> bool {
    let Some(symbol) = symbol else {
        return false;
    };
    c.alias_symbol_links
        .try_get(symbol)
        .is_some_and(|links| links.alias_target.get() == Some(c.unknown_symbol))
}
