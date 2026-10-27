use ets_shim::ast::{Kind, Node, SymbolFlags};
use ets_shim::checker::{SignatureKind, Type};
use ets_shim::core::P;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/pipeable.go:10 TypeParser.IsPipeableType d0b478c5cef831b9
    // IsPipeableType returns true if the type has a callable "pipe" property,
    // indicating it supports the pipeable pattern (e.g., value.pipe(f1, f2, ...)).
    pub fn is_pipeable_type(&mut self, t: impl Into<Option<P<Type>>>) -> bool {
        let Some(t) = t.into() else {
            return false;
        };
        cached(
            self,
            |l| &mut l.is_pipeable_type,
            t,
            |tp| {
                let Some(pipe_type) = tp.get_type_of_property_by_name(t, "pipe") else {
                    return false;
                };
                let signatures =
                    tp.checker.get_signatures_of_type_exported(pipe_type, SignatureKind::Call);
                !signatures.is_empty()
            },
        )
    }

    // go: internal/typeparser/pipeable.go:30 TypeParser.LiteralCannotBePipeable f40d4578c0d72fe2
    // An object literal's type has only its own properties plus those of the global
    // Object interface. An array literal's type is an array or tuple whose members
    // come from Array or ReadonlyArray and Object. The global interfaces are read
    // through their declared types so members inherited from augmented base
    // interfaces count.
    pub fn literal_cannot_be_pipeable(&mut self, node: impl Into<Option<P<Node>>>) -> bool {
        let Some(node) = node.into() else {
            return false;
        };
        let node = ets_shim::ast::skip_parentheses(node);
        match node.kind() {
            Kind::ObjectLiteralExpression => {
                for &member in node.as_object_literal_expression().properties().nodes() {
                    let name = member.name();
                    if member.kind() == Kind::SpreadAssignment {
                        return false;
                    }
                    let Some(name) = name else {
                        return false;
                    };
                    if name.kind() == Kind::ComputedPropertyName
                        || ets_shim::ast::get_text_of_property_name(name) == "pipe"
                    {
                        return false;
                    }
                }
                !self.global_interface_has_pipe("Object")
            }
            Kind::ArrayLiteralExpression => {
                !self.global_interface_has_pipe("Object")
                    && !self.global_interface_has_pipe("Array")
                    && !self.global_interface_has_pipe("ReadonlyArray")
            }
            _ => false,
        }
    }

    // go: internal/typeparser/pipeable.go:50 TypeParser.globalInterfaceHasPipe 70067f647ba61c91
    pub(crate) fn global_interface_has_pipe(&mut self, name: &str) -> bool {
        cached(
            self,
            |l| &mut l.global_interface_has_pipe,
            name.to_string(),
            |tp| {
                let Some(symbol) =
                    tp.checker.get_global_symbol_exported(name, SymbolFlags::Type, None)
                else {
                    return false;
                };
                let declared = tp.checker.get_declared_type_of_symbol_exported(symbol);
                tp.checker.get_property_of_type_exported(declared, "pipe").is_some()
            },
        )
    }

    // go: internal/typeparser/pipeable.go:65 TypeParser.IsSafelyPipeableCallee e829f4254f27fd53
    // IsSafelyPipeableCallee returns true if a callee expression can be safely
    // extracted into a pipe argument without losing `this` context.
    // This is used by the missedPipeableOpportunity rule to determine which
    // call expressions can be converted to pipe style.
    pub fn is_safely_pipeable_callee(&mut self, callee: impl Into<Option<P<Node>>>) -> bool {
        let Some(callee) = callee.into() else {
            return false;
        };

        // Call expressions are safe - they return a value
        if ets_shim::ast::is_call_expression(callee) {
            return true;
        }

        // Arrow functions are safe - no `this` binding
        if callee.kind() == Kind::ArrowFunction {
            return true;
        }

        // Function expressions are safe
        if callee.kind() == Kind::FunctionExpression {
            return true;
        }

        // Parenthesized expressions - check inner
        if callee.kind() == Kind::ParenthesizedExpression {
            return self
                .is_safely_pipeable_callee(callee.as_parenthesized_expression().expression());
        }

        // Simple identifiers - check if it's a module/namespace or standalone function
        if ets_shim::ast::is_identifier(callee) {
            let Some(sym) = self.get_symbol_at_location(callee) else {
                return false;
            };

            // Module/namespace imports are safe
            if sym
                .flags()
                .intersects(SymbolFlags::Module | SymbolFlags::Namespace | SymbolFlags::ValueModule)
            {
                return true;
            }

            // Check if the symbol's declaration is a function, variable, or import (not a method)
            if let Some(&decl) = sym.declarations().first() {
                match decl.kind() {
                    Kind::FunctionDeclaration
                    | Kind::VariableDeclaration
                    | Kind::ImportSpecifier
                    | Kind::ImportClause
                    | Kind::NamespaceImport => return true,
                    _ => {}
                }
            }

            return false;
        }

        // Property access - check if subject is a module/namespace
        if ets_shim::ast::is_property_access_expression(callee) {
            let prop_access = callee.as_property_access_expression();
            let subject = prop_access.expression();
            let Some(sym) = self.get_symbol_at_location(subject) else {
                return false;
            };

            // Check if subject is a module/namespace
            if sym
                .flags()
                .intersects(SymbolFlags::Module | SymbolFlags::Namespace | SymbolFlags::ValueModule)
            {
                return true;
            }

            // Check if the symbol's declaration indicates it's a module import
            if let Some(&decl) = sym.declarations().first() {
                match decl.kind() {
                    Kind::NamespaceImport | Kind::SourceFile | Kind::ModuleDeclaration => {
                        return true;
                    }
                    _ => {}
                }
            }

            return false;
        }

        false
    }
}
