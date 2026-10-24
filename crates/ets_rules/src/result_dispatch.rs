use ets_shim::P;
use ets_shim::ast::{self, Kind, Node, Symbol};
use ets_typeparser::{DispatchCondition, TypeParser};

use crate::*;

// go: internal/rules/result_dispatch.go:9 resultDispatchTagValue
pub(crate) fn result_dispatch_tag_value(condition: DispatchCondition) -> Option<String> {
    let value = unwrap_transparent_expression(condition.tag_value());
    if condition.tag_subject().is_some()
        && let Some(value) = value
        && ast::is_string_literal(value)
    {
        return Some(value.as_string_literal().text().to_string());
    }
    None
}

// go: internal/rules/result_dispatch.go:17 isResultDispatchTagReference
//
// Rust: Go's checker argument is the parser's checker (`tp.checker()`).
pub(crate) fn is_result_dispatch_tag_reference(
    tp: &mut TypeParser<'_>,
    tag_subject: Option<P<Node>>,
    root_symbol: Option<P<Symbol>>,
) -> bool {
    let tag_subject = unwrap_transparent_expression(tag_subject);
    let (Some(tag_subject), Some(root_symbol)) = (tag_subject, root_symbol) else {
        return false;
    };

    let mut root = Some(tag_subject);
    loop {
        root = unwrap_transparent_expression(root);
        let Some(current) = root else {
            break;
        };
        if current.kind() != Kind::PropertyAccessExpression {
            break;
        }
        // Rust: a property access always has an expression (Go's nil checks drop).
        root = Some(current.as_property_access_expression().expression());
    }
    let Some(root) = root else {
        return false;
    };
    if !same_catch_reason_symbol(tp, Some(root), Some(root_symbol)) {
        return false;
    }

    let Some(owner_type) = tp.get_type_at_location(tag_subject) else {
        return false;
    };
    if tp.checker().get_property_of_type_exported(owner_type, "_tag").is_some() {
        return true;
    }
    for member in tp.unroll_union_members(owner_type).unwrap_or_default() {
        if tp.checker().get_property_of_type_exported(member, "_tag").is_some() {
            return true;
        }
    }
    false
}
