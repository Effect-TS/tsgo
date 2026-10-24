use ets_shim::ast::Node;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/extends_data_tagged.go:8 DataTaggedErrorResult
// DataTaggedErrorResult holds the parsed result of a class extending Data.TaggedError.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DataTaggedErrorResult {
    pub(crate) class_name: Option<P<Node>>, // The class name identifier, or nil for an anonymous declaration
    pub(crate) call_expression: P<Node>,    // The matched Data.TaggedError call, always non-nil
    pub(crate) key_string_literal: Option<P<Node>>, // The key string literal from the call's first argument, or nil
    pub(crate) fields_type_node: Option<P<Node>>,
}

impl DataTaggedErrorResult {
    // go: internal/typeparser/extends_data_tagged.go:15 DataTaggedErrorResult.ClassName
    pub fn class_name(&self) -> Option<P<Node>> {
        self.class_name
    }

    // go: internal/typeparser/extends_data_tagged.go:17 DataTaggedErrorResult.CallExpression
    pub fn call_expression(&self) -> P<Node> {
        self.call_expression
    }

    // go: internal/typeparser/extends_data_tagged.go:19 DataTaggedErrorResult.KeyStringLiteral
    pub fn key_string_literal(&self) -> Option<P<Node>> {
        self.key_string_literal
    }

    // go: internal/typeparser/extends_data_tagged.go:21 DataTaggedErrorResult.FieldsTypeNode
    pub fn fields_type_node(&self) -> Option<P<Node>> {
        self.fields_type_node
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/extends_data_tagged.go:33 TypeParser.ExtendsDataTaggedError
    // ExtendsDataTaggedError checks if a class declaration extends Data.TaggedError("key")<Fields>.
    // It detects the pattern:
    //
    //	class X extends Data.TaggedError("key")<{ msg: string }> {}
    //
    // where the ExpressionWithTypeArguments.expression is a CallExpression (Data.TaggedError("key")),
    // the call's expression is a PropertyAccessExpression resolving to Data.TaggedError,
    // and the type arguments <Fields> are on the ExpressionWithTypeArguments.
    //
    // Returns nil if the class does not extend Data.TaggedError.
    pub fn extends_data_tagged_error(
        &mut self,
        class_node: impl Into<Option<P<Node>>>,
    ) -> Option<DataTaggedErrorResult> {
        let class_node = class_node.into()?;
        cached(
            self,
            |l| &mut l.extends_data_tagged_error,
            class_node,
            |tp| {
                let heritage_elements =
                    ets_shim::ast::get_extends_heritage_clause_elements(class_node);
                if heritage_elements.is_empty() {
                    return None;
                }

                for &element in heritage_elements {
                    let ewta = element.as_expression_with_type_arguments();

                    // The expression should be a CallExpression: Data.TaggedError("key")
                    let call_node = ewta.expression();
                    if !ets_shim::ast::is_call_expression(call_node) {
                        continue;
                    }
                    let call = call_node.as_call_expression();

                    // The call's expression should be a PropertyAccessExpression: Data.TaggedError
                    if !tp.is_node_reference_to_effect_data_module_api(
                        call.expression(),
                        "TaggedError",
                    ) {
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

                    return Some(DataTaggedErrorResult {
                        class_name: class_node.name(),
                        call_expression: call_node,
                        key_string_literal,
                        fields_type_node,
                    });
                }

                None
            },
        )
    }
}
