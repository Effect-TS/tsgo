use ets_shim::ast::Node;
use ets_shim::checker::{CheckMode, Checker, Type};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/checked_expression_types.go:16 RecordCheckedExpressionType d1118c301f8a473f
// RecordCheckedExpressionType stores the type the checker computed for an
// expression while checking a source file. Only final evaluations count: plain
// Normal mode, or Contextual alone when a call argument is checked against its
// parameter. Inference passes and type-only queries are tentative. The checker
// may evaluate a node more than once, and the last final evaluation is the one
// its diagnostics are based on, so later writes win. Rule runs pause recording,
// and evaluations outside a source-file check are ignored, so only the check
// itself writes the store.
pub fn record_checked_expression_type(
    c: &mut Checker,
    node: P<Node>,
    check_mode: CheckMode,
    t: P<Type>,
) {
    if !(check_mode & !CheckMode::Contextual).is_empty() || !is_checking_source_file(c) {
        return;
    }
    // Rust: borrows the checker's links without cloning their handle (this runs on every checked expression).
    let mut links = effect_links_ref(c).lock();
    if links.checked_expression_types_paused {
        return;
    }
    links.checked_expression_type.insert(node, Some(t));
}

// go: internal/typeparser/checked_expression_types.go:29 PauseCheckedExpressionTypes 1fcc5adecc4e9f1f
// PauseCheckedExpressionTypes stops recording for the checker until the
// returned function is called.
pub fn pause_checked_expression_types(c: &mut Checker) -> impl FnOnce() + use<> {
    let links = effect_links_of(c);
    links.lock().checked_expression_types_paused = true;
    move || links.lock().checked_expression_types_paused = false
}

impl TypeParser<'_> {
    // go: internal/typeparser/checked_expression_types.go:38 TypeParser.checkedExpressionType 2ee86b7861731293
    // checkedExpressionType returns the type the file's check computed for an
    // expression, without evaluating anything, or nil when the check never
    // evaluated it.
    //
    pub(crate) fn checked_expression_type(&mut self, node: P<Node>) -> Option<P<Type>> {
        if !ets_shim::ast::is_expression_node(node) || ets_shim::ast::is_part_of_type_node(node) {
            return None;
        }
        let mut node = node;
        if ets_shim::ast::is_right_side_of_qualified_name_or_property_access(node) {
            node = node.parent()?;
        }
        // A checked file stays checked, so remember the last one found checked. An
        // unchecked file can be checked later and is asked again.
        let sf = ets_shim::ast::get_source_file_of_node(node)?;
        if Some(sf) != self.links().checked_expression_type_source_file {
            if !is_source_file_type_checked(self.checker, sf) {
                return None;
            }
            self.links().checked_expression_type_source_file = Some(sf);
        }
        let mut t = None;
        if let Some(&recorded) = self.links().checked_expression_type.get(&node) {
            t = recorded;
        }
        if t.is_none() {
            // Initializers and return expressions first evaluated while recording
            // was paused are only in the checker's own cache.
            t = get_resolved_type_from_type_node(self.checker, node);
        }
        let t = t?;
        Some(get_regular_type_of_literal_type(self.checker, t))
    }
}
