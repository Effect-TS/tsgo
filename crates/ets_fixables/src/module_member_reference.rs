use std::cmp::Ordering;

use ets_fixable::Context;
use ets_rewriter::Tracker;
use ets_shim::P;
use ets_shim::ast::{self, Kind, Node, NodeFlags, SymbolFlags};
use ets_shim::core::alloc_str;
use ets_typeparser::TypeParser;

/// Rust-only: Go's `func(node *ast.Node) bool` closures over `ctx.TypeParser`; the parser is passed in, since it
/// cannot be captured next to the context that owns it.
pub(crate) type ModuleMemberPredicate = Box<dyn Fn(&mut TypeParser<'_>, P<Node>) -> bool>;

// go: internal/fixables/module_member_reference.go:13 moduleMember
// moduleMember describes a module export that a fix wants to reference.
pub(crate) struct ModuleMember {
    pub(crate) name: &'static str,
    pub(crate) is_module: ModuleMemberPredicate, // node references the module namespace
    pub(crate) is_member: ModuleMemberPredicate, // node references the export itself
}

// go: internal/fixables/module_member_reference.go:20 moduleMemberReference
// moduleMemberReference is a binding, visible at a location, that reaches a moduleMember.
#[derive(Clone, Copy)]
pub(crate) struct ModuleMemberReference {
    node: P<Node>,
    via_module: bool, // node is the module namespace, so the member is accessed on it
}

// go: internal/fixables/module_member_reference.go:34 resolveModuleMember
// resolveModuleMember finds a binding that resolves at location to member: the
// receiver when it is the module, otherwise an import binding in scope, trying
// preferred first. It returns nil when nothing reaches the export, so the fix
// never introduces an unbound, shadowed or uninitialized name.
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
    receiver: Option<P<Node>>,
    preferred: &str,
    member: &ModuleMember,
) -> Option<ModuleMemberReference> {
    if let Some(receiver) = receiver
        && (member.is_module)(&mut ctx.type_parser, receiver)
        && module_has_member(ctx, receiver, member)
    {
        return Some(ModuleMemberReference { node: receiver, via_module: true });
    }
    let mut symbols = ctx
        .checker()
        .get_symbols_in_scope_exported(location, SymbolFlags::Value | SymbolFlags::Alias);
    symbols.sort_by(|left, right| {
        let (left_name, right_name) = (left.name(), right.name());
        if (left_name == preferred) != (right_name == preferred) {
            if left_name == preferred {
                return Ordering::Less;
            }
            return Ordering::Greater;
        }
        left_name.cmp(right_name)
    });
    for symbol in symbols {
        if symbol.declarations().len() != 1 {
            continue;
        }
        let declaration = symbol.declarations()[0];
        if declaration.kind() != Kind::ImportSpecifier
            && declaration.kind() != Kind::NamespaceImport
            || ast::is_part_of_type_only_import_or_export_declaration(declaration)
        {
            continue;
        }
        let name = declaration.name();
        // The generated identifier must resolve to this import at location, not
        // to a local that shadows it.
        let Some(name) = name else {
            continue;
        };
        if name.kind() != Kind::Identifier
            || ctx.checker().resolve_name_exported(
                name.text(),
                location,
                SymbolFlags::Value | SymbolFlags::Alias,
                false,
            ) != Some(symbol)
        {
            continue;
        }
        if (member.is_member)(&mut ctx.type_parser, name) {
            return Some(ModuleMemberReference { node: name, via_module: false });
        }
        if (member.is_module)(&mut ctx.type_parser, name) && module_has_member(ctx, name, member) {
            return Some(ModuleMemberReference { node: name, via_module: true });
        }
    }
    None
}

// go: internal/fixables/module_member_reference.go:75 moduleHasMember
//
// Rust: Go reads a property of the receiver's type, which `GetTypeAtLocation` always finds for these expressions;
// a missing type is no property.
pub(crate) fn module_has_member(
    ctx: &mut Context<'_, '_>,
    receiver: P<Node>,
    member: &ModuleMember,
) -> bool {
    let Some(receiver_type) = ctx.type_parser.get_type_at_location(receiver) else {
        return false;
    };
    let property = ctx.checker().get_property_of_type_exported(receiver_type, member.name);
    let Some(property) = property else {
        return false;
    };
    let Some(declaration) = property.value_declaration() else {
        return false;
    };
    declaration.name().is_some_and(|name| (member.is_member)(&mut ctx.type_parser, name))
}

impl ModuleMemberReference {
    // go: internal/fixables/module_member_reference.go:85 moduleMemberReference.expression
    // expression builds the reference: module.member, or the direct binding.
    pub(crate) fn expression(self, tracker: &Tracker, member: &ModuleMember) -> P<Node> {
        if !self.via_module {
            return tracker.new_identifier(alloc_str(self.node.text()));
        }
        tracker.new_property_access_expression(
            tracker.deep_clone_node(Some(self.node)).unwrap(),
            None,
            tracker.new_identifier(member.name),
            NodeFlags::None,
        )
    }
}
