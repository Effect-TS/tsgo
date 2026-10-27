use ets_shim::ast::Node;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/extends_data_tagged.go:8 DataTaggedResult 6289ae677b5314a5
// DataTaggedResult holds the parsed result of a class extending Data.TaggedError or Data.TaggedClass.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DataTaggedResult {
    pub(crate) class_name: Option<P<Node>>, // The class name identifier, or nil for an anonymous declaration
    pub(crate) call_expression: P<Node>,    // The matched Data.<member> call, always non-nil
    pub(crate) key_string_literal: Option<P<Node>>, // The key string literal from the call's first argument, or nil
    pub(crate) fields_type_node: Option<P<Node>>,
}

impl DataTaggedResult {
    // go: internal/typeparser/extends_data_tagged.go:15 DataTaggedResult.ClassName 9f461ace3ef56380
    pub fn class_name(&self) -> Option<P<Node>> {
        self.class_name
    }

    // go: internal/typeparser/extends_data_tagged.go:17 DataTaggedResult.CallExpression e5467276088380e7
    pub fn call_expression(&self) -> P<Node> {
        self.call_expression
    }

    // go: internal/typeparser/extends_data_tagged.go:19 DataTaggedResult.KeyStringLiteral 292edb367b54876e
    pub fn key_string_literal(&self) -> Option<P<Node>> {
        self.key_string_literal
    }

    // go: internal/typeparser/extends_data_tagged.go:21 DataTaggedResult.FieldsTypeNode 1e2547daa34676e4
    pub fn fields_type_node(&self) -> Option<P<Node>> {
        self.fields_type_node
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/extends_data_tagged.go:33 TypeParser.extendsDataTagged 224e622798be31cb
    // extendsDataTagged checks if a class declaration extends Data.<memberName>("key")<Fields>.
    // It detects the pattern:
    //
    //	class X extends Data.TaggedError("key")<{ msg: string }> {}
    //
    // where the ExpressionWithTypeArguments.expression is a CallExpression (Data.<memberName>("key")),
    // the call's expression is a PropertyAccessExpression resolving to Data.<memberName>,
    // and the type arguments <Fields> are on the ExpressionWithTypeArguments.
    //
    // Returns nil if the class does not match.
    pub(crate) fn extends_data_tagged(
        &mut self,
        class_node: P<Node>,
        member_name: &str,
    ) -> Option<DataTaggedResult> {
        let heritage_elements = ets_shim::ast::get_extends_heritage_clause_elements(class_node);
        if heritage_elements.is_empty() {
            return None;
        }

        for &element in heritage_elements {
            let ewta = element.as_expression_with_type_arguments();

            // The expression should be a CallExpression: Data.<memberName>("key")
            let call_node = ewta.expression();
            if !ets_shim::ast::is_call_expression(call_node) {
                continue;
            }
            let call = call_node.as_call_expression();

            // The call's expression should be a PropertyAccessExpression: Data.<memberName>
            if !self.is_node_reference_to_effect_data_module_api(call.expression(), member_name) {
                continue;
            }

            // Extract key string literal from call's first argument
            let mut key_string_literal = None;
            if let Some(&arg) = call.arguments().nodes().first()
                && ets_shim::ast::is_string_literal(arg)
            {
                key_string_literal = Some(arg);
            }

            let mut fields_type_node = None;
            if let Some(type_arguments) = ewta.type_arguments()
                && let Some(&first) = type_arguments.nodes().first()
            {
                fields_type_node = Some(first);
            }

            return Some(DataTaggedResult {
                class_name: class_node.name(),
                call_expression: call_node,
                key_string_literal,
                fields_type_node,
            });
        }

        None
    }

    // go: internal/typeparser/extends_data_tagged.go:93 TypeParser.ExtendsDataTaggedError 44f2e93398105262
    // ExtendsDataTaggedError checks if a class declaration extends Data.TaggedError("key")<Fields>.
    pub fn extends_data_tagged_error(
        &mut self,
        class_node: impl Into<Option<P<Node>>>,
    ) -> Option<DataTaggedResult> {
        let class_node = class_node.into()?;
        cached(
            self,
            |l| &mut l.extends_data_tagged_error,
            class_node,
            |tp| tp.extends_data_tagged(class_node, "TaggedError"),
        )
    }

    // go: internal/typeparser/extends_data_tagged.go:104 TypeParser.ExtendsDataTaggedClass a08f34aa951760b3
    // ExtendsDataTaggedClass checks if a class declaration extends Data.TaggedClass("tag")<Fields>.
    pub fn extends_data_tagged_class(
        &mut self,
        class_node: impl Into<Option<P<Node>>>,
    ) -> Option<DataTaggedResult> {
        let class_node = class_node.into()?;
        cached(
            self,
            |l| &mut l.extends_data_tagged_class,
            class_node,
            |tp| tp.extends_data_tagged(class_node, "TaggedClass"),
        )
    }
}
