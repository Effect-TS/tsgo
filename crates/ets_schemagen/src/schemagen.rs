use std::borrow::Cow;

use ets_rewriter::Tracker;
use ets_shim::ast::{self, Kind, Node, NodeFlags, SourceFile, TokenFlags};
use ets_shim::core::P;
use ets_shim::scanner;
use ets_typeparser::{EffectMajorVersion, ModuleReference};

/// Rust-only: Go's `error` values of this package.
///
/// Go only reads their text (`err.Error()`): the sentinels below are constants, the structural generator's
/// ad-hoc errors (`errors.New`, `fmt.Errorf`) owned texts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub Cow<'static, str>);

impl Error {
    /// Rust-only: Go's `err.Error()`.
    pub fn error(&self) -> &str {
        &self.0
    }
}

// go: internal/schemagen/schemagen.go:18 ErrTypeParametersNotSupported 331666aceb73a760
// Errors returned by schema generation when a type cannot be converted.
pub const ERR_TYPE_PARAMETERS_NOT_SUPPORTED: Error =
    Error(Cow::Borrowed("could not process types with type parameters"));
// go: internal/schemagen/schemagen.go:18 ErrTypeParametersNotSupported 331666aceb73a760
pub const ERR_ONLY_LITERAL_PROPERTIES_SUPPORTED: Error =
    Error(Cow::Borrowed("only literal properties are supported"));
// go: internal/schemagen/schemagen.go:18 ErrTypeParametersNotSupported 331666aceb73a760
pub const ERR_REQUIRED_EXPLICIT_TYPES: Error =
    Error(Cow::Borrowed("only explicit types are supported"));
// go: internal/schemagen/schemagen.go:18 ErrTypeParametersNotSupported 331666aceb73a760
pub const ERR_INDEX_SIGNATURE_MULTIPLE_PARAMETERS: Error =
    Error(Cow::Borrowed("only index signatures with one parameter are supported"));

// go: internal/schemagen/schemagen.go:26 SchemaGen f7dfb3b648d66ace
// SchemaGen holds the context needed to convert AST type nodes into Schema expressions.
//
// Rust: the tracker is borrowed (Go: a pointer); the generator only builds nodes with its factory.
pub struct SchemaGen<'t> {
    pub tracker: &'t Tracker,
    pub source_file: P<SourceFile>,
    pub schema_reference: ModuleReference,
    pub version: EffectMajorVersion,
}

// go: internal/schemagen/schemagen.go:35 New 671ac9c7a1db6d4e
// New creates a SchemaGen from the given tracker and source file, resolving the
// Schema import identifier automatically.
pub fn new(tracker: &Tracker, sf: P<SourceFile>, version: EffectMajorVersion) -> SchemaGen<'_> {
    SchemaGen {
        tracker,
        source_file: sf,
        schema_reference: ets_typeparser::find_module_reference(sf, "Schema"),
        version,
    }
}

impl SchemaGen<'_> {
    // go: internal/schemagen/schemagen.go:45 SchemaGen.createApiPropertyAccess 4e0bcc7d6582debc
    // createApiPropertyAccess creates Schema.<name> (e.g., Schema.String).
    pub(crate) fn create_api_property_access(&self, name: &str) -> P<Node> {
        self.tracker.new_property_access_path(&self.schema_reference.append(&[name]))
    }

    // go: internal/schemagen/schemagen.go:50 SchemaGen.createApiCall c50a8c2ca5867af7
    // createApiCall creates Schema.<name>(...args) (e.g., Schema.Array(Schema.String)).
    pub(crate) fn create_api_call(&self, name: &str, args: Vec<P<Node>>) -> P<Node> {
        let t = self.tracker;
        t.new_call_expression(
            t.new_property_access_path(&self.schema_reference.append(&[name])),
            None, // questionDotToken
            None, // typeArguments
            t.new_node_list(args),
            NodeFlags::None,
        )
    }

    // go: internal/schemagen/schemagen.go:62 SchemaGen.unsupportedComment b2c1a2740f28090b
    // unsupportedComment creates a placeholder identifier for unsupported conversions.
    // This produces /* Not supported conversion: <text> */ in the emitted output.
    pub(crate) fn unsupported_comment(&self, node: P<Node>) -> P<Node> {
        let text = scanner::get_text_of_node(node);
        let t = self.tracker;
        t.new_identifier(t.alloc_text(&format!("undefined /* Not supported conversion: {text} */")))
    }

    // go: internal/schemagen/schemagen.go:68 SchemaGen.processNode e45837890b13fd5f
    // processNode recursively converts a TypeScript type AST node into a Schema expression.
    //
    // Rust: the node lists of union, tuple and type-literal nodes are never nil in tsrs, so Go's nil-list checks
    // of those three drop (an empty list behaves as Go's nil one).
    pub(crate) fn process_node(&self, node: Option<P<Node>>) -> Result<P<Node>, Error> {
        let Some(node) = node else {
            return Ok(self.create_api_property_access("Unknown"));
        };

        // Primitive keywords
        match node.kind() {
            Kind::AnyKeyword => return Ok(self.create_api_property_access("Any")),
            Kind::NeverKeyword => return Ok(self.create_api_property_access("Never")),
            Kind::UnknownKeyword => return Ok(self.create_api_property_access("Unknown")),
            Kind::VoidKeyword => return Ok(self.create_api_property_access("Void")),
            Kind::NullKeyword => return Ok(self.create_api_property_access("Null")),
            Kind::UndefinedKeyword => return Ok(self.create_api_property_access("Undefined")),
            Kind::StringKeyword => return Ok(self.create_api_property_access("String")),
            Kind::NumberKeyword => return Ok(self.create_api_property_access("Number")),
            Kind::BooleanKeyword => return Ok(self.create_api_property_access("Boolean")),
            Kind::BigIntKeyword => return Ok(self.create_api_property_access("BigInt")),
            _ => {}
        }

        // Literal types: string/number/boolean literals and null
        if node.kind() == Kind::LiteralType {
            let lt = node.as_literal_type_node();
            if lt.literal().kind() == Kind::NullKeyword {
                return Ok(self.create_api_property_access("Null"));
            }
            if let Some(literals) = self.parse_all_literals(Some(node))
                && !literals.is_empty()
            {
                return Ok(self.create_api_call("Literal", literals));
            }
        }

        // Union types: A | B
        if node.kind() == Kind::UnionType {
            let ut = node.as_union_type_node();

            // Optimization: try to collapse all-literal unions into Schema.Literal(...)
            if self.version != EffectMajorVersion::V4
                && let Some(all_literals) = self.parse_all_literals(Some(node))
                && !all_literals.is_empty()
            {
                return Ok(self.create_api_call("Literal", all_literals));
            }

            let members = self.process_node_list(ut.types().nodes())?;

            let args = if self.version == EffectMajorVersion::V4 {
                vec![
                    self.tracker
                        .new_array_literal_expression(self.tracker.new_node_list(members), false),
                ]
            } else {
                members
            };
            return Ok(self.create_api_call("Union", args));
        }

        // Intersection types: A & B
        if node.kind() == Kind::IntersectionType {
            let it = node.as_intersection_type_node();
            if it.types().nodes().is_empty() {
                return Ok(self.unsupported_comment(node));
            }

            let members = self.process_node_list(it.types().nodes())?;

            if members.len() == 1 {
                return Ok(members[0]);
            }

            // firstSchema.pipe(Schema.extend(second), Schema.extend(third), ...)
            let first = members[0];
            let mut extend_args = Vec::with_capacity(members.len() - 1);
            for &member in &members[1..] {
                extend_args.push(self.create_api_call("extend", vec![member]));
            }
            let t = self.tracker;
            return Ok(t.new_call_expression(
                t.new_property_access_expression(
                    first,
                    None,
                    t.new_identifier("pipe"),
                    NodeFlags::None,
                ),
                None,
                None,
                t.new_node_list(extend_args),
                NodeFlags::None,
            ));
        }

        // Type operator: keyof T, readonly T
        if node.kind() == Kind::TypeOperator {
            let to = node.as_type_operator_node();
            if to.operator() == Kind::KeyOfKeyword {
                let inner = self.process_node(Some(to.type_()))?;
                return Ok(self.create_api_call("keyof", vec![inner]));
            }
            if to.operator() == Kind::ReadonlyKeyword {
                return self.process_node(Some(to.type_()));
            }
        }

        // Array type: T[]
        if node.kind() == Kind::ArrayType {
            let at = node.as_array_type_node();
            let elem_schema = self.process_node(Some(at.element_type()))?;
            return Ok(self.create_api_call("Array", vec![elem_schema]));
        }

        // Tuple type: [A, B, C]
        if node.kind() == Kind::TupleType {
            let tt = node.as_tuple_type_node();
            let elems = self.process_node_list(tt.elements().nodes())?;
            let args = if self.version == EffectMajorVersion::V4 {
                vec![
                    self.tracker
                        .new_array_literal_expression(self.tracker.new_node_list(elems), false),
                ]
            } else {
                elems
            };
            return Ok(self.create_api_call("Tuple", args));
        }

        // Type literal (object shape): { a: string, b: number }
        if node.kind() == Kind::TypeLiteral {
            let tl = node.as_type_literal_node();
            let member_nodes = tl.members().nodes();
            let (properties, records) = self.process_members(member_nodes)?;
            let mut args = vec![
                self.tracker
                    .new_object_literal_expression(self.tracker.new_node_list(properties), true),
            ];
            args.extend(records);
            return Ok(self.create_api_call("Struct", args));
        }

        // Parenthesized type: (A)
        if node.kind() == Kind::ParenthesizedType {
            let pt = node.as_parenthesized_type_node();
            return self.process_node(Some(pt.type_()));
        }

        // Type reference: resolve known Effect types and data types
        if node.kind() == Kind::TypeReference {
            let tr = node.as_type_reference_node();
            let data_type_name = self.entity_name_to_data_type_name(Some(tr.type_name()));
            if !data_type_name.is_empty() {
                return self.process_type_reference(node, data_type_name);
            }

            // Simple type reference without type arguments → pass through as identifier
            if tr.type_arguments().is_none_or(|args| args.nodes().is_empty()) {
                return Ok(self.type_entity_name_to_node(Some(tr.type_name())));
            }
        }

        // Unsupported
        Ok(self.unsupported_comment(node))
    }

    // go: internal/schemagen/schemagen.go:258 SchemaGen.processTypeReference d0f32ebd303c02a7
    // processTypeReference handles known type references like Array, Record, Option, etc.
    pub(crate) fn process_type_reference(
        &self,
        node: P<Node>,
        data_type_name: &str,
    ) -> Result<P<Node>, Error> {
        let tr = node.as_type_reference_node();
        let type_args: &[P<Node>] = tr.type_arguments().map_or(&[], |args| args.nodes());
        let t = self.tracker;

        match data_type_name {
            "Duration" | "Date" => Ok(self.create_api_property_access(data_type_name)),

            "Option" | "Chunk" | "Array" => {
                let elements = self.process_node_list(type_args)?;
                Ok(self.create_api_call(data_type_name, elements))
            }

            "Record" => {
                let elements = self.process_node_list(type_args)?;
                if elements.len() >= 2 {
                    if self.version == EffectMajorVersion::V4 {
                        return Ok(self.create_api_call("Record", vec![elements[0], elements[1]]));
                    }
                    return Ok(self.create_api_call(
                        data_type_name,
                        vec![t.new_object_literal_expression(
                            t.new_node_list(vec![
                                t.new_property_assignment(
                                    None,
                                    t.new_identifier("key"),
                                    None,
                                    None,
                                    elements[0],
                                ),
                                t.new_property_assignment(
                                    None,
                                    t.new_identifier("value"),
                                    None,
                                    None,
                                    elements[1],
                                ),
                            ]),
                            false,
                        )],
                    ));
                }
                Ok(self.unsupported_comment(node))
            }

            "Either" => {
                let elements = self.process_node_list(type_args)?;
                if elements.len() >= 2 {
                    return Ok(self.create_api_call(
                        data_type_name,
                        vec![t.new_object_literal_expression(
                            t.new_node_list(vec![
                                t.new_property_assignment(
                                    None,
                                    t.new_identifier("right"),
                                    None,
                                    None,
                                    elements[0],
                                ),
                                t.new_property_assignment(
                                    None,
                                    t.new_identifier("left"),
                                    None,
                                    None,
                                    elements[1],
                                ),
                            ]),
                            false,
                        )],
                    ));
                }
                Ok(self.unsupported_comment(node))
            }

            "Pick" | "Omit" => {
                if type_args.len() != 2 {
                    return Ok(self.unsupported_comment(node));
                }
                let base_type = self.process_node(Some(type_args[0]))?;
                let Some(string_literals) =
                    self.parse_all_literals(Some(type_args[1])).filter(|l| !l.is_empty())
                else {
                    return Ok(self.unsupported_comment(node));
                };
                // Use lowercase: "pick" or "omit"
                let method_name = if data_type_name == "Omit" { "omit" } else { "pick" };
                Ok(t.new_call_expression(
                    t.new_property_access_expression(
                        base_type,
                        None,
                        t.new_identifier("pipe"),
                        NodeFlags::None,
                    ),
                    None,
                    None,
                    t.new_node_list(vec![self.create_api_call(method_name, string_literals)]),
                    NodeFlags::None,
                ))
            }

            _ => Ok(self.unsupported_comment(node)),
        }
    }

    // go: internal/schemagen/schemagen.go:342 SchemaGen.entityNameToDataTypeName a9fd9dd99db13a9f
    // entityNameToDataTypeName maps a type name to a recognized data type name.
    //
    // Rust: a qualified name's two sides are never nil in tsrs, so Go's nil check of them drops.
    pub(crate) fn entity_name_to_data_type_name(&self, type_name: Option<P<Node>>) -> &'static str {
        let Some(type_name) = type_name else {
            return "";
        };
        if type_name.kind() == Kind::Identifier {
            let name = type_name.as_identifier().text();
            return match name {
                "Date" | "Pick" | "Omit" | "Record" => name,
                "ReadonlyArray" | "Array" => "Array",
                _ => "",
            };
        }
        // Qualified name: Module.Name (e.g., Option.Option)
        if type_name.kind() == Kind::QualifiedName {
            let qn = type_name.as_qualified_name();
            let right_name = scanner::get_text_of_node(qn.right());
            // Check if the left side is the imported reference to a known module
            for module_name in ["Option", "Either", "Chunk", "Duration"] {
                let imported = ets_typeparser::find_module_reference(self.source_file, module_name);
                if imported.matches_node(qn.left()) && right_name == module_name {
                    return module_name;
                }
            }
        }
        ""
    }

    // go: internal/schemagen/schemagen.go:375 SchemaGen.typeEntityNameToNode 471fc1d5bcd58cd1
    // typeEntityNameToNode converts a type entity name to an expression node (identifier or property access).
    pub(crate) fn type_entity_name_to_node(&self, entity_name: Option<P<Node>>) -> P<Node> {
        let t = self.tracker;
        let Some(entity_name) = entity_name else {
            return t.new_identifier("unknown");
        };
        if entity_name.kind() == Kind::Identifier {
            return t.new_identifier(entity_name.as_identifier().text());
        }
        if entity_name.kind() == Kind::QualifiedName {
            let qn = entity_name.as_qualified_name();
            let left = self.type_entity_name_to_node(Some(qn.left()));
            return t.new_property_access_expression(
                left,
                None,
                t.new_identifier(t.alloc_text(&scanner::get_text_of_node(qn.right()))),
                NodeFlags::None,
            );
        }
        t.new_identifier(t.alloc_text(&scanner::get_text_of_node(entity_name)))
    }

    // go: internal/schemagen/schemagen.go:396 SchemaGen.parseAllLiterals 9da5e42f5514837e
    // parseAllLiterals tries to parse a type node as a collection of literal values.
    // Returns (literals, true) on success, (nil, false) if the node contains non-literal types.
    //
    // Rust: Go's `(literals, ok)` is `Option<Vec<_>>`.
    pub(crate) fn parse_all_literals(&self, node: Option<P<Node>>) -> Option<Vec<P<Node>>> {
        let node = node?;
        let t = self.tracker;

        if node.kind() == Kind::LiteralType {
            let lt = node.as_literal_type_node();
            let literal = lt.literal();
            return match literal.kind() {
                Kind::StringLiteral => Some(vec![
                    t.new_string_literal(literal.as_string_literal().text(), TokenFlags::None),
                ]),
                Kind::NumericLiteral => {
                    Some(vec![t.new_numeric_literal(
                        literal.as_numeric_literal().text(),
                        TokenFlags::None,
                    )])
                }
                Kind::TrueKeyword => Some(vec![t.new_keyword_expression(Kind::TrueKeyword)]),
                Kind::FalseKeyword => Some(vec![t.new_keyword_expression(Kind::FalseKeyword)]),
                _ => None,
            };
        }

        if node.kind() == Kind::UnionType {
            let ut = node.as_union_type_node();
            let mut result = Vec::new();
            for &member in ut.types().nodes() {
                let lits = self.parse_all_literals(Some(member))?;
                result.extend(lits);
            }
            return Some(result);
        }

        if node.kind() == Kind::ParenthesizedType {
            let pt = node.as_parenthesized_type_node();
            return self.parse_all_literals(Some(pt.type_()));
        }

        None
    }

    // go: internal/schemagen/schemagen.go:441 SchemaGen.processNodeList a1f37737bb4867ba
    // processNodeList processes a list of type nodes, converting each to a Schema expression.
    pub(crate) fn process_node_list(&self, nodes: &[P<Node>]) -> Result<Vec<P<Node>>, Error> {
        let mut result = Vec::with_capacity(nodes.len());
        for &n in nodes {
            let expr = self.process_node(Some(n))?;
            result.push(expr);
        }
        Ok(result)
    }
}

// go: internal/schemagen/schemagen.go:454 membersResult 042ea05b9a6b00b2
// membersResult holds the processed properties and records from type members.
//
// Rust: declared but never used in Go, as here.
#[expect(dead_code, reason = "Go declares membersResult without using it")]
pub(crate) struct MembersResult {
    pub(crate) properties: Vec<P<Node>>,
    pub(crate) records: Vec<P<Node>>,
}

impl SchemaGen<'_> {
    // go: internal/schemagen/schemagen.go:460 SchemaGen.processMembers cbfd7a55c8d79268
    // processMembers handles property signatures and index signatures from type members.
    //
    // Rust: Go's `(properties, records, err)` is `Result<(properties, records), Error>`. A property signature's
    // name is never nil in tsrs, so Go's nil-name check drops.
    pub(crate) fn process_members(
        &self,
        members: &[P<Node>],
    ) -> Result<(Vec<P<Node>>, Vec<P<Node>>), Error> {
        let t = self.tracker;
        let mut properties = Vec::new();
        let mut records = Vec::new();
        for &member in members {
            if member.kind() == Kind::PropertySignature {
                let ps = member.as_property_signature_declaration();
                let name = ps.name();
                if name.kind() != Kind::Identifier && name.kind() != Kind::StringLiteral {
                    return Err(ERR_ONLY_LITERAL_PROPERTIES_SUPPORTED);
                }
                let Some(ps_type) = ps.type_() else {
                    return Err(ERR_REQUIRED_EXPLICIT_TYPES);
                };

                let mut prop_schema = self.process_node(Some(ps_type))?;

                // Wrap in Schema.optional(...) if question token present
                if member.question_token().is_some() {
                    prop_schema = self.create_api_call("optional", vec![prop_schema]);
                }

                // Create property name node
                let prop_name = if name.kind() == Kind::Identifier {
                    t.new_identifier(name.as_identifier().text())
                } else {
                    t.new_string_literal(name.as_string_literal().text(), TokenFlags::None)
                };

                let assignment =
                    t.new_property_assignment(None, prop_name, None, None, prop_schema);
                properties.push(assignment);
            }

            if member.kind() == Kind::IndexSignature {
                let is = member.as_index_signature_declaration();
                let Some(parameters) = is.parameters().filter(|p| p.nodes().len() == 1) else {
                    return Err(ERR_INDEX_SIGNATURE_MULTIPLE_PARAMETERS);
                };
                let param = parameters.nodes()[0];
                let param_decl = param.as_parameter_declaration();
                let Some(param_type) = param_decl.type_() else {
                    return Err(ERR_REQUIRED_EXPLICIT_TYPES);
                };

                let key = self.process_node(Some(param_type))?;
                let value = self.process_node(is.type_())?;

                let record = t.new_object_literal_expression(
                    t.new_node_list(vec![
                        t.new_property_assignment(None, t.new_identifier("key"), None, None, key),
                        t.new_property_assignment(
                            None,
                            t.new_identifier("value"),
                            None,
                            None,
                            value,
                        ),
                    ]),
                    false,
                );
                records.push(record);
            }
        }
        Ok((properties, records))
    }

    // go: internal/schemagen/schemagen.go:531 SchemaGen.ProcessInterfaceDeclaration ec254bb9f7380e1a
    // ProcessInterfaceDeclaration generates schema statements for an interface declaration.
    // When preferClass is true and there are no index signatures, generates a Schema.Class declaration.
    // Otherwise generates a Schema.Struct variable declaration.
    // Returns the generated statement node.
    pub fn process_interface_declaration(
        &self,
        node: P<Node>,
        prefer_class: bool,
    ) -> Result<P<Node>, Error> {
        let iface = node.as_interface_declaration();
        if iface.type_parameters().is_some_and(|tps| !tps.nodes().is_empty()) {
            return Err(ERR_TYPE_PARAMETERS_NOT_SUPPORTED);
        }

        let name = scanner::get_text_of_node(iface.name());

        let member_nodes = iface.members().nodes();

        let (properties, records) = self.process_members(member_nodes)?;

        if prefer_class && records.is_empty() {
            return Ok(self.create_export_schema_class_declaration(&name, properties));
        }

        let mut args = vec![
            self.tracker
                .new_object_literal_expression(self.tracker.new_node_list(properties), true),
        ];
        args.extend(records);
        let schema_struct = self.create_api_call("Struct", args);
        Ok(self.create_export_variable_declaration(&name, schema_struct))
    }

    // go: internal/schemagen/schemagen.go:565 SchemaGen.ProcessTypeAliasDeclaration 99c15d66bf8eed13
    // ProcessTypeAliasDeclaration generates schema statements for a type alias declaration.
    // When preferClass is true and the type is an object literal without index signatures,
    // generates a Schema.Class declaration. Otherwise generates a variable declaration.
    // Returns the generated statement node.
    pub fn process_type_alias_declaration(
        &self,
        node: P<Node>,
        prefer_class: bool,
    ) -> Result<P<Node>, Error> {
        let ta = node.as_type_alias_declaration();
        if ta.type_parameters().is_some_and(|tps| !tps.nodes().is_empty()) {
            return Err(ERR_TYPE_PARAMETERS_NOT_SUPPORTED);
        }

        let name = scanner::get_text_of_node(ta.name());

        // For preferClass with object literal types, generate class declaration
        if prefer_class
            && let Some(ta_type) = ta.type_()
            && ta_type.kind() == Kind::TypeLiteral
        {
            let tl = ta_type.as_type_literal_node();
            let member_nodes = tl.members().nodes();
            let (properties, records) = self.process_members(member_nodes)?;
            if records.is_empty() {
                return Ok(self.create_export_schema_class_declaration(&name, properties));
            }
        }

        let effect_schema = self.process_node(ta.type_())?;

        Ok(self.create_export_variable_declaration(&name, effect_schema))
    }

    // go: internal/schemagen/schemagen.go:598 SchemaGen.createExportVariableDeclaration b129c3ae47b7c407
    // createExportVariableDeclaration creates: export const <name> = <initializer>
    pub(crate) fn create_export_variable_declaration(
        &self,
        name: &str,
        initializer: P<Node>,
    ) -> P<Node> {
        let t = self.tracker;
        let var_decl = t.new_variable_declaration(
            t.new_identifier(t.alloc_text(name)),
            None, // exclamationToken
            None, // type
            Some(initializer),
        );
        let var_decl_list =
            t.new_variable_declaration_list(t.new_node_list(vec![var_decl]), NodeFlags::Const);
        let modifiers = t.new_modifier_list(vec![t.new_modifier(Kind::ExportKeyword)]);
        t.new_variable_statement(Some(modifiers), var_decl_list)
    }

    // go: internal/schemagen/schemagen.go:618 SchemaGen.createExportSchemaClassDeclaration e826e597b4d057c7
    // createExportSchemaClassDeclaration creates:
    //
    //	export class <Name> extends Schema.Class<Name>("Name")({...}) { }
    pub(crate) fn create_export_schema_class_declaration(
        &self,
        name: &str,
        properties: Vec<P<Node>>,
    ) -> P<Node> {
        let t = self.tracker;
        let name = t.alloc_text(name);

        // Schema.Class
        let class_access = self.create_api_property_access("Class");

        // Schema.Class<Name>("Name")
        let inner_call = t.new_call_expression(
            class_access,
            None, // questionDotToken
            Some(t.new_node_list(vec![t.new_type_reference_node(t.new_identifier(name), None)])),
            t.new_node_list(vec![t.new_string_literal(name, TokenFlags::None)]),
            NodeFlags::None,
        );

        // Schema.Class<Name>("Name")({...})
        let outer_call = t.new_call_expression(
            inner_call,
            None,
            None,
            t.new_node_list(vec![
                t.new_object_literal_expression(t.new_node_list(properties), true),
            ]),
            NodeFlags::None,
        );

        // extends ...
        let expr_with_type_args =
            t.new_expression_with_type_arguments(outer_call, Some(t.new_node_list(Vec::new())));
        let heritage_clause =
            t.new_heritage_clause(Kind::ExtendsKeyword, t.new_node_list(vec![expr_with_type_args]));

        let modifiers = t.new_modifier_list(vec![t.new_modifier(Kind::ExportKeyword)]);

        t.new_class_declaration(
            Some(modifiers),
            Some(t.new_identifier(name)),
            None, // typeParameters
            Some(t.new_node_list(vec![heritage_clause])),
            t.new_node_list(Vec::new()), // empty members
        )
    }

    // go: internal/schemagen/schemagen.go:670 SchemaGen.Process 0dd3198db41ff7e3
    // Process converts an interface or type alias declaration into a schema statement.
    // On error, returns a comment node describing the error.
    pub fn process(&self, node: P<Node>, prefer_class: bool) -> Option<P<Node>> {
        let result = match node.kind() {
            Kind::InterfaceDeclaration => self.process_interface_declaration(node, prefer_class),
            Kind::TypeAliasDeclaration => self.process_type_alias_declaration(node, prefer_class),
            _ => return None,
        };

        let result = match result {
            Ok(result) => result,
            Err(err) => {
                let t = self.tracker;
                return Some(
                    t.new_identifier(t.alloc_text(&format!("undefined /* {} */", err.error()))),
                );
            }
        };

        ast::set_parent_in_children(result);
        Some(result)
    }
}

// go: internal/schemagen/schemagen.go:693 HasIndexSignatures c7210144c0915995
// HasIndexSignatures checks whether an interface or type alias has index signatures.
// Used by typeToEffectSchemaClass to determine applicability.
pub fn has_index_signatures(node: P<Node>) -> bool {
    let members: &[P<Node>] = match node.kind() {
        Kind::InterfaceDeclaration => node.as_interface_declaration().members().nodes(),
        Kind::TypeAliasDeclaration => {
            let ta = node.as_type_alias_declaration();
            match ta.type_() {
                Some(ta_type) if ta_type.kind() == Kind::TypeLiteral => {
                    ta_type.as_type_literal_node().members().nodes()
                }
                _ => &[],
            }
        }
        _ => &[],
    };
    members.iter().any(|m| m.kind() == Kind::IndexSignature)
}
