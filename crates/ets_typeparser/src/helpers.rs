use ets_shim::ast::{Kind, Node, Symbol, SymbolFlags};
use ets_shim::checker::{SignatureKind, Type};
use ets_shim::core::P;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/helpers.go:10 TypeParser.extractCovariantType
    // extractCovariantType gets the type argument from a covariant property.
    // Covariant<A> is encoded as () => A, so we get the return type.
    pub(crate) fn extract_covariant_type(
        &mut self,
        t: Option<P<Type>>,
        prop_name: &str,
    ) -> Option<P<Type>> {
        let prop_type = self.get_type_of_property_by_name(t, prop_name)?;
        let signatures =
            self.checker.get_signatures_of_type_exported(prop_type, SignatureKind::Call);

        if signatures.len() != 1 {
            return None;
        }

        if !signatures[0].type_parameters().is_empty() {
            return None;
        }

        Some(self.checker.get_return_type_of_signature_exported(signatures[0]))
    }

    // go: internal/typeparser/helpers.go:31 TypeParser.extractContravariantType
    // extractContravariantType gets the type argument from a contravariant property.
    // Contravariant<A> is encoded as (_: A) => void, so we get the first parameter type.
    pub(crate) fn extract_contravariant_type(
        &mut self,
        t: Option<P<Type>>,
        prop_name: &str,
    ) -> Option<P<Type>> {
        let prop_type = self.get_type_of_property_by_name(t, prop_name)?;
        let signatures =
            self.checker.get_signatures_of_type_exported(prop_type, SignatureKind::Call);

        if signatures.len() != 1 {
            return None;
        }

        if !signatures[0].type_parameters().is_empty() {
            return None;
        }

        let params = signatures[0].parameters();
        if params.is_empty() {
            return None;
        }

        Some(self.checker.get_type_of_symbol_exported(params[0]))
    }

    // go: internal/typeparser/helpers.go:57 TypeParser.extractInvariantType
    // extractInvariantType gets the type argument from an invariant property.
    // Invariant<A> is encoded as (_: A) => A, so we extract the return type (same as covariant).
    pub(crate) fn extract_invariant_type(
        &mut self,
        t: Option<P<Type>>,
        prop_name: &str,
    ) -> Option<P<Type>> {
        self.extract_covariant_type(t, prop_name)
    }

    // go: internal/typeparser/helpers.go:63 TypeParser.GetTypeOfPropertyByName
    // GetTypeOfPropertyByName returns the type of a property by name.
    // Prefer this when only the property type is needed.
    pub fn get_type_of_property_by_name(
        &mut self,
        t: impl Into<Option<P<Type>>>,
        name: &str,
    ) -> Option<P<Type>> {
        let t = t.into()?;
        self.checker.get_type_of_property_of_type_exported(t, name)
    }

    // go: internal/typeparser/helpers.go:75 TypeParser.GetSymbolAtLocation
    // GetSymbolAtLocation guards meta properties and delete targets.
    // Meta properties (import.meta, import.defer, new.target) never
    // reference a symbol rules care about, and the checker debug-asserts (panics)
    // when asked for `import.defer` used as an import-call callee. Always use
    // this instead of the raw checker call.
    pub fn get_symbol_at_location(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> Option<P<Symbol>> {
        let node = node.into()?;
        if node.kind() == Kind::MetaProperty {
            return None;
        }
        // Symbol lookup can cache an index symbol that makes a later delete check
        // incorrectly require an optional property.
        let mut target = node;
        if let Some(parent) = target.parent()
            && ets_shim::ast::is_property_access_expression(parent)
            && parent.name() == Some(target)
        {
            target = parent;
        }
        if ets_shim::ast::is_access_expression(target) {
            let parent = ets_shim::ast::walk_up_parenthesized_expressions(target.parent());
            if let Some(parent) = parent
                && parent.kind() == Kind::DeleteExpression
            {
                return None;
            }
        }
        self.checker.get_symbol_at_location_exported(node)
    }

    // go: internal/typeparser/helpers.go:97 TypeParser.resolveAliasedSymbol
    pub(crate) fn resolve_aliased_symbol(&mut self, sym: Option<P<Symbol>>) -> Option<P<Symbol>> {
        let mut sym = sym;
        while let Some(s) = sym
            && s.flags().intersects(SymbolFlags::Alias)
        {
            sym = Some(self.checker.get_aliased_symbol(s));
        }
        sym
    }

    // go: internal/typeparser/helpers.go:110 TypeParser.ResolveToGlobalSymbol
    // ResolveToGlobalSymbol follows aliases and up to two simple variable indirections
    // so rules can recognize references to the original global symbol.
    pub fn resolve_to_global_symbol(
        &mut self,
        sym: impl Into<Option<P<Symbol>>>,
    ) -> Option<P<Symbol>> {
        let sym = sym.into()?;

        let mut sym = self.resolve_aliased_symbol(Some(sym));
        let mut depth = 0;
        while depth < 2
            && let Some(s) = sym
            && let Some(value_declaration) = s.value_declaration()
            && value_declaration.kind() == Kind::VariableDeclaration
        {
            let decl = value_declaration.as_variable_declaration();
            let Some(initializer) = decl.initializer() else {
                break;
            };

            let Some(next) = self.get_symbol_at_location(initializer) else {
                break;
            };
            let next = self.resolve_aliased_symbol(Some(next));
            if next == sym {
                break;
            }

            sym = next;
            depth += 1;
        }

        sym
    }

    // go: internal/typeparser/helpers.go:143 TypeParser.IsNodeReferenceToGlobalMember
    // IsNodeReferenceToGlobalMember reports whether node resolves to a property of
    // a global value. ReferenceSymbolAtNode follows import and const aliases, so
    // callers do not need to constrain the source expression to property-access
    // syntax merely to recognize the global API.
    pub fn is_node_reference_to_global_member(
        &mut self,
        node: impl Into<Option<P<Node>>>,
        global_name: &str,
        member_name: &str,
    ) -> bool {
        let Some(node) = node.into() else {
            return false;
        };
        let Some(global) =
            self.checker.resolve_name_exported(global_name, None, SymbolFlags::Value, false)
        else {
            return false;
        };
        let Some(global_type) = self.checker.get_type_of_symbol_at_location(global, Some(node))
        else {
            return false;
        };
        let member = self.checker.get_property_of_type_exported(global_type, member_name);
        let actual = self.reference_symbol_at_node(node);
        member.is_some()
            && actual.is_some()
            && get_symbol_if_same_reference(self.checker, member, actual).is_some()
    }
}
