use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/effect_model_type.go:8 effectModelPackageSourceFileDescriptor bc7e19c1896fd89c
pub(crate) static EFFECT_MODEL_PACKAGE_SOURCE_FILE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::EffectModel,
        "effect",
        Some(&is_effect_model_type_source_file),
    );

// go: internal/typeparser/effect_model_type.go:14 isEffectModelTypeSourceFile 7f0443896c10eeaa
// isEffectModelTypeSourceFile checks if a source file is the effect/schema Model module
// by verifying it exports "Class", "Generated", and "FieldOption".
// These symbols are chosen to disambiguate Model from Schema (which also exports "Class"),
// matching the TypeScript reference implementation.
pub(crate) fn is_effect_model_type_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let c = &mut *tp.checker;
    let Some(module_sym) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };

    if c.try_get_member_in_module_exports_and_properties("Class", module_sym).is_none() {
        return false;
    }
    // Generated was split into GeneratedByDb / GeneratedByApp in newer v4 betas.
    if c.try_get_member_in_module_exports_and_properties("Generated", module_sym).is_none()
        && c.try_get_member_in_module_exports_and_properties("GeneratedByDb", module_sym).is_none()
        && c.try_get_member_in_module_exports_and_properties("GeneratedByApp", module_sym).is_none()
    {
        return false;
    }
    // FieldOption is unique to v4 Model
    if c.try_get_member_in_module_exports_and_properties("FieldOption", module_sym).is_none() {
        return false;
    }

    true
}

impl TypeParser<'_> {
    // go: internal/typeparser/effect_model_type.go:44 TypeParser.IsNodeReferenceToEffectModelModuleApi 8bb5c502fd939c5b
    // IsNodeReferenceToEffectModelModuleApi reports whether node resolves to a member
    // exported by the "effect" package from a module that exports the Model API
    // (effect/schema).
    pub fn is_node_reference_to_effect_model_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &EFFECT_MODEL_PACKAGE_SOURCE_FILE_DESCRIPTOR,
            member_name,
        )
    }
}
