use ets_shim::ast::{Kind, Node, OuterExpressionKinds, Symbol, SymbolFlags};
use ets_shim::checker::LiteralValue;
use ets_shim::checker::evaluator::{self, Result as EvaluatorResult, new_result};
use ets_shim::core::P;
use ets_shim::core::jsnum;

use crate::*;

/// Rust-only: the state Go's evaluator closure in `EvaluateConstantExpression` captures (the parser and the
/// `Number` global), passed back to it by tsrs's `evaluator::evaluate` as its host.
struct ConstantEvaluationHost<'a, 'c> {
    tp: &'a mut TypeParser<'c>,
    number_symbol: Option<P<Symbol>>,
}

impl TypeParser<'_> {
    // go: internal/typeparser/constant_evaluation.go:12 TypeParser.EvaluateConstantExpression c55981dd1bf2e994
    // EvaluateConstantExpression evaluates the subset of JavaScript expressions
    // that can be resolved statically without executing user code.
    pub fn evaluate_constant_expression(
        &mut self,
        node: impl Into<Option<P<Node>>>,
        location: Option<P<Node>>,
    ) -> EvaluatorResult {
        let Some(node) = node.into() else {
            return new_result(None, false, false, false);
        };
        let location = location.unwrap_or(node);

        let number_symbol =
            self.checker.get_global_symbol_exported("Number", SymbolFlags::Value, None);
        let mut host = ConstantEvaluationHost { tp: self, number_symbol };
        evaluator::evaluate(
            &mut host,
            evaluate_constant_entity,
            OuterExpressionKinds::Parentheses,
            node,
            location,
        )
    }

    // go: internal/typeparser/constant_evaluation.go:78 TypeParser.EvaluateConstantNumber 47531c51edf9c1a9
    // EvaluateConstantNumber evaluates a statically resolvable JavaScript numeric
    // expression without executing user code.
    pub fn evaluate_constant_number(
        &mut self,
        node: impl Into<Option<P<Node>>>,
        location: Option<P<Node>>,
    ) -> Option<f64> {
        let result = self.evaluate_constant_expression(node, location);
        match result.value {
            Some(LiteralValue::Number(number)) => Some(number.0),
            _ => None,
        }
    }
}

// go: internal/typeparser/constant_evaluation.go:12 TypeParser.EvaluateConstantExpression c55981dd1bf2e994
// Rust: the evaluator closure of `EvaluateConstantExpression` (tsrs's evaluator takes a function and a host).
fn evaluate_constant_entity(
    host: &mut ConstantEvaluationHost<'_, '_>,
    entity: P<Node>,
    entity_location: P<Node>,
) -> EvaluatorResult {
    let tp = &mut *host.tp;
    let mut receiver: Option<P<Node>> = None;
    let mut property_name = "";
    match entity.kind() {
        Kind::PropertyAccessExpression => {
            let access = entity.as_property_access_expression();
            receiver = Some(access.expression());
            property_name = access.name().text();
        }
        Kind::ElementAccessExpression => {
            let access = entity.as_element_access_expression();
            if ets_shim::ast::is_string_literal_like(access.argument_expression()) {
                receiver = Some(access.expression());
                property_name = access.argument_expression().text();
            }
        }
        _ => {}
    }

    if host.number_symbol.is_some()
        && let Some(receiver) = receiver
        && tp.get_symbol_at_location(receiver) == host.number_symbol
    {
        match property_name {
            "POSITIVE_INFINITY" => {
                return new_result(Some(LiteralValue::Number(jsnum::inf(1))), false, false, false);
            }
            "NEGATIVE_INFINITY" => {
                return new_result(Some(LiteralValue::Number(jsnum::inf(-1))), false, false, false);
            }
            "NaN" => {
                return new_result(Some(LiteralValue::Number(jsnum::nan())), false, false, false);
            }
            _ => {}
        }
    }

    let result = tp.checker.evaluate_entity(entity, Some(entity_location));
    if result.value.is_some() || entity.kind() != Kind::Identifier {
        return result;
    }

    let Some(symbol) = tp.get_symbol_at_location(entity) else {
        return result;
    };
    if !tp.checker.is_constant_variable(symbol) {
        return result;
    }
    let Some(declaration) = symbol.value_declaration() else {
        return result;
    };
    // Rust: Go's `entityLocation != nil` holds (the evaluator always passes a location).
    if !ets_shim::ast::is_variable_declaration(declaration)
        || declaration.type_node().is_some()
        || declaration.initializer().is_none()
        || (declaration == entity_location
            || !tp.checker.is_block_scoped_name_declared_before_use(declaration, entity_location))
    {
        return result;
    }

    let initializer = declaration.initializer().expect("checked above");
    let constant = evaluator::evaluate(
        host,
        evaluate_constant_entity,
        OuterExpressionKinds::Parentheses,
        initializer,
        declaration,
    );
    let mut resolved_other_files = constant.resolved_other_files;
    if ets_shim::ast::get_source_file_of_node(entity_location)
        != ets_shim::ast::get_source_file_of_node(declaration)
    {
        resolved_other_files = true;
    }
    new_result(constant.value, constant.is_syntactically_string, resolved_other_files, true)
}
