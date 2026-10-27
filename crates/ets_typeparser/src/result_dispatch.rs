use ets_shim::ast::{Kind, Node};
use ets_shim::core::P;

use crate::*;

impl ResultDispatch {
    // go: internal/typeparser/result_dispatch.go:8 ResultDispatch.CommonTagSubject 697bdc50f4736e9d
    // CommonTagSubject returns the common subject of a tag-based dispatch. It
    // returns nil when any branch is not tag-based or when the tag subjects do not
    // refer to the same expression by symbol.
    //
    // Rust: `this` is Go's receiver, which may be nil.
    pub fn common_tag_subject(this: Option<&Self>, tp: &mut TypeParser<'_>) -> Option<P<Node>> {
        let dispatch = this?;
        let first = dispatch.branches.first()?;

        let common = first.condition.tag_subject?;
        first.condition.tag_value?;
        for branch in &dispatch.branches[1..] {
            let condition = branch.condition;
            let (Some(tag_subject), Some(_)) = (condition.tag_subject, condition.tag_value) else {
                return None;
            };
            if !tp.same_result_dispatch_reference(Some(common), Some(tag_subject)) {
                return None;
            }
        }
        Some(common)
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/result_dispatch.go:27 TypeParser.sameResultDispatchReference 7f43bec15f29ce07
    pub(crate) fn same_result_dispatch_reference(
        &mut self,
        left: Option<P<Node>>,
        right: Option<P<Node>>,
    ) -> bool {
        let (Some(left), Some(right)) =
            (unwrap_result_dispatch_expression(left), unwrap_result_dispatch_expression(right))
        else {
            return false;
        };
        if left.kind() != right.kind() {
            return false;
        }
        if left == right {
            return true;
        }

        match left.kind() {
            Kind::Identifier => {
                let left_symbol = self.get_symbol_at_location(left);
                let right_symbol = self.get_symbol_at_location(right);
                same_symbol_reference(self.checker, left_symbol, right_symbol)
            }
            Kind::PropertyAccessExpression => {
                let left_access = left.as_property_access_expression();
                let right_access = right.as_property_access_expression();
                let left_symbol = self.get_symbol_at_location(left);
                let right_symbol = self.get_symbol_at_location(right);
                same_symbol_reference(self.checker, left_symbol, right_symbol)
                    && self.same_result_dispatch_reference(
                        Some(left_access.expression()),
                        Some(right_access.expression()),
                    )
            }
            Kind::ThisKeyword | Kind::SuperKeyword => true,
            _ => false,
        }
    }
}
