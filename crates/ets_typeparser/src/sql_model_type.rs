use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/sql_model_type.go:8 sqlModelModuleDescriptor
pub(crate) static SQL_MODEL_MODULE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::SqlModel,
        "@effect/sql",
        Some(&is_sql_model_type_source_file),
    );

// go: internal/typeparser/sql_model_type.go:12 isSqlModelTypeSourceFile
// isSqlModelTypeSourceFile checks if a source file is @effect/sql Model module
// by verifying it exports "Class", "makeRepository", and "makeDataLoaders".
pub(crate) fn is_sql_model_type_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let c = &mut *tp.checker;
    let Some(module_sym) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };

    if c.try_get_member_in_module_exports_and_properties("Class", module_sym).is_none() {
        return false;
    }
    if c.try_get_member_in_module_exports_and_properties("makeRepository", module_sym).is_none() {
        return false;
    }
    if c.try_get_member_in_module_exports_and_properties("makeDataLoaders", module_sym).is_none() {
        return false;
    }

    true
}

impl TypeParser<'_> {
    // go: internal/typeparser/sql_model_type.go:37 TypeParser.IsNodeReferenceToEffectSqlModelModuleApi
    // IsNodeReferenceToEffectSqlModelModuleApi reports whether node resolves to a member
    // exported by the "@effect/sql" package from a module that exports the Model API.
    pub fn is_node_reference_to_effect_sql_model_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(node, &SQL_MODEL_MODULE_DESCRIPTOR, member_name)
    }
}
