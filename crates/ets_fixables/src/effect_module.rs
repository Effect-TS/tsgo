use ets_rewriter::Tracker;
use ets_shim::P;
use ets_shim::ast::{Node, SourceFile};

// go: internal/fixables/effect_module.go:11 effectModuleMethod
// effectModuleMethod builds <module>.method, where <module> is the reference
// to the Effect module from the file's imports.
pub(crate) fn effect_module_method(tracker: &Tracker, sf: P<SourceFile>, method: &str) -> P<Node> {
    tracker.new_property_access_path(
        &ets_typeparser::find_effect_module_reference(Some(sf)).append(&[method]),
    )
}

// go: internal/fixables/effect_module.go:16 replaceEffectMethodCallee
// replaceEffectMethodCallee replaces the whole callee with <module>.method.
pub(crate) fn replace_effect_method_callee(
    tracker: &mut Tracker,
    sf: P<SourceFile>,
    callee: Option<P<Node>>,
    method: &str,
) {
    let replacement = effect_module_method(tracker, sf, method);
    tracker.replace_node(sf, callee, Some(replacement), None);
}
