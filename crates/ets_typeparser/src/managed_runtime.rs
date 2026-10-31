use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::checker::{ObjectFlags, TypeFlags};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/managed_runtime.go:8 effectManagedRuntimeModuleDescriptor 0af5ddb0d0adaae9
pub(crate) static EFFECT_MANAGED_RUNTIME_MODULE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::ManagedRuntime,
        "effect",
        Some(&is_effect_managed_runtime_source_file),
    );

// go: internal/typeparser/managed_runtime.go:8 effectManagedRuntimeModuleDescriptor 0af5ddb0d0adaae9
// Rust: the descriptor's matcher (a Go func literal). Go's nil checker and source file guards cannot fail here.
fn is_effect_managed_runtime_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let c = &mut *tp.checker;
    let Some(module) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };
    c.try_get_member_in_module_exports_and_properties("ManagedRuntime", module).is_some()
        && c.try_get_member_in_module_exports_and_properties("make", module).is_some()
        && c.try_get_member_in_module_exports_and_properties("isManagedRuntime", module).is_some()
}

impl TypeParser<'_> {
    // go: internal/typeparser/managed_runtime.go:25 TypeParser.IsNodeReferenceToEffectManagedRuntimeMember cf6b2269590d1c43
    // IsNodeReferenceToEffectManagedRuntimeMember reports whether node references
    // the memberName property of Effect's ManagedRuntime interface, as in
    // runtime.runPromise for a runtime built with ManagedRuntime.make.
    pub fn is_node_reference_to_effect_managed_runtime_member(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        let Some(sym) = self.reference_symbol_at_node(node) else {
            return false;
        };
        if sym.name() != member_name {
            return false;
        }
        for &decl in sym.declarations() {
            let Some(parent) = decl.parent() else {
                continue;
            };
            if decl.kind() != Kind::PropertySignature || parent.kind() != Kind::InterfaceDeclaration
            {
                continue;
            }
            let Some(name) = parent.name() else {
                continue;
            };
            if name.kind() != Kind::Identifier || name.text() != "ManagedRuntime" {
                continue;
            }
            if let Some(sf) = ets_shim::ast::get_source_file_of_node(decl)
                && self.source_file_matches_module_cached(
                    sf,
                    &EFFECT_MANAGED_RUNTIME_MODULE_DESCRIPTOR,
                )
            {
                return true;
            }
        }
        false
    }

    // go: internal/typeparser/managed_runtime.go:51 TypeParser.IsExpressionPlainEffectManagedRuntime fdbd86512c4b6b3d
    // IsExpressionPlainEffectManagedRuntime reports whether the non-nullable type
    // of node is a plain instantiation of Effect's ManagedRuntime interface, such
    // as ManagedRuntime<R, ER>. Unions, intersections, mapped types like Pick, and
    // interfaces extending ManagedRuntime do not count, so every ManagedRuntime
    // member is known to be present with its declared type.
    pub fn is_expression_plain_effect_managed_runtime(&mut self, node: P<Node>) -> bool {
        let Some(t) = self.get_type_at_location(node) else {
            return false;
        };
        let t = self.checker.get_non_nullable_type(t);
        if !t.flags().intersects(TypeFlags::Object)
            || !t.object_flags().intersects(ObjectFlags::Reference)
        {
            return false;
        }
        let Some(target) = t.target() else {
            return false;
        };
        let Some(target) = target.symbol() else {
            return false;
        };
        for &decl in target.declarations() {
            if decl.kind() != Kind::InterfaceDeclaration {
                continue;
            }
            let Some(name) = decl.name() else {
                continue;
            };
            if name.kind() != Kind::Identifier || name.text() != "ManagedRuntime" {
                continue;
            }
            if let Some(sf) = ets_shim::ast::get_source_file_of_node(decl)
                && self.source_file_matches_module_cached(
                    sf,
                    &EFFECT_MANAGED_RUNTIME_MODULE_DESCRIPTOR,
                )
            {
                return true;
            }
        }
        false
    }
}
