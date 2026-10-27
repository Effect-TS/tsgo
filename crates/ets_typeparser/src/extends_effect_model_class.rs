use ets_shim::ast::Node;
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/extends_effect_model_class.go:9 EffectModelClassResult ed63a1428821ce17
// EffectModelClassResult holds the parsed result of a class extending Model.Class
// from effect/schema.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EffectModelClassResult {
    pub(crate) class_name: P<Node>,     // The class name identifier
    pub(crate) self_type_node: P<Node>, // The Self type argument node (first type arg of the inner call)
}

impl EffectModelClassResult {
    // go: internal/typeparser/extends_effect_model_class.go:14 EffectModelClassResult.ClassName 45f93a577fea829a
    pub fn class_name(self) -> P<Node> {
        self.class_name
    }

    // go: internal/typeparser/extends_effect_model_class.go:16 EffectModelClassResult.SelfTypeNode 064d261344daa7c1
    pub fn self_type_node(self) -> P<Node> {
        self.self_type_node
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/extends_effect_model_class.go:27 TypeParser.ExtendsEffectModelClass ebfcfac741bd11bc
    // ExtendsEffectModelClass checks if a class declaration extends Model.Class<Self>(...)({...})
    // from the effect/schema module.
    // It detects the double-call pattern:
    //
    //	class X extends Model.Class<X>("name")({}) {}
    //
    // where the ExpressionWithTypeArguments.expression is a CallExpression (outer call)
    // whose own .expression is also a CallExpression (inner call) with type arguments,
    // and the inner call resolves to the effect Model.Class.
    pub fn extends_effect_model_class(
        &mut self,
        class_node: impl Into<Option<P<Node>>>,
    ) -> Option<EffectModelClassResult> {
        let class_node = class_node.into()?;
        cached(
            self,
            |l| &mut l.extends_effect_model_class,
            class_node,
            |tp| {
                let class_name = class_node.name()?;

                let heritage_elements =
                    ets_shim::ast::get_extends_heritage_clause_elements(class_node);
                if heritage_elements.is_empty() {
                    return None;
                }

                for &element in heritage_elements {
                    let ewta = element.as_expression_with_type_arguments();

                    let outer_call_node = ewta.expression();
                    if !ets_shim::ast::is_call_expression(outer_call_node) {
                        continue;
                    }
                    let outer_call = outer_call_node.as_call_expression();

                    let inner_call_node = outer_call.expression();
                    if !ets_shim::ast::is_call_expression(inner_call_node) {
                        continue;
                    }
                    let inner_call = inner_call_node.as_call_expression();

                    let Some(type_arguments) = inner_call.type_arguments() else {
                        continue;
                    };
                    if type_arguments.nodes().is_empty() {
                        continue;
                    }

                    if !tp.is_node_reference_to_effect_model_module_api(
                        inner_call.expression(),
                        "Class",
                    ) {
                        continue;
                    }

                    return Some(EffectModelClassResult {
                        class_name,
                        self_type_node: type_arguments.nodes()[0],
                    });
                }

                None
            },
        )
    }
}
