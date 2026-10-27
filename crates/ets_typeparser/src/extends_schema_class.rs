use ets_shim::ast::Node;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/extends_schema_class.go:8 SchemaClassResult 5c1ad87ae9d8b7e5
// SchemaClassResult holds the parsed result of a class extending Schema.Class, Schema.Error or Schema.RequestClass.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SchemaClassResult {
    pub(crate) class_name: P<Node>,     // The class name identifier
    pub(crate) self_type_node: P<Node>, // The Self type argument node (first type arg of the inner call)
    pub(crate) brand_type_node: Option<P<Node>>,
    pub(crate) key_string_literal: Option<P<Node>>, // The identifier arg from the inner call (first arg), or nil
    pub(crate) fields_node: Option<P<Node>>,
}

impl SchemaClassResult {
    // go: internal/typeparser/extends_schema_class.go:16 SchemaClassResult.ClassName f438d9bc86d55028
    pub fn class_name(&self) -> P<Node> {
        self.class_name
    }

    // go: internal/typeparser/extends_schema_class.go:18 SchemaClassResult.SelfTypeNode fdf334b4e340efc1
    pub fn self_type_node(&self) -> P<Node> {
        self.self_type_node
    }

    // go: internal/typeparser/extends_schema_class.go:20 SchemaClassResult.BrandTypeNode aa9f95cdb2f6e15a
    pub fn brand_type_node(&self) -> Option<P<Node>> {
        self.brand_type_node
    }

    // go: internal/typeparser/extends_schema_class.go:22 SchemaClassResult.KeyStringLiteral 06b8cf0a53806db8
    pub fn key_string_literal(&self) -> Option<P<Node>> {
        self.key_string_literal
    }

    // go: internal/typeparser/extends_schema_class.go:24 SchemaClassResult.FieldsNode adf6a05290d9ec67
    pub fn fields_node(&self) -> Option<P<Node>> {
        self.fields_node
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/extends_schema_class.go:36 TypeParser.ExtendsSchemaClass 7f8dbeec3a207abf
    // ExtendsSchemaClass checks if a class declaration extends Schema.Class<Self>("name")({}).
    // It detects the double-call pattern:
    //
    //	class X extends Schema.Class<X>("name")({}) {}
    //
    // where the ExpressionWithTypeArguments.expression is a CallExpression (outer call)
    // whose own .expression is also a CallExpression (inner call) with type arguments,
    // and the inner call resolves to Schema.Class.
    //
    // Returns nil if the class does not extend Schema.Class.
    pub fn extends_schema_class(
        &mut self,
        class_node: impl Into<Option<P<Node>>>,
    ) -> Option<SchemaClassResult> {
        let class_node = class_node.into()?;
        cached(
            self,
            |l| &mut l.extends_schema_class,
            class_node,
            |tp| tp.extends_schema_class_like(class_node, "Class"),
        )
    }

    // go: internal/typeparser/extends_schema_class.go:46 TypeParser.ExtendsSchemaError 9013891a7c7f2d76
    pub fn extends_schema_error(
        &mut self,
        class_node: impl Into<Option<P<Node>>>,
    ) -> Option<SchemaClassResult> {
        let class_node = class_node.into()?;
        cached(
            self,
            |l| &mut l.extends_schema_error,
            class_node,
            |tp| tp.extends_schema_class_like(class_node, "Error"),
        )
    }

    // go: internal/typeparser/extends_schema_class.go:60 TypeParser.ExtendsSchemaRequestClass f1d11e661eccd1d0
    // ExtendsSchemaRequestClass checks if a class declaration extends Schema.RequestClass<Self>("name")({}).
    // Same double-call pattern as ExtendsSchemaClass but for Schema.RequestClass.
    //
    // Returns nil if the class does not extend Schema.RequestClass.
    pub fn extends_schema_request_class(
        &mut self,
        class_node: impl Into<Option<P<Node>>>,
    ) -> Option<SchemaClassResult> {
        let class_node = class_node.into()?;
        cached(
            self,
            |l| &mut l.extends_schema_request_class,
            class_node,
            |tp| tp.extends_schema_class_like(class_node, "RequestClass"),
        )
    }

    // go: internal/typeparser/extends_schema_class.go:70 TypeParser.extendsSchemaClassLike e00fa76bc25f4eb6
    pub(crate) fn extends_schema_class_like(
        &mut self,
        class_node: P<Node>,
        member_name: &str,
    ) -> Option<SchemaClassResult> {
        // Must have a name
        let class_name = class_node.name()?;

        let heritage_elements = ets_shim::ast::get_extends_heritage_clause_elements(class_node);
        if heritage_elements.is_empty() {
            return None;
        }

        for &element in heritage_elements {
            let ewta = element.as_expression_with_type_arguments();

            // The expression should be a CallExpression (the outer call)
            let outer_call_node = ewta.expression();
            if !ets_shim::ast::is_call_expression(outer_call_node) {
                continue;
            }
            let outer_call = outer_call_node.as_call_expression();

            // The outer call's expression should also be a CallExpression (the inner call)
            let inner_call_node = outer_call.expression();
            if !ets_shim::ast::is_call_expression(inner_call_node) {
                continue;
            }
            let inner_call = inner_call_node.as_call_expression();

            // The inner call must have type arguments (Schema.Class<Self>())
            let Some(type_arguments) = inner_call.type_arguments() else {
                continue;
            };
            let type_arguments = type_arguments.nodes();
            if type_arguments.is_empty() {
                continue;
            }
            let mut brand_type_node = None;
            if (member_name == "Class" || member_name == "Error") && type_arguments.len() > 1 {
                brand_type_node = Some(type_arguments[1]);
            }

            // Check if the inner call's expression resolves to Schema.<memberName>
            if !self
                .is_node_reference_to_effect_schema_module_api(inner_call.expression(), member_name)
            {
                continue;
            }

            let mut key_string_literal = None;
            if let Some(&arg) = inner_call.arguments().nodes().first()
                && ets_shim::ast::is_string_literal(arg)
            {
                key_string_literal = Some(arg);
            }

            let fields_node = outer_call.arguments().nodes().first().copied();

            return Some(SchemaClassResult {
                class_name,
                self_type_node: type_arguments[0],
                brand_type_node,
                key_string_literal,
                fields_node,
            });
        }

        None
    }
}
