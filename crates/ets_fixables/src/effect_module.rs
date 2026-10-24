use ets_rewriter::Tracker;
use ets_shim::P;
use ets_shim::ast::{Node, NodeFlags, SourceFile};
use ets_shim::core::alloc_str;

// go: internal/fixables/effect_module.go:9 effectModuleMethod
//
// Rust: the factory's `new_identifier` takes `&'static str`; Go's strings are copied into the arena with
// `alloc_str`, as tsrs's own code fixes do.
pub(crate) fn effect_module_method(
    tracker: &Tracker,
    sf: P<SourceFile>,
    receiver: Option<P<Node>>,
    method: &str,
) -> P<Node> {
    let receiver = match receiver {
        None => tracker
            .new_identifier(alloc_str(&ets_typeparser::find_effect_module_identifier(Some(sf)))),
        Some(receiver) => tracker.deep_clone_node(Some(receiver)).unwrap(),
    };
    tracker.new_property_access_expression(
        receiver,
        None,
        tracker.new_identifier(alloc_str(method)),
        NodeFlags::None,
    )
}

// go: internal/fixables/effect_module.go:18 replaceEffectMethodCallee
pub(crate) fn replace_effect_method_callee(
    tracker: &mut Tracker,
    sf: P<SourceFile>,
    callee: Option<P<Node>>,
    name: Option<P<Node>>,
    method: &str,
) {
    if name.is_some() {
        let replacement = tracker.new_identifier(alloc_str(method));
        tracker.replace_node(sf, name, Some(replacement), None);
        return;
    }
    let replacement = effect_module_method(tracker, sf, None, method);
    tracker.replace_node(sf, callee, Some(replacement), None);
}
