use std::sync::Arc;

use ets_shim::ast::{Kind, Node, Symbol, SymbolFlags};
use ets_shim::checker::Type;
use ets_shim::core::P;
use rustc_hash::FxHashMap;

use crate::*;

// go: internal/typeparser/native_array.go:8 nativeArrayNamespace 4832b94011b1a3f9
// go: internal/typeparser/native_array.go:10 nativeArrayInstance 10038c162f683677
// Rust: a newtype over Go's `uint8` (its zero value means "no member"), with the Go constants as associated consts.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub(crate) struct NativeArrayNamespace(pub(crate) u8);

// go: internal/typeparser/native_array.go:10 nativeArrayInstance 10038c162f683677
impl NativeArrayNamespace {
    pub(crate) const Instance: NativeArrayNamespace = NativeArrayNamespace(1);
    pub(crate) const Constructor: NativeArrayNamespace = NativeArrayNamespace(2);
}

// go: internal/typeparser/native_array.go:15 nativeArrayMember b02f7bc1b19e3243
#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub(crate) struct NativeArrayMember {
    namespace: NativeArrayNamespace,
    name: String,
}

// go: internal/typeparser/native_array.go:20 nativeArrayRegistry 078fc8d9c91fb16a
#[derive(Default)]
pub(crate) struct NativeArrayRegistry {
    instance: FxHashMap<String, Vec<P<Symbol>>>,
    constructor: FxHashMap<String, Vec<P<Symbol>>>,
}

impl TypeParser<'_> {
    // go: internal/typeparser/native_array.go:27 TypeParser.IsArrayType 0346be6181525fcf
    // IsArrayType recognizes mutable and readonly arrays, but not tuples.
    // It does not normalize unions or type parameter constraints.
    pub fn is_array_type(&self, t: impl Into<Option<P<Type>>>) -> bool {
        t.into().is_some_and(|t| self.checker.is_array_type(t))
    }

    // go: internal/typeparser/native_array.go:33 TypeParser.IsNativeArrayMethodReference 15d6ab49f8e5a5b3
    // IsNativeArrayMethodReference compares the reference's roots with the named
    // Array or ReadonlyArray member, even when a mapped type renames the access.
    pub fn is_native_array_method_reference(
        &mut self,
        node: impl Into<Option<P<Node>>>,
        method_name: &str,
    ) -> bool {
        self.native_array_reference(
            node.into(),
            &NativeArrayMember {
                namespace: NativeArrayNamespace::Instance,
                name: method_name.to_string(),
            },
        )
    }

    // go: internal/typeparser/native_array.go:39 TypeParser.IsNativeArrayConstructorMethodReference 30be0415b3c3571e
    // IsNativeArrayConstructorMethodReference compares the reference's roots with
    // the named Array constructor member.
    pub fn is_native_array_constructor_method_reference(
        &mut self,
        node: impl Into<Option<P<Node>>>,
        method_name: &str,
    ) -> bool {
        self.native_array_reference(
            node.into(),
            &NativeArrayMember {
                namespace: NativeArrayNamespace::Constructor,
                name: method_name.to_string(),
            },
        )
    }

    // go: internal/typeparser/native_array.go:43 TypeParser.getNativeArrayRegistry 5a76ee261b1b23d6
    pub(crate) fn get_native_array_registry(&mut self) -> Arc<NativeArrayRegistry> {
        if let Some(registry) = &self.links().native_array_registry {
            return Arc::clone(registry);
        }
        let mut r = NativeArrayRegistry::default();
        for name in ["Array", "ReadonlyArray"] {
            if let Some(symbol) =
                self.checker.resolve_name_exported(name, None, SymbolFlags::Type, false)
            {
                let t = self.checker.get_declared_type_of_symbol_exported(symbol);
                add_native_array_members(self, t, &mut r.instance);
            }
        }
        if let Some(symbol) =
            self.checker.resolve_name_exported("Array", None, SymbolFlags::Value, false)
        {
            let t = self.checker.get_type_of_symbol_exported(symbol);
            add_native_array_members(self, t, &mut r.constructor);
        }
        let r = Arc::new(r);
        self.links().native_array_registry = Some(Arc::clone(&r));
        r
    }

    // go: internal/typeparser/native_array.go:85 TypeParser.nativeArrayReference 42feb6c802885134
    pub(crate) fn native_array_reference(
        &mut self,
        node: Option<P<Node>>,
        requested: &NativeArrayMember,
    ) -> bool {
        let Some(node) = node else {
            return false;
        };
        if requested.name.is_empty() {
            return false;
        }
        let Some(symbol) = native_array_reference_symbol(self, node) else {
            return false;
        };
        let member = cached(
            self,
            |l| &mut l.native_array_member,
            symbol,
            |tp| {
                let registry = tp.get_native_array_registry();
                let mut member = NativeArrayMember::default();
                for root in tp.checker.get_root_symbols(symbol) {
                    let mut matched = NativeArrayMember::default();
                    for &canonical in
                        registry.instance.get(root.name()).map_or(&[][..], Vec::as_slice)
                    {
                        if get_symbol_if_same_reference(tp.checker, Some(root), Some(canonical))
                            .is_some()
                        {
                            matched = NativeArrayMember {
                                namespace: NativeArrayNamespace::Instance,
                                name: root.name().to_string(),
                            };
                            break;
                        }
                    }
                    if matched.namespace.0 == 0 {
                        for &canonical in
                            registry.constructor.get(root.name()).map_or(&[][..], Vec::as_slice)
                        {
                            if get_symbol_if_same_reference(tp.checker, Some(root), Some(canonical))
                                .is_some()
                            {
                                matched = NativeArrayMember {
                                    namespace: NativeArrayNamespace::Constructor,
                                    name: root.name().to_string(),
                                };
                                break;
                            }
                        }
                    }
                    if matched.namespace.0 == 0 || member.namespace.0 != 0 && matched != member {
                        return NativeArrayMember::default();
                    }
                    member = matched;
                }
                member
            },
        );
        member == *requested
    }
}

// go: internal/typeparser/native_array.go:43 TypeParser.getNativeArrayRegistry 5a76ee261b1b23d6
// Rust: Go's `add` closure: the members of `t`'s symbol declared only in default library files, with their roots.
// Go ranges over the member table (a map); the result does not depend on the order, as each name is one member.
pub(crate) fn add_native_array_members(
    tp: &mut TypeParser<'_>,
    t: P<Type>,
    members: &mut FxHashMap<String, Vec<P<Symbol>>>,
) {
    let Some(owner) = t.symbol() else {
        return;
    };
    let Some(table) = tp.checker.get_members_of_symbol(owner) else {
        return;
    };
    for symbol in table.values() {
        if !symbol.flags().intersects(SymbolFlags::Value) || symbol.declarations().is_empty() {
            continue;
        }
        let mut library_only = true;
        for &declaration in symbol.declarations() {
            let Some(sf) = ets_shim::ast::get_source_file_of_node(declaration) else {
                library_only = false;
                break;
            };
            if !tp.program.is_source_file_default_library(sf.path()) {
                library_only = false;
                break;
            }
        }
        if library_only {
            let roots = tp.checker.get_root_symbols(symbol);
            members.entry(symbol.name().to_string()).or_default().extend(roots);
        }
    }
}

// go: internal/typeparser/native_array.go:85 TypeParser.nativeArrayReference 42feb6c802885134
// Rust: the symbol lookup at the start of `nativeArrayReference`:
// the referenced member of a property access, or of an element access with a string literal argument.
pub(crate) fn native_array_reference_symbol(
    tp: &mut TypeParser<'_>,
    node: P<Node>,
) -> Option<P<Symbol>> {
    match node.kind() {
        Kind::PropertyAccessExpression => tp.get_symbol_at_location(node),
        Kind::ElementAccessExpression => {
            let access = node.as_element_access_expression();
            let argument_expression = access.argument_expression();
            if !ets_shim::ast::is_string_literal_like(argument_expression) {
                return None;
            }
            let mut symbol = tp.get_symbol_at_location(argument_expression);
            if symbol.is_none() {
                // Go: GetNonNullableType dereferences a nil type (a checker panic).
                let receiver = tp
                    .get_type_at_location(access.expression())
                    .expect("the receiver of an element access has a type");
                let receiver_type = tp.checker.get_non_nullable_type(receiver);
                symbol = tp
                    .checker
                    .get_property_of_type_exported(receiver_type, argument_expression.text());
            }
            symbol
        }
        _ => None,
    }
}
