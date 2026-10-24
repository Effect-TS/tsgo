// Package schemagen provides structural (type-checker-based) schema generation
// that converts resolved TypeScript types into Effect Schema expressions.
// Unlike the AST-based SchemaGen which works on syntax nodes, this generator works
// on actual types from the checker, enabling handling of recursive types and
// complex type hierarchies through a hoisting mechanism.

use std::borrow::Cow;
use std::sync::LazyLock;

use ets_rewriter::Tracker;
use ets_shim::ast::{self, Kind, Node, NodeFlags, SourceFile, SymbolFlags, TokenFlags};
use ets_shim::checker::{LiteralValue, Type, TypeFlags, TypeId};
use ets_shim::core::P;
use ets_shim::core::collections::OrderedMap;
use ets_typeparser::{EffectMajorVersion, TypeParser};
use regex::Regex;
use rustc_hash::FxHashMap;

use crate::*;

// go: internal/schemagen/structural.go:21 StructuralSchemaGen
// StructuralSchemaGen holds the context for converting resolved types to Schema expressions.
//
// Rust: Go keeps the checker and the type parser in the generator; the parser borrows the checker mutably, so
// the generator owns only its own state and every method that reads types takes the parser as `tp`
// (docs/PORTING.md, "Types"; the checker is `tp.checker()`). The tracker is borrowed (Go: a pointer). Go's maps
// that are iterated (`hoistedSchemas`, `nameToType`) iterate in random order; here they keep insertion order
// (docs/DIVERGENCES.md, "`schemagen` structural generator map order").
pub struct StructuralSchemaGen<'t> {
    pub tracker: &'t Tracker,
    pub source_file: P<SourceFile>,
    pub schema_identifier: String,
    pub version: EffectMajorVersion,

    // hoistedSchemas maps types to expression generators for reuse
    pub(crate) hoisted_schemas: OrderedMap<TypeId, HoistedEntry>,
    // typeToStatementIndex tracks which types have been hoisted to which statement index
    pub(crate) type_to_statement_index: FxHashMap<TypeId, usize>,
    // nameToType maps requested names to their types
    pub(crate) name_to_type: OrderedMap<String, P<Type>>,
    // schemaStatements accumulates generated top-level statements
    pub(crate) schema_statements: Vec<P<Node>>,
    // usedGlobalIdentifiers tracks used identifier names and their count
    pub(crate) used_global_identifiers: FxHashMap<String, i64>,
}

// go: internal/schemagen/structural.go:41 hoistedEntry
// Rust: every `createRef` Go stores returns `Tracker.NewIdentifier(name)` for a name it captures; the entry keeps
// that name and `create_ref` builds the identifier.
#[derive(Clone)]
pub(crate) struct HoistedEntry {
    pub(crate) t: P<Type>,
    pub(crate) ref_name: String,
}

impl HoistedEntry {
    // go: internal/schemagen/structural.go:41 hoistedEntry
    pub(crate) fn create_ref(&self, tracker: &Tracker) -> P<Node> {
        tracker.new_identifier(tracker.alloc_text(&self.ref_name))
    }
}

// go: internal/schemagen/structural.go:47 processingContext
// processingContext tracks state during recursive type traversal.
#[derive(Clone)]
pub(crate) struct ProcessingContext {
    pub(crate) depth: i64,
    pub(crate) max_depth: i64,
    pub(crate) hoist_name: String,
    pub(crate) version: EffectMajorVersion,
}

// go: internal/schemagen/structural.go:55 NewStructuralSchemaGen
// NewStructuralSchemaGen creates a StructuralSchemaGen for type-checker-based schema generation.
//
// Rust: Go's `tp` and `c` arguments are passed to the methods instead (see `StructuralSchemaGen`).
pub fn new_structural_schema_gen(
    tracker: &Tracker,
    sf: P<SourceFile>,
    version: EffectMajorVersion,
) -> StructuralSchemaGen<'_> {
    StructuralSchemaGen {
        tracker,
        source_file: sf,
        schema_identifier: ets_typeparser::find_module_identifier(sf, "Schema"),
        version,
        hoisted_schemas: OrderedMap::default(),
        type_to_statement_index: FxHashMap::default(),
        name_to_type: OrderedMap::default(),
        schema_statements: Vec::new(),
        used_global_identifiers: FxHashMap::default(),
    }
}

/// Rust-only: Go's `errors.New(text)` / `fmt.Errorf(...)` of this file.
fn new_error(text: String) -> Error {
    Error(Cow::Owned(text))
}

impl StructuralSchemaGen<'_> {
    // go: internal/schemagen/structural.go:72 StructuralSchemaGen.createApiPropertyAccess
    // createApiPropertyAccess creates Schema.<name>.
    pub(crate) fn create_api_property_access(&self, name: &str) -> P<Node> {
        let t = self.tracker;
        t.new_property_access_expression(
            t.new_identifier(t.alloc_text(&self.schema_identifier)),
            None,
            t.new_identifier(t.alloc_text(name)),
            NodeFlags::None,
        )
    }

    // go: internal/schemagen/structural.go:82 StructuralSchemaGen.createApiCall
    // createApiCall creates Schema.<name>(...args).
    pub(crate) fn create_api_call(&self, name: &str, args: Vec<P<Node>>) -> P<Node> {
        let t = self.tracker;
        t.new_call_expression(
            t.new_property_access_expression(
                t.new_identifier(t.alloc_text(&self.schema_identifier)),
                None,
                t.new_identifier(t.alloc_text(name)),
                NodeFlags::None,
            ),
            None,
            None,
            t.new_node_list(args),
            NodeFlags::None,
        )
    }

    // go: internal/schemagen/structural.go:97 StructuralSchemaGen.pushHoistedStatement
    // pushHoistedStatement records a generated statement and its reference generator.
    //
    // Rust: `ref_name` is the name Go's `createRef` closure returns an identifier for (see `HoistedEntry`).
    pub(crate) fn push_hoisted_statement(
        &mut self,
        name: &str,
        t: P<Type>,
        statement: P<Node>,
        ref_name: &str,
    ) {
        *self.used_global_identifiers.entry(name.to_string()).or_insert(0) += 1;
        self.schema_statements.push(statement);
        self.type_to_statement_index.insert(t.id(), self.schema_statements.len() - 1);
        self.hoisted_schemas.insert(t.id(), HoistedEntry { t, ref_name: ref_name.to_string() });
    }

    // go: internal/schemagen/structural.go:105 StructuralSchemaGen.pushHoistedVariableStatement
    // pushHoistedVariableStatement creates a const variable statement and hoists it.
    pub(crate) fn push_hoisted_variable_statement(
        &mut self,
        name: &str,
        t: P<Type>,
        result: P<Node>,
    ) {
        let tr = self.tracker;
        let var_decl = tr.new_variable_declaration(
            tr.new_identifier(tr.alloc_text(name)),
            None,
            None,
            Some(result),
        );
        let var_decl_list =
            tr.new_variable_declaration_list(tr.new_node_list(vec![var_decl]), NodeFlags::Const);
        let stmt = tr.new_variable_statement(None, var_decl_list);
        self.push_hoisted_statement(name, t, stmt, name);
    }

    // go: internal/schemagen/structural.go:122 StructuralSchemaGen.processType
    // processType converts a TypeScript type to a Schema expression, with hoisting support.
    pub(crate) fn process_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: P<Type>,
        ctx: &ProcessingContext,
    ) -> Result<P<Node>, Error> {
        if ctx.depth >= ctx.max_depth {
            return Err(new_error("maximum depth exceeded".to_string()));
        }

        // Try to resolve a hoist name from the type
        let hoist_name = self.resolve_hoist_name(tp, t);

        let nested_ctx = ProcessingContext {
            depth: ctx.depth + 1,
            max_depth: ctx.max_depth,
            hoist_name: hoist_name.clone(),
            version: ctx.version,
        };

        // Check if we can reuse a hoisted schema
        for entry in self.hoisted_schemas.values() {
            if entry.t == t || types_equal(tp, t, entry.t) {
                return Ok(entry.create_ref(self.tracker));
            }
        }

        // Process the type
        let (expr, skip_hoisting) = self.process_type_impl(tp, t, &nested_ctx)?;

        // Hoist if we have a name and hoisting isn't skipped
        if !skip_hoisting && !hoist_name.is_empty() {
            self.push_hoisted_variable_statement(&hoist_name, t, expr);
            if let Some(entry) = self.hoisted_schemas.get(&t.id()) {
                return Ok(entry.create_ref(self.tracker));
            }
        }

        Ok(expr)
    }

    // go: internal/schemagen/structural.go:162 StructuralSchemaGen.resolveHoistName
    // resolveHoistName tries to find a name for the type for hoisting purposes.
    pub(crate) fn resolve_hoist_name(&mut self, tp: &mut TypeParser<'_>, t: P<Type>) -> String {
        // Check nameToType map first
        for (name, &existing_type) in &self.name_to_type {
            if existing_type == t {
                return name.clone();
            }
        }

        // Try to get name from the type's symbol
        if let Some(symbol) = t.symbol()
            && symbol.declarations().len() == 1
        {
            let decl = symbol.declarations()[0];
            let mut hoist_name = "";
            if decl.kind() == Kind::InterfaceDeclaration {
                hoist_name = decl.as_interface_declaration().name().as_identifier().text();
            } else if let Some(parent) = decl.parent()
                && parent.kind() == Kind::TypeAliasDeclaration
            {
                hoist_name = parent.as_type_alias_declaration().name().as_identifier().text();
            }
            if !hoist_name.is_empty() {
                let existing_type = self.name_to_type.get(hoist_name).copied();
                if let Some(existing_type) = existing_type
                    && types_equal(tp, t, existing_type)
                {
                    return hoist_name.to_string();
                }
                // Avoid name conflicts
                let used_count = self.used_global_identifiers.get(hoist_name).copied().unwrap_or(0);
                self.used_global_identifiers.insert(hoist_name.to_string(), used_count + 1);
                if used_count > 0 {
                    return format!("{hoist_name}_{used_count}");
                }
                return hoist_name.to_string();
            }
        }

        String::new()
    }
}

// go: internal/schemagen/structural.go:198 StructuralSchemaGen.typesEqual
// typesEqual checks if two types are bidirectionally assignable.
//
// Rust: it reads only the checker, so it is a free function over the parser (it is called while the generator's
// maps are borrowed).
pub(crate) fn types_equal(tp: &mut TypeParser<'_>, a: P<Type>, b: P<Type>) -> bool {
    tp.checker().is_type_assignable_to(a, b) && tp.checker().is_type_assignable_to(b, a)
}

impl StructuralSchemaGen<'_> {
    // go: internal/schemagen/structural.go:204 StructuralSchemaGen.processTypeImpl
    // processTypeImpl dispatches type processing based on type flags.
    //
    // Rust: Go's `(expr, skipHoisting, err)` is `Result<(expr, skipHoisting), Error>`.
    pub(crate) fn process_type_impl(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: P<Type>,
        ctx: &ProcessingContext,
    ) -> Result<(P<Node>, bool), Error> {
        let flags = t.flags();

        // Primitive types
        if flags.intersects(TypeFlags::String) {
            return Ok((self.create_api_property_access("String"), true));
        }
        if flags.intersects(TypeFlags::Number) {
            return Ok((self.create_api_property_access("Number"), true));
        }
        if flags.intersects(TypeFlags::Boolean) {
            return Ok((self.create_api_property_access("Boolean"), true));
        }
        if flags.intersects(TypeFlags::BigInt) {
            return Ok((self.create_api_property_access("BigInt"), true));
        }
        if flags.intersects(TypeFlags::Void) {
            return Ok((self.create_api_property_access("Void"), true));
        }
        if flags.intersects(TypeFlags::Undefined) {
            return Ok((self.create_api_property_access("Undefined"), true));
        }
        if flags.intersects(TypeFlags::Null) {
            return Ok((self.create_api_property_access("Null"), true));
        }
        if flags.intersects(TypeFlags::Never) {
            return Ok((self.create_api_property_access("Never"), true));
        }
        if flags.intersects(TypeFlags::Any) {
            return Ok((self.create_api_property_access("Any"), true));
        }
        if flags.intersects(TypeFlags::Unknown) {
            return Ok((self.create_api_property_access("Unknown"), true));
        }

        let tr = self.tracker;

        // Literal types
        if flags.intersects(TypeFlags::StringLiteral)
            && let Some(LiteralValue::String(val)) = t.as_literal_type().value()
        {
            return Ok((
                self.create_api_call("Literal", vec![tr.new_string_literal(val, TokenFlags::None)]),
                true,
            ));
        }
        if flags.intersects(TypeFlags::NumberLiteral) {
            // Go: `t.AsLiteralType().String()` (`ValueToString`, which panics on a nil value).
            let text = ets_shim::checker::value_to_string(
                t.as_literal_type().value().expect("unhandled value type in valueToString"),
            );
            return Ok((
                self.create_api_call(
                    "Literal",
                    vec![tr.new_numeric_literal(tr.alloc_text(&text), TokenFlags::None)],
                ),
                true,
            ));
        }
        if flags.intersects(TypeFlags::BooleanLiteral) {
            if t.as_literal_type().value() == Some(LiteralValue::Boolean(true)) {
                return Ok((
                    self.create_api_call(
                        "Literal",
                        vec![tr.new_keyword_expression(Kind::TrueKeyword)],
                    ),
                    true,
                ));
            }
            return Ok((
                self.create_api_call(
                    "Literal",
                    vec![tr.new_keyword_expression(Kind::FalseKeyword)],
                ),
                true,
            ));
        }

        // Union types
        if flags.intersects(TypeFlags::Union) {
            let types = t.types();
            return self.process_union_type(tp, types, ctx);
        }

        // Intersection types
        if flags.intersects(TypeFlags::Intersection) {
            let types = t.types();
            return self.process_intersection_type(tp, types, ctx);
        }

        // Array types (check before generic object)
        if tp.checker().is_array_type(t) {
            return self.process_array_type(tp, t, ctx);
        }

        // Tuple types
        if ets_shim::checker::is_tuple_type_exported(t) {
            return self.process_tuple_type(tp, t, ctx);
        }

        // Object types
        if flags.intersects(TypeFlags::Object) {
            if let Some(sym) = t.symbol() {
                let type_name = tp.checker().symbol_to_string_exported(sym);
                if type_name == "Date" {
                    return Ok((self.create_api_property_access("Date"), false));
                }
                if type_name == "ReadonlyArray" || type_name == "Array" {
                    return self.process_array_type(tp, t, ctx);
                }
            }
            return self.process_object_type(tp, t, ctx);
        }

        Err(new_error(format!("type with flags {} is not supported", flags.bits())))
    }

    // go: internal/schemagen/structural.go:306 StructuralSchemaGen.processUnionType
    // processUnionType converts union types to Schema.Union or optimized Schema.Literal.
    pub(crate) fn process_union_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        types: &[P<Type>],
        ctx: &ProcessingContext,
    ) -> Result<(P<Node>, bool), Error> {
        // Check if all members are literals (V3 optimization)
        let mut all_literals = true;
        for t in types {
            let f = t.flags();
            if !f.intersects(TypeFlags::StringLiteral)
                && !f.intersects(TypeFlags::NumberLiteral)
                && !f.intersects(TypeFlags::BooleanLiteral)
            {
                all_literals = false;
                break;
            }
        }

        if all_literals && ctx.version != EffectMajorVersion::V4 {
            let mut literals = Vec::with_capacity(types.len());
            for &t in types {
                let expr = self.process_type(tp, t, ctx)?;
                // Extract literal value from Schema.Literal(value) call
                if expr.kind() == Kind::CallExpression {
                    let ce = expr.as_call_expression();
                    if let Some(&first) = ce.arguments().nodes().first() {
                        literals.push(first);
                        continue;
                    }
                }
                literals.push(expr);
            }
            return Ok((self.create_api_call("Literal", literals), false));
        }

        // Process each union member
        let mut members = Vec::with_capacity(types.len());
        for &t in types {
            let expr = self.process_type(tp, t, ctx)?;
            members.push(expr);
        }

        if members.len() == 1 {
            return Ok((members[0], false));
        }

        let args = if ctx.version == EffectMajorVersion::V4 {
            vec![
                self.tracker
                    .new_array_literal_expression(self.tracker.new_node_list(members), false),
            ]
        } else {
            members
        };
        Ok((self.create_api_call("Union", args), false))
    }

    // go: internal/schemagen/structural.go:361 StructuralSchemaGen.processIntersectionType
    // processIntersectionType converts intersection types to Schema.extend chains.
    pub(crate) fn process_intersection_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        types: &[P<Type>],
        ctx: &ProcessingContext,
    ) -> Result<(P<Node>, bool), Error> {
        let mut members = Vec::with_capacity(types.len());
        for &t in types {
            let expr = self.process_type(tp, t, ctx)?;
            members.push(expr);
        }

        if members.is_empty() {
            return Err(new_error("empty intersection type".to_string()));
        }
        if members.len() == 1 {
            return Ok((members[0], false));
        }

        // firstSchema.pipe(Schema.extend(second), Schema.extend(third), ...)
        let first = members[0];
        let mut extend_args = Vec::with_capacity(members.len() - 1);
        for &member in &members[1..] {
            extend_args.push(self.create_api_call("extend", vec![member]));
        }
        let tr = self.tracker;
        Ok((
            tr.new_call_expression(
                tr.new_property_access_expression(
                    first,
                    None,
                    tr.new_identifier("pipe"),
                    NodeFlags::None,
                ),
                None,
                None,
                tr.new_node_list(extend_args),
                NodeFlags::None,
            ),
            false,
        ))
    }

    // go: internal/schemagen/structural.go:395 StructuralSchemaGen.processArrayType
    // processArrayType converts array types to Schema.Array (wrapping in Schema.mutable for non-readonly).
    pub(crate) fn process_array_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: P<Type>,
        ctx: &ProcessingContext,
    ) -> Result<(P<Node>, bool), Error> {
        let type_args = tp.checker().get_type_arguments(t);
        if type_args.is_empty() {
            return Err(new_error("array type has no type arguments".to_string()));
        }

        let elem_schema = self.process_type(tp, type_args[0], ctx)?;
        let expr = self.create_api_call("Array", vec![elem_schema]);
        if tp.checker().is_readonly_array_type(t) {
            return Ok((expr, false));
        }
        Ok((self.create_api_call("mutable", vec![expr]), false))
    }

    // go: internal/schemagen/structural.go:413 StructuralSchemaGen.processTupleType
    // processTupleType converts tuple types to Schema.Tuple.
    pub(crate) fn process_tuple_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: P<Type>,
        ctx: &ProcessingContext,
    ) -> Result<(P<Node>, bool), Error> {
        let type_args = tp.checker().get_type_arguments(t);
        let mut elems = Vec::with_capacity(type_args.len());
        for &ta in type_args {
            let expr = self.process_type(tp, ta, ctx)?;
            elems.push(expr);
        }

        let args = if ctx.version == EffectMajorVersion::V4 {
            vec![
                self.tracker.new_array_literal_expression(self.tracker.new_node_list(elems), false),
            ]
        } else {
            elems
        };
        Ok((self.create_api_call("Tuple", args), false))
    }
}

// go: internal/schemagen/structural.go:434 validIdentifierRegex
// validIdentifierRegex matches valid JS identifiers.
pub(crate) static VALID_IDENTIFIER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z_$][a-zA-Z0-9_$]*$").unwrap());

impl StructuralSchemaGen<'_> {
    // go: internal/schemagen/structural.go:437 StructuralSchemaGen.processObjectType
    // processObjectType converts object types to Schema.Struct or Schema.Class.
    pub(crate) fn process_object_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: P<Type>,
        ctx: &ProcessingContext,
    ) -> Result<(P<Node>, bool), Error> {
        struct RecordEntry {
            key: P<Node>,
            value: P<Node>,
        }

        let tr = self.tracker;
        let properties = tp.checker().get_properties_of_type_exported(t);
        let mut property_assignments = Vec::with_capacity(properties.len());

        for &prop in properties {
            let prop_name = tp.checker().symbol_to_string_exported(prop);
            let prop_type = tp.checker().get_type_of_symbol_exported(prop);

            let is_optional = prop.flags().intersects(SymbolFlags::Optional);

            let schema_expr = if is_optional {
                let inner_expr = self.process_type(tp, prop_type, ctx)?;
                self.create_api_call("optional", vec![inner_expr])
            } else {
                self.process_type(tp, prop_type, ctx)?
            };

            // Create property name node
            let prop_name_node = if VALID_IDENTIFIER_REGEX.is_match(&prop_name) {
                tr.new_identifier(tr.alloc_text(&prop_name))
            } else {
                tr.new_string_literal(tr.alloc_text(&prop_name), TokenFlags::None)
            };

            let assignment =
                tr.new_property_assignment(None, prop_name_node, None, None, schema_expr);
            property_assignments.push(assignment);
        }

        // Handle index signatures
        let index_infos = tp.checker().get_index_infos_of_type(t);
        let has_properties = !property_assignments.is_empty();
        let mut args =
            vec![tr.new_object_literal_expression(
                tr.new_node_list(property_assignments),
                has_properties,
            )];

        let mut records = Vec::new();
        for info in index_infos {
            // Go: `IndexInfo_keyType` / `IndexInfo_valueType` (set once the index info is resolved).
            let key_type = info.key_type.get().expect("index info key type");
            let value_type = info.value_type.get().expect("index info value type");

            let key_schema = self.process_type(tp, key_type, ctx)?;
            let value_schema = self.process_type(tp, value_type, ctx)?;
            records.push(RecordEntry { key: key_schema, value: value_schema });
        }

        // V4: use StructWithRest for structs with index signatures
        if ctx.version == EffectMajorVersion::V4 && !records.is_empty() {
            let mut record_exprs = Vec::with_capacity(records.len());
            for r in &records {
                record_exprs.push(self.create_api_call("Record", vec![r.key, r.value]));
            }
            return Ok((
                self.create_api_call(
                    "StructWithRest",
                    vec![
                        self.create_api_call("Struct", args),
                        tr.new_array_literal_expression(tr.new_node_list(record_exprs), false),
                    ],
                ),
                !has_properties,
            ));
        }

        // When no records and we have a hoistName, generate Schema.Class
        if records.is_empty() && !ctx.hoist_name.is_empty() {
            let hoist_name = tr.alloc_text(&ctx.hoist_name);
            let class_access = self.create_api_property_access("Class");
            let inner_call = tr.new_call_expression(
                class_access,
                None,
                Some(tr.new_node_list(vec![
                    tr.new_type_reference_node(tr.new_identifier(hoist_name), None),
                ])),
                tr.new_node_list(vec![tr.new_string_literal(hoist_name, TokenFlags::None)]),
                NodeFlags::None,
            );
            let outer_call = tr.new_call_expression(
                inner_call,
                None,
                None,
                tr.new_node_list(args),
                NodeFlags::None,
            );
            let expr_with_type_args = tr
                .new_expression_with_type_arguments(outer_call, Some(tr.new_node_list(Vec::new())));
            let heritage_clause = tr.new_heritage_clause(
                Kind::ExtendsKeyword,
                tr.new_node_list(vec![expr_with_type_args]),
            );
            let class_decl = tr.new_class_declaration(
                None, // modifiers (export added later)
                Some(tr.new_identifier(hoist_name)),
                None,
                Some(tr.new_node_list(vec![heritage_clause])),
                tr.new_node_list(Vec::new()),
            );

            self.push_hoisted_statement(&ctx.hoist_name, t, class_decl, &ctx.hoist_name);
            if let Some(entry) = self.hoisted_schemas.get(&t.id()) {
                return Ok((entry.create_ref(tr), true));
            }
            return Ok((tr.new_identifier(hoist_name), true));
        }

        // V3: records go as additional arguments to Schema.Struct
        for r in &records {
            args.push(tr.new_object_literal_expression(
                tr.new_node_list(vec![
                    tr.new_property_assignment(None, tr.new_identifier("key"), None, None, r.key),
                    tr.new_property_assignment(
                        None,
                        tr.new_identifier("value"),
                        None,
                        None,
                        r.value,
                    ),
                ]),
                false,
            ));
        }

        Ok((self.create_api_call("Struct", args), !has_properties))
    }

    // go: internal/schemagen/structural.go:570 StructuralSchemaGen.ScanExistingSchemas
    // ScanExistingSchemas looks for existing schema variables in scope and records them
    // in the hoisted map, so they can be reused instead of regenerated.
    pub fn scan_existing_schemas(&mut self, tp: &mut TypeParser<'_>, scope: P<Node>) {
        let symbols = tp.checker().get_symbols_in_scope_exported(scope, SymbolFlags::Value);
        for sym in symbols {
            let name = tp.checker().symbol_to_string_exported(sym);
            self.used_global_identifiers.insert(name.clone(), 1);
            let Some(t) =
                tp.checker().get_type_of_symbol_at_location(sym, Some(self.source_file.as_node()))
            else {
                continue;
            };
            let Some(schema_types) = tp.effect_schema_types(t) else {
                continue;
            };
            let a_type = schema_types.a();
            let captured_name = name;
            self.hoisted_schemas
                .insert(a_type.id(), HoistedEntry { t: a_type, ref_name: captured_name });
        }
    }

    // go: internal/schemagen/structural.go:596 StructuralSchemaGen.Process
    // Process generates schema statements for one or more named types.
    // It populates internal state and returns accumulated statements.
    //
    // Rust: Go's `map[string]*checker.Type` is an insertion-ordered map (see `StructuralSchemaGen`).
    pub fn process(
        &mut self,
        tp: &mut TypeParser<'_>,
        type_map: &OrderedMap<String, P<Type>>,
        scope: P<Node>,
        is_exported: bool,
    ) -> Vec<P<Node>> {
        // Process each type
        struct ProcessResult {
            requested_name: String,
            t: P<Type>,
            result: P<Node>,
        }

        for (name, &t) in type_map {
            self.name_to_type.insert(name.clone(), t);
        }

        // Scan existing schemas in scope for reuse
        self.scan_existing_schemas(tp, scope);

        let tr = self.tracker;

        let mut results = Vec::new();
        for (name, &t) in type_map {
            let ctx = ProcessingContext {
                depth: 0,
                max_depth: 200,
                hoist_name: String::new(),
                version: self.version,
            };
            let expr = match self.process_type(tp, t, &ctx) {
                Ok(expr) => expr,
                // On error, emit an error comment
                Err(err) => {
                    tr.new_identifier(tr.alloc_text(&format!("undefined /* {} */", err.error())))
                }
            };
            results.push(ProcessResult { requested_name: name.clone(), t, result: expr });
        }

        // Add variable statements for types that were not hoisted
        for r in &results {
            if self.type_to_statement_index.contains_key(&r.t.id()) {
                continue;
            }
            let var_decl = tr.new_variable_declaration(
                tr.new_identifier(tr.alloc_text(&r.requested_name)),
                None,
                None,
                Some(r.result),
            );
            let var_decl_list = tr
                .new_variable_declaration_list(tr.new_node_list(vec![var_decl]), NodeFlags::Const);
            let modifiers = tr.new_modifier_list(vec![tr.new_modifier(Kind::ExportKeyword)]);
            let stmt = tr.new_variable_statement(Some(modifiers), var_decl_list);
            self.schema_statements.push(stmt);
            self.type_to_statement_index.insert(r.t.id(), self.schema_statements.len() - 1);
        }

        // Add export modifiers to requested type statements
        if is_exported {
            let mut export_indices = rustc_hash::FxHashSet::default();
            for t in self.name_to_type.values() {
                if let Some(&idx) = self.type_to_statement_index.get(&t.id()) {
                    export_indices.insert(idx);
                }
            }
            for i in 0..self.schema_statements.len() {
                if !export_indices.contains(&i) {
                    continue;
                }
                let stmt = self.schema_statements[i];
                if stmt.kind() == Kind::VariableStatement {
                    // Already has export modifier from non-hoisted path; skip if so
                    let vs = stmt.as_variable_statement();
                    if vs.modifiers().is_some_and(|m| !m.nodes().is_empty()) {
                        continue;
                    }
                    let modifiers =
                        tr.new_modifier_list(vec![tr.new_modifier(Kind::ExportKeyword)]);
                    self.schema_statements[i] =
                        tr.new_variable_statement(Some(modifiers), vs.declaration_list());
                } else if stmt.kind() == Kind::ClassDeclaration {
                    let cd = stmt.as_class_declaration();
                    if cd.modifiers().is_some_and(|m| !m.nodes().is_empty()) {
                        continue;
                    }
                    let modifiers =
                        tr.new_modifier_list(vec![tr.new_modifier(Kind::ExportKeyword)]);
                    self.schema_statements[i] = tr.new_class_declaration(
                        Some(modifiers),
                        cd.name(),
                        cd.type_parameters(),
                        cd.heritage_clauses(),
                        cd.members(),
                    );
                }
            }
        }

        // Set parent pointers
        for &stmt in &self.schema_statements {
            ast::set_parent_in_children(stmt);
        }

        self.schema_statements.clone()
    }
}
