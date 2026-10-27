use ets_shim::ast::Node;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/extends_schema_tagged.go:8 SchemaTaggedResult 7cd59a2d737f3aa0
// SchemaTaggedResult holds the parsed result of a class extending Schema.TaggedClass/TaggedError/TaggedRequest.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SchemaTaggedResult {
    pub(crate) class_name: P<Node>,     // The class name identifier
    pub(crate) self_type_node: P<Node>, // The Self type argument node (first type arg of the inner call)
    pub(crate) brand_type_node: Option<P<Node>>,
    pub(crate) key_string_literal: Option<P<Node>>, // The identifier arg from the inner call (first arg), or nil
    pub(crate) tag_string_literal: Option<P<Node>>, // The tag arg from the outer call (first arg), or nil
    pub(crate) fields_node: Option<P<Node>>,
}

impl SchemaTaggedResult {
    // go: internal/typeparser/extends_schema_tagged.go:17 SchemaTaggedResult.ClassName 80eb40081fb1c4b0
    pub fn class_name(&self) -> P<Node> {
        self.class_name
    }

    // go: internal/typeparser/extends_schema_tagged.go:19 SchemaTaggedResult.SelfTypeNode c0838d28a52d0a77
    pub fn self_type_node(&self) -> P<Node> {
        self.self_type_node
    }

    // go: internal/typeparser/extends_schema_tagged.go:21 SchemaTaggedResult.BrandTypeNode 69398f0ac535671c
    pub fn brand_type_node(&self) -> Option<P<Node>> {
        self.brand_type_node
    }

    // go: internal/typeparser/extends_schema_tagged.go:23 SchemaTaggedResult.KeyStringLiteral f07eceb1df44f4c1
    pub fn key_string_literal(&self) -> Option<P<Node>> {
        self.key_string_literal
    }

    // go: internal/typeparser/extends_schema_tagged.go:25 SchemaTaggedResult.TagStringLiteral e49053298c904ded
    pub fn tag_string_literal(&self) -> Option<P<Node>> {
        self.tag_string_literal
    }

    // go: internal/typeparser/extends_schema_tagged.go:27 SchemaTaggedResult.FieldsNode 6afce22de436b1db
    pub fn fields_node(&self) -> Option<P<Node>> {
        self.fields_node
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/extends_schema_tagged.go:39 TypeParser.extendsSchemaTagged 8e701be8c078c136
    // extendsSchemaTagged checks if a class declaration extends Schema.<memberName>
    // with the double-call pattern:
    //
    //	class X extends Schema.TaggedClass<X>("identifier")("tag", { ... }) {}
    //
    // where the ExpressionWithTypeArguments.expression is a CallExpression (outer call)
    // whose own .expression is also a CallExpression (inner call) with type arguments,
    // and the inner call resolves to Schema.<memberName>.
    //
    // Returns nil if the class does not match.
    pub(crate) fn extends_schema_tagged(
        &mut self,
        class_node: P<Node>,
        member_name: &str,
    ) -> Option<SchemaTaggedResult> {
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

            // The inner call must have type arguments (Schema.TaggedClass<Self>())
            let Some(type_arguments) = inner_call.type_arguments() else {
                continue;
            };
            let type_arguments = type_arguments.nodes();
            if type_arguments.is_empty() {
                continue;
            }
            let mut brand_type_node = None;
            if (member_name == "TaggedClass" || member_name == "TaggedError")
                && type_arguments.len() > 1
            {
                brand_type_node = Some(type_arguments[1]);
            }

            // Check if the inner call's expression resolves to Schema.<memberName>
            if !self
                .is_node_reference_to_effect_schema_module_api(inner_call.expression(), member_name)
            {
                continue;
            }

            // Extract keyStringLiteral from inner call's first argument (if it's a string literal)
            let mut key_string_literal = None;
            if let Some(&arg) = inner_call.arguments().nodes().first()
                && ets_shim::ast::is_string_literal(arg)
            {
                key_string_literal = Some(arg);
            }

            // Extract tagStringLiteral from outer call's first argument (if it's a string literal)
            let outer_arguments = outer_call.arguments().nodes();
            let mut tag_string_literal = None;
            if let Some(&arg) = outer_arguments.first()
                && ets_shim::ast::is_string_literal(arg)
            {
                tag_string_literal = Some(arg);
            }

            let mut fields_node = None;
            if outer_arguments.len() > 1 {
                fields_node = Some(outer_arguments[1]);
            }

            return Some(SchemaTaggedResult {
                class_name,
                self_type_node: type_arguments[0],
                brand_type_node,
                key_string_literal,
                tag_string_literal,
                fields_node,
            });
        }

        None
    }

    // go: internal/typeparser/extends_schema_tagged.go:139 TypeParser.ExtendsSchemaTaggedClass 6681882a7b34a9c7
    // ExtendsSchemaTaggedClass checks if a class declaration extends Schema.TaggedClass<T>("identifier")("tag", { ... }).
    pub fn extends_schema_tagged_class(
        &mut self,
        class_node: impl Into<Option<P<Node>>>,
    ) -> Option<SchemaTaggedResult> {
        let class_node = class_node.into()?;
        cached(
            self,
            |l| &mut l.extends_schema_tagged_class,
            class_node,
            |tp| tp.extends_schema_tagged(class_node, "TaggedClass"),
        )
    }

    // go: internal/typeparser/extends_schema_tagged.go:150 TypeParser.ExtendsSchemaTaggedError e8bb40c4788e6e8b
    // ExtendsSchemaTaggedError checks if a class declaration extends Schema.TaggedError<T>("identifier")("tag", { ... }).
    pub fn extends_schema_tagged_error(
        &mut self,
        class_node: impl Into<Option<P<Node>>>,
    ) -> Option<SchemaTaggedResult> {
        let class_node = class_node.into()?;
        cached(
            self,
            |l| &mut l.extends_schema_tagged_error,
            class_node,
            |tp| tp.extends_schema_tagged(class_node, "TaggedError"),
        )
    }

    // go: internal/typeparser/extends_schema_tagged.go:161 TypeParser.ExtendsSchemaTaggedRequest fc15a080c268af5f
    // ExtendsSchemaTaggedRequest checks if a class declaration extends Schema.TaggedRequest<T>("identifier")("tag", { ... }).
    pub fn extends_schema_tagged_request(
        &mut self,
        class_node: impl Into<Option<P<Node>>>,
    ) -> Option<SchemaTaggedResult> {
        let class_node = class_node.into()?;
        cached(
            self,
            |l| &mut l.extends_schema_tagged_request,
            class_node,
            |tp| tp.extends_schema_tagged(class_node, "TaggedRequest"),
        )
    }
}
