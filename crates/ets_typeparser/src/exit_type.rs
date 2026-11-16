use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/exit_type.go:8 exitModuleDiscriminatingExport 6bc8fe1a88dce026
const EXIT_MODULE_DISCRIMINATING_EXPORT: &str = "isExit";

// go: internal/typeparser/exit_type.go:10 exitModuleRequiredExports 691aadefbd14488c
static EXIT_MODULE_REQUIRED_EXPORTS: &[&str] =
    &["Exit", EXIT_MODULE_DISCRIMINATING_EXPORT, "isFailure", "isSuccess", "failCause"];

// go: internal/typeparser/exit_type.go:12 effectExitPackageSourceFileDescriptor 95e8d03ca4befece
pub(crate) static EFFECT_EXIT_PACKAGE_SOURCE_FILE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::Exit,
        "effect",
        Some(&is_effect_exit_source_file),
    );

// go: internal/typeparser/exit_type.go:12 effectExitPackageSourceFileDescriptor 95e8d03ca4befece
// Rust: the descriptor's matcher (a Go func literal). Go's nil checker and source file guards cannot fail here.
fn is_effect_exit_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let c = &mut *tp.checker;
    let Some(module_sym) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };

    for name in EXIT_MODULE_REQUIRED_EXPORTS {
        if c.try_get_member_in_module_exports_and_properties(name, module_sym).is_none() {
            return false;
        }
    }
    true
}

impl TypeParser<'_> {
    // go: internal/typeparser/exit_type.go:33 TypeParser.IsNodeReferenceToEffectExitModuleApi 8803411d2e16c36f
    pub fn is_node_reference_to_effect_exit_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &EFFECT_EXIT_PACKAGE_SOURCE_FILE_DESCRIPTOR,
            member_name,
        )
    }
}
