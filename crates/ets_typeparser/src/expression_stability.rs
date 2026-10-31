use ets_shim::ast::{Kind, Node, NodeFlags, Symbol, SymbolFlags};
use ets_shim::core::P;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/expression_stability.go:10 TypeParser.IsExpressionValueStableAtLocation a8d28de85fcae8d7
    // IsExpressionValueStableAtLocation reports whether evaluating expression at
    // location produces the same value as evaluating it at its original site.
    pub fn is_expression_value_stable_at_location(
        &mut self,
        expression: impl Into<Option<P<Node>>>,
        location: impl Into<Option<P<Node>>>,
    ) -> bool {
        let (Some(expression), Some(location)) = (expression.into(), location.into()) else {
            return false;
        };

        let expression = ets_shim::ast::skip_parentheses(expression);

        match expression.kind() {
            Kind::StringLiteral
            | Kind::NumericLiteral
            | Kind::BigIntLiteral
            | Kind::NoSubstitutionTemplateLiteral
            | Kind::TrueKeyword
            | Kind::FalseKeyword
            | Kind::NullKeyword => true,
            Kind::PrefixUnaryExpression => {
                // -1 and -1n are signed literals; no other unary forms are accepted.
                // Rust: Go's `unary.Operand != nil` always holds (the operand is not optional).
                let unary = expression.as_prefix_unary_expression();
                unary.operator() == Kind::MinusToken
                    && (unary.operand().kind() == Kind::NumericLiteral
                        || unary.operand().kind() == Kind::BigIntLiteral)
            }
            Kind::PropertyAccessExpression => {
                // Enum members cannot be reassigned; the receiver must be the enum itself.
                let access = expression.as_property_access_expression();
                if !ets_shim::ast::is_entity_name_expression(access.expression()) {
                    return false;
                }
                let enum_symbol = self.get_symbol_at_location(access.expression());
                let member = self.get_symbol_at_location(access.name());
                let (Some(enum_symbol), Some(member)) = (enum_symbol, member) else {
                    return false;
                };
                if !enum_symbol.flags().intersects(SymbolFlags::Enum)
                    || !member.flags().intersects(SymbolFlags::EnumMember)
                {
                    return false;
                }
                let Some(declaration) = member.value_declaration() else {
                    return false;
                };
                ets_shim::ast::is_enum_member(declaration)
                    && self.is_declared_before_location(declaration.parent(), location)
            }
            Kind::Identifier => {
                let symbol = self.get_symbol_at_location(expression);
                if expression.text() == "undefined" {
                    return symbol.is_some()
                        && symbol
                            == self.checker.get_global_symbol_exported(
                                "undefined",
                                SymbolFlags::Value,
                                None,
                            );
                }
                let Some(symbol) = symbol else {
                    return false;
                };
                if !self.checker.is_constant_variable(symbol) {
                    return false;
                }
                let Some(declaration) = symbol.value_declaration() else {
                    return false;
                };
                if !ets_shim::ast::is_variable_declaration(declaration) {
                    return false;
                }
                // A `declare const` has no initializer, but its value cannot be reassigned.
                if declaration.initializer().is_none()
                    && !declaration.flags().intersects(NodeFlags::Ambient)
                {
                    return false;
                }
                self.is_declared_before_location(Some(declaration), location)
            }
            _ => false,
        }
    }

    // go: internal/typeparser/expression_stability.go:75 TypeParser.isDeclaredBeforeLocation a77d7353b1800f30
    // isDeclaredBeforeLocation reports whether declaration's value already exists
    // when location is evaluated. The checker alone accepts later declarations used
    // from deferred function bodies, so the positional check is kept alongside it.
    pub(crate) fn is_declared_before_location(
        &mut self,
        declaration: Option<P<Node>>,
        location: P<Node>,
    ) -> bool {
        declaration.is_some_and(|declaration| {
            ets_shim::ast::get_source_file_of_node(declaration)
                == ets_shim::ast::get_source_file_of_node(location)
                && declaration.end() <= location.pos()
                && self.checker.is_block_scoped_name_declared_before_use(declaration, location)
        })
    }

    // go: internal/typeparser/expression_stability.go:87 TypeParser.IsExpressionEvaluableAtLocation 365647b28ff4a0d0
    // IsExpressionEvaluableAtLocation reports whether evaluating expression once at
    // location, instead of at its own deferred site, reads the same bindings. Reads
    // inside nested function bodies still happen when those functions run, so only
    // the eagerly evaluated part of expression is checked: it must not read a
    // mutable binding or a binding that is still uninitialized at location.
    pub fn is_expression_evaluable_at_location(
        &mut self,
        expression: impl Into<Option<P<Node>>>,
        location: impl Into<Option<P<Node>>>,
    ) -> bool {
        let (Some(expression), Some(location)) = (expression.into(), location.into()) else {
            return false;
        };

        let mut evaluable = true;
        // Rust: Go's recursive `visit` closure is a nested function.
        fn visit(
            tp: &mut TypeParser<'_>,
            expression: P<Node>,
            location: P<Node>,
            evaluable: &mut bool,
            node: P<Node>,
        ) -> bool {
            if ets_shim::ast::is_part_of_type_node(node) {
                return false;
            }
            if ets_shim::ast::is_function_like_declaration(node) {
                // The body runs later; a computed name is evaluated with the enclosing expression.
                if let Some(name) = node.name()
                    && ets_shim::ast::is_computed_property_name(name)
                {
                    visit(tp, expression, location, evaluable, name);
                }
                return !*evaluable;
            }
            if node.kind() == Kind::Identifier {
                if let Some(symbol) = tp.evaluated_reference_symbol(node)
                    && !tp.is_binding_evaluable_at_location(symbol, expression, location)
                {
                    *evaluable = false;
                }
                return !*evaluable;
            }
            node.for_each_child(&mut |child| visit(tp, expression, location, evaluable, child));
            !*evaluable
        }
        visit(self, expression, location, &mut evaluable, expression);
        evaluable
    }

    // go: internal/typeparser/expression_stability.go:122 TypeParser.evaluatedReferenceSymbol 5b5f32eba40bce68
    // evaluatedReferenceSymbol resolves an identifier that reads a binding. The
    // second result is false for identifiers that name a declaration, property or
    // label instead of reading a value.
    //
    // Rust: Go's `(symbol, ok)` is `Option<Option<P<Symbol>>>`: `None` for Go's false, `Some(None)` for an
    // unresolved read.
    #[expect(
        clippy::option_option,
        reason = "Go's `(symbol, ok)` has an unresolved symbol with ok true"
    )]
    pub(crate) fn evaluated_reference_symbol(
        &mut self,
        node: P<Node>,
    ) -> Option<Option<P<Symbol>>> {
        let parent = node.parent();
        if let Some(parent) = parent
            && ets_shim::ast::is_shorthand_property_assignment(parent)
            && parent.name() == Some(node)
        {
            return Some(self.checker.get_shorthand_assignment_value_symbol(Some(parent)));
        }
        if ets_shim::ast::is_declaration_name_or_import_property_name(node)
            || ets_shim::ast::is_label_name(node)
        {
            return None;
        }
        if let Some(parent) = parent {
            match parent.kind() {
                Kind::PropertyAccessExpression => {
                    if parent.name() == Some(node) {
                        return None;
                    }
                }
                Kind::QualifiedName => {
                    if parent.as_qualified_name().right() == node {
                        return None;
                    }
                }
                Kind::PropertyAssignment | Kind::BindingElement => {
                    if parent.name() == Some(node) || parent.property_name() == Some(node) {
                        return None;
                    }
                }
                Kind::MetaProperty => return None,
                _ => {}
            }
        }
        Some(self.get_symbol_at_location(node))
    }

    // go: internal/typeparser/expression_stability.go:153 TypeParser.isBindingEvaluableAtLocation 17b77cb6bf7f98a1
    // isBindingEvaluableAtLocation reports whether reading symbol at location
    // observes the value it holds when read from within expression.
    pub(crate) fn is_binding_evaluable_at_location(
        &mut self,
        symbol: Option<P<Symbol>>,
        expression: P<Node>,
        location: P<Node>,
    ) -> bool {
        let Some(symbol) = symbol else {
            return false;
        };
        if symbol.flags().intersects(SymbolFlags::Alias) {
            return true;
        }

        let source_file = ets_shim::ast::get_source_file_of_node(location);
        let mut declarations = Vec::new();
        for declaration in symbol.declarations() {
            if ets_shim::ast::get_source_file_of_node(*declaration) == source_file {
                declarations.push(*declaration);
            }
        }
        // Globals from lib and other files are initialized before this module runs.
        if declarations.is_empty() {
            return true;
        }

        for declaration in declarations {
            if ets_shim::ast::is_node_descendant_of(declaration, expression) {
                continue;
            }
            let root = ets_shim::ast::get_root_declaration(declaration);
            if root.kind() == Kind::Parameter
                || ets_shim::ast::is_variable_declaration(root)
                    && root.parent().is_some_and(|parent| parent.kind() == Kind::CatchClause)
            {
                if self.checker.is_symbol_assigned(symbol) {
                    return false;
                }
            } else if ets_shim::ast::is_variable_declaration(root) {
                if !ets_shim::ast::is_var_const(root)
                    || !self.is_declared_before_location(Some(root), location)
                {
                    return false;
                }
            } else if root.kind() == Kind::FunctionDeclaration {
                // Function declarations are hoisted with their value.
            } else if !self.is_declared_before_location(Some(root), location) {
                return false;
            }
        }
        true
    }
}
