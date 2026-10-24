use ets_shim::ast::{Node, SourceFile, SymbolFlags};
use ets_shim::checker::Type;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/schema_type.go:8 effectSchemaModuleDescriptor
pub(crate) static EFFECT_SCHEMA_MODULE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::Schema,
        "effect",
        Some(&is_schema_type_source_file),
    );

// go: internal/typeparser/schema_type.go:10 effectParseResultModuleDescriptor
pub(crate) static EFFECT_PARSE_RESULT_MODULE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::ParseResult,
        "effect",
        Some(&is_parse_result_source_file),
    );

// go: internal/typeparser/schema_type.go:12 effectSchemaParserModuleDescriptor
pub(crate) static EFFECT_SCHEMA_PARSER_MODULE_DESCRIPTOR: PackageSourceFileDescriptor =
    new_package_source_file_descriptor(
        ModuleDescriptorId::SchemaParser,
        "effect",
        Some(&is_schema_parser_source_file),
    );

// go: internal/typeparser/schema_type.go:15 SchemaTypeId
// SchemaTypeId is the property key for Schema's variance struct.
pub const SCHEMA_TYPE_ID: &str = "~effect/Schema/Schema";

impl TypeParser<'_> {
    // go: internal/typeparser/schema_type.go:18 TypeParser.IsSchemaType
    // IsSchemaType returns true if the type is a Schema type (v4 or v3).
    pub fn is_schema_type(&mut self, t: impl Into<Option<P<Type>>>) -> bool {
        self.effect_schema_types(t).is_some()
    }
}

// go: internal/typeparser/schema_type.go:26 SchemaTypes
// SchemaTypes holds the A (Type) and E (Encoded) types extracted from a Schema type.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SchemaTypes {
    pub(crate) a: P<Type>,
    pub(crate) e: P<Type>,
}

impl SchemaTypes {
    // go: internal/typeparser/schema_type.go:31 SchemaTypes.A
    pub fn a(self) -> P<Type> {
        self.a
    }

    // go: internal/typeparser/schema_type.go:33 SchemaTypes.E
    pub fn e(self) -> P<Type> {
        self.e
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/schema_type.go:37 TypeParser.EffectSchemaTypes
    // EffectSchemaTypes extracts the A (Type) and E (Encoded) types from a Schema type.
    // Returns nil if the type is not a recognized Schema type or types cannot be extracted.
    pub fn effect_schema_types(&mut self, t: impl Into<Option<P<Type>>>) -> Option<SchemaTypes> {
        let t = t.into()?;
        cached(
            self,
            |l| &mut l.effect_schema_types,
            t,
            |tp| {
                let version = tp.detect_effect_version();
                if version == EffectMajorVersion::V4 {
                    tp.get_type_of_property_by_name(t, SCHEMA_TYPE_ID)?;
                    // V4: get Type and Encoded properties directly
                    let a_type = tp.get_type_of_property_by_name(t, "Type");
                    let e_type = tp.get_type_of_property_by_name(t, "Encoded");
                    let (Some(a), Some(e)) = (a_type, e_type) else {
                        return None;
                    };
                    return Some(SchemaTypes { a, e });
                }

                // V3: check for 'ast' property first
                tp.checker.get_property_of_type_exported(t, "ast")?;

                // Find the variance struct property and extract A/I types
                let props = tp.checker.get_properties_of_type_exported(t);
                for &prop in props {
                    if !prop.flags().intersects(SymbolFlags::Property)
                        || prop.flags().intersects(SymbolFlags::Optional)
                        || prop.value_declaration().is_none()
                    {
                        continue;
                    }
                    let prop_type = tp.checker.get_type_of_symbol_at_location(prop, None);
                    let Some(a) = tp.extract_invariant_type(prop_type, "_A") else {
                        continue;
                    };
                    let Some(i) = tp.extract_invariant_type(prop_type, "_I") else {
                        continue;
                    };
                    if tp.extract_covariant_type(prop_type, "_R").is_none() {
                        continue;
                    }
                    return Some(SchemaTypes { a, e: i });
                }

                None
            },
        )
    }
}

// go: internal/typeparser/schema_type.go:88 isSchemaTypeSourceFile
// Rust: Go's `c` argument is `tp.checker`.
pub(crate) fn is_schema_type_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let Some(module_sym) = get_symbol_of_declaration(tp.checker, sf.as_node()) else {
        return false;
    };

    let Some(schema_sym) =
        tp.checker.try_get_member_in_module_exports_and_properties("Schema", module_sym)
    else {
        return false;
    };

    let schema_type = tp.checker.get_declared_type_of_symbol_exported(schema_sym);

    tp.is_schema_type(schema_type)
}

impl TypeParser<'_> {
    // go: internal/typeparser/schema_type.go:113 TypeParser.IsNodeReferenceToEffectSchemaModuleApi
    // IsNodeReferenceToEffectSchemaModuleApi reports whether node resolves to a member
    // exported by the "effect" package from a module that exports the Schema type.
    pub fn is_node_reference_to_effect_schema_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(node, &EFFECT_SCHEMA_MODULE_DESCRIPTOR, member_name)
    }
}

// go: internal/typeparser/schema_type.go:117 isParseResultSourceFile
pub(crate) fn is_parse_result_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let c = &mut *tp.checker;
    let Some(module_sym) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };

    // Check for ParseIssue type
    if c.try_get_member_in_module_exports_and_properties("ParseIssue", module_sym).is_none() {
        return false;
    }

    // Check for decodeSync export
    if c.try_get_member_in_module_exports_and_properties("decodeSync", module_sym).is_none() {
        return false;
    }

    // Check for encodeSync export
    if c.try_get_member_in_module_exports_and_properties("encodeSync", module_sym).is_none() {
        return false;
    }

    true
}

impl TypeParser<'_> {
    // go: internal/typeparser/schema_type.go:147 TypeParser.IsNodeReferenceToEffectParseResultModuleApi
    // IsNodeReferenceToEffectParseResultModuleApi reports whether node resolves to a member
    // exported by the "effect" package from a module that exports the ParseResult type (V3).
    pub fn is_node_reference_to_effect_parse_result_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &EFFECT_PARSE_RESULT_MODULE_DESCRIPTOR,
            member_name,
        )
    }
}

// go: internal/typeparser/schema_type.go:151 isSchemaParserSourceFile
pub(crate) fn is_schema_parser_source_file(tp: &mut TypeParser<'_>, sf: P<SourceFile>) -> bool {
    let c = &mut *tp.checker;
    let Some(module_sym) = get_symbol_of_declaration(c, sf.as_node()) else {
        return false;
    };

    // Check for decodeEffect export
    if c.try_get_member_in_module_exports_and_properties("decodeEffect", module_sym).is_none() {
        return false;
    }

    // Check for encodeEffect export
    if c.try_get_member_in_module_exports_and_properties("encodeEffect", module_sym).is_none() {
        return false;
    }

    true
}

impl TypeParser<'_> {
    // go: internal/typeparser/schema_type.go:176 TypeParser.IsNodeReferenceToEffectSchemaParserModuleApi
    // IsNodeReferenceToEffectSchemaParserModuleApi reports whether node resolves to a member
    // exported by the "effect" package from a module that exports the SchemaParser type (V4).
    pub fn is_node_reference_to_effect_schema_parser_module_api(
        &mut self,
        node: P<Node>,
        member_name: &str,
    ) -> bool {
        self.is_node_reference_to_module_export(
            node,
            &EFFECT_SCHEMA_PARSER_MODULE_DESCRIPTOR,
            member_name,
        )
    }
}
