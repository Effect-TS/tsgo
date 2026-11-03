use ets_shim::checker::{Type, TypeFlags};
use ets_shim::core::P;

use crate::either_type::EFFECT_EITHER_PACKAGE_SOURCE_FILE_DESCRIPTOR;
use crate::exit_type::EFFECT_EXIT_PACKAGE_SOURCE_FILE_DESCRIPTOR;
use crate::option_type::EFFECT_OPTION_PACKAGE_SOURCE_FILE_DESCRIPTOR;
use crate::result_type::EFFECT_RESULT_PACKAGE_SOURCE_FILE_DESCRIPTOR;
use crate::*;

// go: internal/typeparser/tagged_data_type.go:9 taggedDataTypeModules a0911067a08ed5cf
// taggedDataTypeModules are the Effect modules whose data type is a union of
// tagged members, each with an is<Tag> guard, such as Option.isSome.
//
// Rust: Go's anonymous struct is a (name, descriptor) pair.
pub(crate) static TAGGED_DATA_TYPE_MODULES: [(&str, &PackageSourceFileDescriptor); 4] = [
    ("Option", &EFFECT_OPTION_PACKAGE_SOURCE_FILE_DESCRIPTOR),
    ("Exit", &EFFECT_EXIT_PACKAGE_SOURCE_FILE_DESCRIPTOR),
    ("Result", &EFFECT_RESULT_PACKAGE_SOURCE_FILE_DESCRIPTOR),
    ("Either", &EFFECT_EITHER_PACKAGE_SOURCE_FILE_DESCRIPTOR),
];

impl TypeParser<'_> {
    // go: internal/typeparser/tagged_data_type.go:21 TypeParser.TaggedDataTypeModule 9a23c1733ec94d7c
    // TaggedDataTypeModule returns "Option", "Exit", "Result" or "Either" when
    // that Effect module declares every member of the union t, and "" otherwise.
    pub fn tagged_data_type_module(&mut self, t: Option<P<Type>>) -> &'static str {
        let Some(t) = t else {
            return "";
        };
        if !t.flags().intersects(TypeFlags::Union) {
            return "";
        }
        for (name, descriptor) in TAGGED_DATA_TYPE_MODULES {
            let mut declared = true;
            for &member in t.types() {
                let declared_in_module = member.symbol().is_some_and(|symbol| {
                    self.symbol_has_declaration_in_module_cached(symbol, descriptor)
                });
                if !declared_in_module {
                    declared = false;
                    break;
                }
            }
            if declared {
                return name;
            }
        }
        ""
    }
}
