// Ported from effect-tsgo's internal/typeparser/service_type_test.go (see helpers_test.rs).

use ets_go::sprintf;
use ets_go::testing::T;
use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::checker::Type;
use ets_shim::core::P;

use crate::*;

// go test: internal/typeparser/service_type_test.go:10 findClassTypeByName afdfa021aa182bc5
// Rust: Go builds a second parser over the checker (`NewTypeParser(c.Program(), c)`), which shares the checker's
// links; the caller's parser is used instead.
pub(crate) fn find_class_type_by_name(
    t: &T,
    tp: &mut TypeParser<'_>,
    sf: P<SourceFile>,
    class_name: &str,
) -> (P<Node>, P<Type>) {
    t.helper();

    fn walk(node: P<Node>, class_name: &str, found_name: &mut Option<P<Node>>) -> bool {
        if found_name.is_some() {
            return false;
        }
        if node.kind() == Kind::ClassDeclaration
            && let Some(name) = node.name()
            && name.text() == class_name
        {
            *found_name = Some(name);
            return false;
        }
        node.for_each_child(&mut |child| walk(child, class_name, found_name));
        false
    }
    let mut found_name = None;
    walk(sf.as_node(), class_name, &mut found_name);
    let Some(found_name) = found_name else {
        t.fatal(sprintf!("class %q not found", class_name));
    };

    let Some(class_sym) = tp.get_symbol_at_location(found_name) else {
        t.fatal(sprintf!("symbol for class %q not found", class_name));
    };
    let Some(class_type) = tp.checker.get_type_of_symbol_at_location(class_sym, Some(found_name))
    else {
        t.fatal(sprintf!("type for class %q not found", class_name));
    };
    (found_name, class_type)
}
