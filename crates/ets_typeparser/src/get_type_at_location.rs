use std::panic::AssertUnwindSafe;

use ets_go::panics::recover as catch_unwind;

use ets_shim::ast::{Kind, Node};
use ets_shim::checker::{Type, TypeFlags};
use ets_shim::core::P;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/get_type_at_location.go:13 TypeParser.GetTypeAtLocation
    // GetTypeAtLocation wraps checker.GetTypeAtLocation with node-kind and JSX safety guards.
    // It returns nil when the node is nil, not an expression/type-node/declaration,
    // an import clause, a JSX tag name, or a JSX attribute name. It also recovers
    // from checker panics (e.g. nil symbol dereferences on certain declaration
    // nodes) and returns nil.
    pub fn get_type_at_location(&mut self, node: impl Into<Option<P<Node>>>) -> Option<P<Type>> {
        // The guards are syntactic, so rejected nodes never touch the cache.
        let node = node.into()?;
        if !is_type_at_location_candidate(node) {
            return None;
        }

        cached_node(
            self,
            |l| &mut l.type_at_location,
            node,
            |tp| tp.get_type_at_location_uncached(node),
        )
    }

    // go: internal/typeparser/get_type_at_location.go:72 TypeParser.getTypeAtLocationUncached
    pub(crate) fn get_type_at_location_uncached(&mut self, node: P<Node>) -> Option<P<Type>> {
        // Go: a deferred recover turns a panic into nil.
        catch_unwind(AssertUnwindSafe(|| {
            if let Some(t) = self.checked_expression_type(node) {
                return t;
            }
            // The checker treats a never-typed spread as array-like when it checks the
            // enclosing array literal or call, but checking the spread element on its
            // own reports TS2488 as a side effect.
            if node.kind() == Kind::SpreadElement {
                if let Some(t) = self.get_type_at_location(node.expression()) {
                    if t.flags().intersects(TypeFlags::Never) {
                        return t;
                    }
                }
            }
            self.checker.get_type_at_location(node)
        }))
        .ok()
    }
}

// go: internal/typeparser/get_type_at_location.go:26 isTypeAtLocationCandidate
// isTypeAtLocationCandidate reports whether GetTypeAtLocation may ask the
// checker for the type of node.
pub(crate) fn is_type_at_location_candidate(node: P<Node>) -> bool {
    if let Some(parent) = node.parent() {
        if ets_shim::ast::is_jsx_tag_name(node) {
            return false;
        }

        if ets_shim::ast::is_jsx_attribute(parent) && parent.name() == Some(node) {
            return false;
        }
    }

    if !ets_shim::ast::is_expression(node)
        && !ets_shim::ast::is_type_node(node)
        && !ets_shim::ast::is_declaration(node)
    {
        return false;
    }

    // ImportClause passes the IsDeclaration check above, but a clause without a
    // default binding (import { A } from "x", import * as ns from "x") declares
    // no symbol itself, and checker.GetTypeAtLocation panics dereferencing the
    // nil symbol. The clause type is never useful to rules: its bindings are
    // visited as separate nodes and carry the actual types.
    if node.kind() == Kind::ImportClause {
        return false;
    }

    // Tagged templates pass interpolation values directly to the tag function;
    // they do not stringify them. Asking the checker for the type of the inner
    // TemplateExpression forces a normally unreachable checking path that can
    // emit TS2731 for symbol-typed interpolations as a side effect.
    if node.kind() == Kind::TemplateExpression
        && node.parent().is_some_and(ets_shim::ast::is_tagged_template_expression)
    {
        return false;
    }

    // A meta property used as a call callee (import.defer(...)) has no type of
    // its own and the checker debug-asserts when asked (checkMetaProperty); the
    // enclosing call expression carries the meaningful type.
    if node.kind() == Kind::MetaProperty
        && let Some(parent) = node.parent()
        && ets_shim::ast::is_call_expression(parent)
        && parent.expression() == Some(node)
    {
        return false;
    }

    if is_inside_type_only_heritage_expression(node) {
        return false;
    }

    true
}

// go: internal/typeparser/get_type_at_location.go:97 isInsideTypeOnlyHeritageExpression
// isInsideTypeOnlyHeritageExpression reports whether node is an
// ExpressionWithTypeArguments or one of its identifier/property-access
// sub-expressions inside a type-only heritage clause. The checker can
// mis-resolve these as value expressions and emit bogus diagnostics.
pub(crate) fn is_inside_type_only_heritage_expression(node: P<Node>) -> bool {
    if node.kind() == Kind::ExpressionWithTypeArguments {
        return is_type_only_heritage_clause(node.parent());
    }

    if node.kind() != Kind::Identifier && node.kind() != Kind::PropertyAccessExpression {
        return false;
    }

    let mut n = node.parent();
    while let Some(current) = n {
        match current.kind() {
            Kind::PropertyAccessExpression => {}
            Kind::ExpressionWithTypeArguments => {
                return is_type_only_heritage_clause(current.parent());
            }
            _ => return false,
        }
        n = current.parent();
    }

    false
}

// go: internal/typeparser/get_type_at_location.go:120 isTypeOnlyHeritageClause
pub(crate) fn is_type_only_heritage_clause(node: Option<P<Node>>) -> bool {
    let Some(node) = node else {
        return false;
    };
    if !ets_shim::ast::is_heritage_clause(node) {
        return false;
    }

    let heritage_clause = node.as_heritage_clause();
    let Some(container) = node.parent() else {
        return false;
    };

    if container.kind() == Kind::InterfaceDeclaration {
        return true;
    }

    ets_shim::ast::is_class_like(container) && heritage_clause.token == Kind::ImplementsKeyword
}
