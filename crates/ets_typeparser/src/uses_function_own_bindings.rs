use ets_shim::ast::{self, Kind, ModifierList, Node};
use ets_shim::core::P;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/uses_function_own_bindings.go:18 TypeParser.UsesFunctionOwnBindings 6fdd6111522688da
    // UsesFunctionOwnBindings reports whether root, a node inside the function fn,
    // reads a binding that fn itself provides:
    //   - this or super whose this-container (ast.GetThisContainer) is fn;
    //   - new.target whose container (ast.GetNewTargetContainer) is fn;
    //   - fn's implicit arguments object, shorthand { arguments } included;
    //   - fn's own name, when fn is a named function expression.
    //
    // fn's parameters are not included: callers check them with ReferencesToSymbol.
    // An arrow function binds none of these, so it always returns false.
    pub fn uses_function_own_bindings(
        &mut self,
        fn_: impl Into<Option<P<Node>>>,
        root: impl Into<Option<P<Node>>>,
    ) -> bool {
        let (Some(fn_), Some(root)) = (fn_.into(), root.into()) else {
            return false;
        };
        if fn_.kind() == Kind::ArrowFunction {
            return false;
        }
        if fn_.kind() == Kind::FunctionExpression
            && let Some(name) = fn_.name()
        {
            let name_symbol = self.get_symbol_at_location(name);
            if !self.references_to_symbol(root, name_symbol).is_empty() {
                return true;
            }
        }
        let arguments_symbol = self.checker.get_arguments_symbol();
        for reference in self.references_to_symbol(root, arguments_symbol) {
            if ast::get_this_container(reference, false, false) == fn_ {
                return true;
            }
        }
        uses_this_container_keywords(fn_, root)
    }
}

// go: internal/typeparser/uses_function_own_bindings.go:38 usesThisContainerKeywords 17be8a8377e6eaad
// usesThisContainerKeywords reports whether root reads this, super or new.target
// of fn. It skips the parts of nested non-arrow this-containers that belong to
// them; ast.GetThisContainer still decides each keyword it finds.
//
// Rust: Go's recursive `visit` and `visitModifiers` closures are nested functions.
fn uses_this_container_keywords(fn_: P<Node>, root: P<Node>) -> bool {
    fn visit_modifiers(fn_: P<Node>, modifiers: Option<P<ModifierList>>) -> bool {
        modifiers.is_some_and(|modifiers| modifiers.nodes().iter().any(|&node| visit(fn_, node)))
    }
    fn visit(fn_: P<Node>, node: P<Node>) -> bool {
        match node.kind() {
            Kind::ThisKeyword | Kind::SuperKeyword => {
                return ast::get_this_container(node, false, false) == fn_;
            }
            Kind::MetaProperty => {
                return node.as_meta_property().keyword_token() == Kind::NewKeyword
                    && ast::get_new_target_container(node) == Some(fn_);
            }
            Kind::FunctionExpression | Kind::FunctionDeclaration if node != fn_ => {
                // Nothing of a nested function is evaluated in the enclosing scope.
                return false;
            }
            Kind::MethodDeclaration
            | Kind::GetAccessor
            | Kind::SetAccessor
            | Kind::Constructor
            | Kind::PropertyDeclaration
            | Kind::ClassStaticBlockDeclaration
                if node != fn_ =>
            {
                // A member's decorators, its parameters' decorators and its computed
                // name are evaluated in the enclosing scope; the rest belongs to it.
                if visit_modifiers(fn_, node.modifiers()) {
                    return true;
                }
                if let Some(name) = node.name()
                    && name.kind() == Kind::ComputedPropertyName
                    && visit(fn_, name)
                {
                    return true;
                }
                if ast::is_function_like(node) {
                    for &parameter in node.parameters() {
                        if visit_modifiers(fn_, parameter.modifiers()) {
                            return true;
                        }
                    }
                }
                return false;
            }
            _ => {}
        }
        // ets:ast-walk scans for this, super and new.target keywords, which have no symbol to look up
        node.for_each_child(&mut |child| visit(fn_, child))
    }
    visit(fn_, root)
}
