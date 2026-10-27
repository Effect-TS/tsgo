// Package typeparser provides Effect type detection and parsing utilities.

use std::sync::Arc;

use ets_readonly::Slice;
use ets_shim::ast::{Kind, ModifierFlags, Node, NodeFlags, Symbol};
use ets_shim::checker::SignatureKind;
use ets_shim::core::{P, TextPos};
use ets_shim::scanner::get_text_of_node;

use crate::*;

// go: internal/typeparser/effect_fn_opportunity.go:12 EffectFnOpportunityResult 515f5759cc1550d6
// EffectFnOpportunityResult represents a function that can be converted to Effect.fn.
//
// Rust: Go's node pointer fields are `P<Node>`; the pipe arguments are `None` where Go's slice is nil.
pub struct EffectFnOpportunityResult {
    pub(crate) target_node: P<Node>, // The function node being reported
    pub(crate) name_identifier: P<Node>, // The discovered name node for the function
    pub(crate) generator_function: Option<P<Node>>, // Non-nil for gen opportunity, nil for regular
    pub(crate) pipe_arguments: Option<Vec<P<Node>>>, // Pipe args from piped Effect.gen (may be empty)
    pub(crate) explicit_trace_expression: Option<P<Node>>, // Span name from Effect.withSpan if last pipe arg, or nil
    pub(crate) suggested_trace_name: String, // The local function/variable name for suggested span
    pub(crate) inferred_trace_name: String, // Context-aware name (e.g., "ServiceTag.member" or exported name)
    pub(crate) effect_module: Option<P<Node>>, // The Effect module identifier
    pub(crate) has_gen_body: bool,          // True for gen opportunity, false for regular
    pub(crate) is_layer_member: bool, // True when the target is a property value inside a Layer service definition
}

impl EffectFnOpportunityResult {
    // go: internal/typeparser/effect_fn_opportunity.go:25 EffectFnOpportunityResult.TargetNode a0077236ce948edd
    pub fn target_node(&self) -> P<Node> {
        self.target_node
    }

    // go: internal/typeparser/effect_fn_opportunity.go:27 EffectFnOpportunityResult.NameIdentifier c84f4cd72ea04866
    pub fn name_identifier(&self) -> P<Node> {
        self.name_identifier
    }

    // go: internal/typeparser/effect_fn_opportunity.go:29 EffectFnOpportunityResult.GeneratorFunction 84460046db842d38
    pub fn generator_function(&self) -> Option<P<Node>> {
        self.generator_function
    }

    // go: internal/typeparser/effect_fn_opportunity.go:33 EffectFnOpportunityResult.PipeArguments 6ff7a67daf4add69
    pub fn pipe_arguments(&self) -> Slice<'_, P<Node>> {
        ets_readonly::new(self.pipe_arguments.as_deref())
    }

    // go: internal/typeparser/effect_fn_opportunity.go:37 EffectFnOpportunityResult.ExplicitTraceExpression 2abb73aa4adc1bd8
    pub fn explicit_trace_expression(&self) -> Option<P<Node>> {
        self.explicit_trace_expression
    }

    // go: internal/typeparser/effect_fn_opportunity.go:41 EffectFnOpportunityResult.SuggestedTraceName de1e4a9b3d89642b
    pub fn suggested_trace_name(&self) -> &str {
        &self.suggested_trace_name
    }

    // go: internal/typeparser/effect_fn_opportunity.go:43 EffectFnOpportunityResult.InferredTraceName b66a3e29516003cd
    pub fn inferred_trace_name(&self) -> &str {
        &self.inferred_trace_name
    }

    // go: internal/typeparser/effect_fn_opportunity.go:45 EffectFnOpportunityResult.EffectModule c7b4bc6fe8a4b2ab
    pub fn effect_module(&self) -> Option<P<Node>> {
        self.effect_module
    }

    // go: internal/typeparser/effect_fn_opportunity.go:47 EffectFnOpportunityResult.HasGenBody f800e21367e73a90
    pub fn has_gen_body(&self) -> bool {
        self.has_gen_body
    }

    // go: internal/typeparser/effect_fn_opportunity.go:49 EffectFnOpportunityResult.IsLayerMember 8202e22e361a52f7
    pub fn is_layer_member(&self) -> bool {
        self.is_layer_member
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/effect_fn_opportunity.go:53 TypeParser.IsInsideEffectFn 4e2b6a2b6bfdcf8d
    // IsInsideEffectFn checks if a function node is already the first argument
    // of an Effect.fn, Effect.fnGen, or Effect.fnUntraced call.
    pub fn is_inside_effect_fn(&mut self, fn_node: impl Into<Option<P<Node>>>) -> bool {
        let Some(fn_node) = fn_node.into() else {
            return false;
        };

        let Some(parent) = fn_node.parent() else {
            return false;
        };
        if parent.kind() != Kind::CallExpression {
            return false;
        }

        let parent_call = parent.as_call_expression();
        let arguments = parent_call.arguments().nodes();
        if arguments.is_empty() {
            return false;
        }

        // The function must be the first argument
        if arguments[0] != fn_node {
            return false;
        }

        if self.effect_fn_call(parent).is_some() {
            return true;
        }

        false
    }

    // go: internal/typeparser/effect_fn_opportunity.go:82 TypeParser.ParseEffectFnOpportunity f33ec963f00478e9
    // ParseEffectFnOpportunity detects whether a function node can be converted to Effect.fn.
    // Returns nil if the node is not an eligible candidate.
    pub fn parse_effect_fn_opportunity(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> Option<Arc<EffectFnOpportunityResult>> {
        let node = node.into()?;
        if !is_effect_fn_opportunity_candidate(node) {
            return None;
        }

        cached(
            self,
            |l| &mut l.parse_effect_fn_opportunity,
            node,
            |tp| tp.parse_effect_fn_opportunity_inner(node).map(Arc::new),
        )
    }
}

// go: internal/typeparser/effect_fn_opportunity.go:92 isEffectFnOpportunityCandidate e3c85bbc8d1032ce
pub(crate) fn is_effect_fn_opportunity_candidate(node: P<Node>) -> bool {
    match node.kind() {
        Kind::FunctionExpression => {
            let f = node.as_function_expression();
            if f.asterisk_token().is_some() || f.name().is_some() {
                return false;
            }
        }
        Kind::FunctionDeclaration => {
            let f = node.as_function_declaration();
            if f.asterisk_token().is_some() {
                return false;
            }
        }
        Kind::ArrowFunction => {}
        _ => return false,
    }
    !has_return_type_annotation(node)
}

impl TypeParser<'_> {
    // go: internal/typeparser/effect_fn_opportunity.go:111 TypeParser.parseEffectFnOpportunityInner b60627b18bd7d78b
    pub(crate) fn parse_effect_fn_opportunity_inner(
        &mut self,
        node: P<Node>,
    ) -> Option<EffectFnOpportunityResult> {
        let mut name_identifier = ets_shim::ast::get_name_of_declaration(node);
        // GetNameOfDeclaration doesn't cover property declaration parents for arrow/function expressions
        if name_identifier.is_none()
            && let Some(parent) = node.parent()
            && parent.kind() == Kind::PropertyDeclaration
        {
            name_identifier = Some(parent.as_property_declaration().name());
        }
        // Also check PropertyAssignment parents (e.g., object literal members like { query: (...) => Effect... })
        if name_identifier.is_none()
            && let Some(parent) = node.parent()
            && parent.kind() == Kind::PropertyAssignment
        {
            let pa = parent.as_property_assignment();
            if pa.initializer() == node {
                name_identifier = Some(pa.name());
            }
        }
        if let Some(name) = name_identifier
            && name.kind() != Kind::Identifier
            && name.kind() != Kind::StringLiteral
        {
            name_identifier = None;
        }
        let name_identifier = name_identifier?;
        let trace_name = get_text_of_node(name_identifier);
        if trace_name.is_empty() {
            return None;
        }

        // Step 5: Reject if already inside Effect.fn
        if self.is_inside_effect_fn(node) {
            return None;
        }

        // Step 6: Get function type, require exactly 1 call signature
        let function_type = self.get_type_at_location(node)?;
        let call_signatures =
            self.checker.get_signatures_of_type_exported(function_type, SignatureKind::Call);
        if call_signatures.len() != 1 {
            return None;
        }

        // Step 7: Get return type, unroll union members, require all are strict Effect types
        let return_type = self.checker.get_return_type_of_signature_exported(call_signatures[0]);
        let union_members = self.unroll_union_members(return_type).unwrap_or_default();
        if union_members.is_empty() {
            return None;
        }
        for member in union_members {
            self.strict_effect_type(member)?;
        }

        // Step 8b: Compute suggestedTraceName (local name) and inferredTraceName (context-aware name)
        let suggested_trace_name = trace_name;
        let inferred_trace_name = self.get_inferred_trace_name(node, &suggested_trace_name);

        // Detect whether the target function is a property value inside a Layer service definition
        let is_layer_member =
            !inferred_trace_name.is_empty() && inferred_trace_name != suggested_trace_name;

        // Converting a hoisted declaration to a const must not invalidate earlier uses.
        // Keep this in the shared parser so neither the diagnostic nor the fix is offered.
        if node.kind() == Kind::FunctionDeclaration
            && self.is_function_referenced_before_declaration(node)
        {
            return None;
        }

        // Step 9: Try gen opportunity first
        if let Some(result) = self.try_parse_gen_opportunity(node) {
            // Safety check: reject if function parameters are referenced in pipe arguments
            if !result.pipe_arguments.is_empty()
                && self.are_parameters_referenced_in(node, &result.pipe_arguments)
            {
                return None;
            }
            return Some(EffectFnOpportunityResult {
                target_node: node,
                name_identifier,
                generator_function: Some(result.generator_function),
                pipe_arguments: Some(result.pipe_arguments),
                explicit_trace_expression: result.explicit_trace_expression,
                suggested_trace_name,
                inferred_trace_name,
                effect_module: result.effect_module,
                has_gen_body: true,
                is_layer_member,
            });
        }

        // Step 10: Try regular opportunity (block body with >5 statements, relaxed in Layer context)
        if try_parse_regular_opportunity(node, is_layer_member) {
            return Some(EffectFnOpportunityResult {
                target_node: node,
                name_identifier,
                generator_function: None,
                pipe_arguments: None,
                explicit_trace_expression: None,
                suggested_trace_name,
                inferred_trace_name,
                effect_module: None,
                has_gen_body: false,
                is_layer_member,
            });
        }

        None
    }

    // go: internal/typeparser/effect_fn_opportunity.go:220 TypeParser.isFunctionReferencedBeforeDeclaration ee07b9dc69275ce2
    // isFunctionReferencedBeforeDeclaration checks the preceding syntax in the
    // declaration's scope. Earlier closures are conservatively included: they may
    // be invoked before the replacement const is initialized.
    pub(crate) fn is_function_referenced_before_declaration(&mut self, node: P<Node>) -> bool {
        let Some(name) = node.name() else {
            return false;
        };
        let Some(mut scope) = node.parent() else {
            return false;
        };
        // Switch clauses share the enclosing case block's lexical scope.
        if scope.kind() == Kind::CaseClause || scope.kind() == Kind::DefaultClause {
            let Some(parent) = scope.parent() else {
                return false;
            };
            scope = parent;
        }
        let Some(symbol) = self.get_symbol_at_location(name) else {
            return false;
        };

        fn visit(
            tp: &mut TypeParser<'_>,
            node: P<Node>,
            name: P<Node>,
            symbol: P<Symbol>,
            current: P<Node>,
        ) -> bool {
            if current.pos() >= node.pos()
                || ets_shim::ast::is_type_node(current)
                || current.kind() == Kind::ExportDeclaration
            {
                return false;
            }
            if current.kind() == Kind::ShorthandPropertyAssignment
                && current.name().is_some_and(|n| n.text() == name.text())
            {
                let value_symbol = tp.checker.get_shorthand_assignment_value_symbol(Some(current));
                if same_symbol_reference(tp.checker, value_symbol, Some(symbol)) {
                    return true;
                }
            }
            if current.kind() == Kind::Identifier && current.text() == name.text() {
                let current_symbol = tp.get_symbol_at_location(current);
                if same_symbol_reference(tp.checker, current_symbol, Some(symbol)) {
                    return true;
                }
            }
            current.for_each_child(&mut |child| visit(tp, node, name, symbol, child))
        }
        scope.for_each_child(&mut |child| visit(self, node, name, symbol, child))
    }
}

// go: internal/typeparser/effect_fn_opportunity.go:258 hasReturnTypeAnnotation 165679d2f5576319
// hasReturnTypeAnnotation checks if a function node has an explicit return type annotation.
pub(crate) fn has_return_type_annotation(node: P<Node>) -> bool {
    match node.kind() {
        Kind::ArrowFunction => node.as_arrow_function().type_().is_some(),
        Kind::FunctionExpression => node.as_function_expression().type_().is_some(),
        Kind::FunctionDeclaration => node.as_function_declaration().type_().is_some(),
        _ => false,
    }
}

// go: internal/typeparser/effect_fn_opportunity.go:280 genOpportunityResult c99580015a7326a7
// genOpportunityResult holds the parsed gen opportunity data.
pub(crate) struct GenOpportunityResult {
    effect_module: Option<P<Node>>,
    generator_function: P<Node>,
    pipe_arguments: Vec<P<Node>>,
    explicit_trace_expression: Option<P<Node>>,
}

impl TypeParser<'_> {
    // go: internal/typeparser/effect_fn_opportunity.go:290 TypeParser.tryParseGenOpportunity b676eceac439dd26
    // tryParseGenOpportunity attempts to parse a function as a gen opportunity.
    // The function body must contain a single return statement (or expression body for arrows)
    // that is an Effect.gen call (possibly piped).
    pub(crate) fn try_parse_gen_opportunity(
        &mut self,
        fn_node: P<Node>,
    ) -> Option<GenOpportunityResult> {
        let body_expr = get_body_expression(fn_node)?;

        // Try to parse as a pipe call first to get subject and pipe args
        let subject;
        let mut outer_pipe_args: &[P<Node>] = &[];

        if let Some(pipe_result) = self.parse_pipe_call(body_expr) {
            subject = pipe_result.subject;
            outer_pipe_args = pipe_result.args;
        } else {
            subject = body_expr;
        }

        // The subject must be an Effect.gen call
        let gen_result = self.effect_gen_call(subject)?;

        // with no this binding
        if gen_result.options_node.is_some() {
            return None;
        }
        let mut pipe_args: Vec<P<Node>> = Vec::new();
        pipe_args.extend_from_slice(gen_result.pipe_arguments.unwrap_or_default());
        pipe_args.extend_from_slice(outer_pipe_args);
        for arg in &pipe_args {
            // A spread can contain bare pipeables, but cannot itself be wrapped as
            // a unary callback. Keep the original pipe call in this case.
            if arg.kind() == Kind::SpreadElement {
                return None;
            }
        }

        // Check if the last pipe argument is Effect.withSpan
        let mut explicit_trace_expression = None;
        if let Some(&last_arg) = pipe_args.last()
            && let Some(with_span_expr) = self.try_extract_with_span_expression(Some(last_arg))
        {
            explicit_trace_expression = Some(with_span_expr);
        }

        Some(GenOpportunityResult {
            effect_module: gen_result.effect_module,
            generator_function: gen_result.generator_function,
            pipe_arguments: pipe_args,
            explicit_trace_expression,
        })
    }
}

// go: internal/typeparser/effect_fn_opportunity.go:347 getBodyExpression c10fdf79c8fd1f7b
// getBodyExpression gets the single return expression from a function body.
// For arrow functions with expression bodies, returns the expression directly.
// For block bodies, requires exactly one return statement.
pub(crate) fn get_body_expression(fn_node: P<Node>) -> Option<P<Node>> {
    match fn_node.kind() {
        Kind::ArrowFunction => {
            let body = fn_node.as_arrow_function().body()?;
            if body.kind() == Kind::Block {
                return find_single_return_expression(Some(body));
            }
            Some(body)
        }
        Kind::FunctionExpression => {
            find_single_return_expression(fn_node.as_function_expression().body())
        }
        Kind::FunctionDeclaration => {
            find_single_return_expression(fn_node.as_function_declaration().body())
        }
        _ => None,
    }
}

// go: internal/typeparser/effect_fn_opportunity.go:377 findSingleReturnExpression e81477af83a2f18a
// findSingleReturnExpression finds the expression from a single return statement in a block.
pub(crate) fn find_single_return_expression(body: Option<P<Node>>) -> Option<P<Node>> {
    let body = body?;
    if body.kind() != Kind::Block {
        return None;
    }
    let statements = body.as_block().statements().nodes();
    if statements.len() != 1 {
        return None;
    }
    let stmt = statements[0];
    if stmt.kind() != Kind::ReturnStatement {
        return None;
    }
    stmt.as_return_statement().expression()
}

// go: internal/typeparser/effect_fn_opportunity.go:395 tryParseRegularOpportunity bff43e98b7a38a81
// tryParseRegularOpportunity checks if a function has a block body with more than 5 statements.
// When hasStrictLayerInferredName is true, the >5 statement requirement and the block body
// requirement for arrow functions are relaxed (any Effect-returning function in Layer context qualifies).
pub(crate) fn try_parse_regular_opportunity(
    fn_node: P<Node>,
    has_strict_layer_inferred_name: bool,
) -> bool {
    match fn_node.kind() {
        Kind::ArrowFunction => {
            let Some(body) = fn_node.as_arrow_function().body() else {
                return false;
            };
            // Arrow with concise body (expression, no block): only allowed in Layer context
            if body.kind() != Kind::Block {
                return has_strict_layer_inferred_name;
            }
            if has_strict_layer_inferred_name {
                return true;
            }
            body.as_block().statements().nodes().len() > 5
        }
        Kind::FunctionExpression => {
            let Some(body) = fn_node.as_function_expression().body() else {
                return false;
            };
            if body.kind() != Kind::Block {
                return false;
            }
            if has_strict_layer_inferred_name {
                return true;
            }
            body.as_block().statements().nodes().len() > 5
        }
        Kind::FunctionDeclaration => {
            let Some(body) = fn_node.as_function_declaration().body() else {
                return false;
            };
            if body.kind() != Kind::Block {
                return false;
            }
            if has_strict_layer_inferred_name {
                return true;
            }
            body.as_block().statements().nodes().len() > 5
        }
        _ => false,
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/effect_fn_opportunity.go:439 TypeParser.tryExtractWithSpanExpression 81996f20119ac4ff
    // tryExtractWithSpanExpression checks if an expression is a call to Effect.withSpan
    // and extracts the span name expression (the first argument).
    pub(crate) fn try_extract_with_span_expression(
        &mut self,
        expr: Option<P<Node>>,
    ) -> Option<P<Node>> {
        let expr = expr?;
        if expr.kind() != Kind::CallExpression {
            return None;
        }

        let call = expr.as_call_expression();

        // withSpan has at least one argument (the span name)
        let arguments = call.arguments().nodes();
        if arguments.is_empty() {
            return None;
        }

        if !self.is_node_reference_to_effect_module_api(call.expression(), "withSpan") {
            return None;
        }

        Some(arguments[0])
    }

    // go: internal/typeparser/effect_fn_opportunity.go:463 TypeParser.areParametersReferencedIn 02355aee337b643b
    // areParametersReferencedIn checks if any of the function's parameter symbols
    // are referenced within the given nodes. Uses declaration position range checking.
    pub(crate) fn are_parameters_referenced_in(
        &mut self,
        fn_node: P<Node>,
        nodes: &[P<Node>],
    ) -> bool {
        if nodes.is_empty() {
            return false;
        }

        let params = get_function_parameters(fn_node);
        if params.is_empty() {
            return false;
        }

        // Get the position range of all parameters
        let first_param = params[0];
        let last_param = params[params.len() - 1];
        let params_start = first_param.pos();
        let params_end = last_param.end();

        // Walk all nodes looking for symbols declared in the function parameters
        let mut queue: std::collections::VecDeque<P<Node>> = std::collections::VecDeque::new();
        queue.extend(nodes.iter().copied());

        while let Some(current) = queue.pop_front() {
            // Check identifiers
            if current.kind() == Kind::Identifier
                && let Some(sym) = self.get_symbol_at_location(current)
                && is_symbol_declared_in_range(sym, params_start, params_end)
            {
                return true;
            }

            // Check shorthand property assignments like { a, b }
            if current.kind() == Kind::ShorthandPropertyAssignment
                && let Some(value_sym) =
                    self.checker.get_shorthand_assignment_value_symbol(Some(current))
                && is_symbol_declared_in_range(value_sym, params_start, params_end)
            {
                return true;
            }

            current.for_each_child(&mut |child| {
                queue.push_back(child);
                false
            });
        }

        false
    }
}

// go: internal/typeparser/effect_fn_opportunity.go:522 isSymbolDeclaredInRange c5d21a16957b8807
// isSymbolDeclaredInRange checks if any of the symbol's declarations fall within the given range.
pub(crate) fn is_symbol_declared_in_range(sym: P<Symbol>, start: TextPos, end: TextPos) -> bool {
    for decl in sym.declarations() {
        if decl.pos() >= start && decl.end() <= end {
            return true;
        }
    }
    false
}

// go: internal/typeparser/effect_fn_opportunity.go:532 getFunctionParameters f6b0b43f3377e49f
// getFunctionParameters returns the parameter nodes of a function.
pub(crate) fn get_function_parameters(fn_node: P<Node>) -> &'static [P<Node>] {
    let parameters = match fn_node.kind() {
        Kind::ArrowFunction => fn_node.as_arrow_function().parameters(),
        Kind::FunctionExpression => fn_node.as_function_expression().parameters(),
        Kind::FunctionDeclaration => fn_node.as_function_declaration().parameters(),
        _ => None,
    };
    parameters.map_or(&[], |parameters| parameters.nodes())
}

impl TypeParser<'_> {
    // go: internal/typeparser/effect_fn_opportunity.go:555 TypeParser.tryGetLayerApiMethod 3c388754aa3a24a0
    // tryGetLayerApiMethod checks if a node references Layer.effect, Layer.succeed, or Layer.sync.
    // Returns the method name ("effect", "succeed", "sync") or empty string.
    pub(crate) fn try_get_layer_api_method(&mut self, node: P<Node>) -> &'static str {
        if self.is_node_reference_to_effect_layer_module_api(node, "effect") {
            return "effect";
        }
        if self.is_node_reference_to_effect_layer_module_api(node, "succeed") {
            return "succeed";
        }
        if self.is_node_reference_to_effect_layer_module_api(node, "sync") {
            return "sync";
        }
        ""
    }
}

// go: internal/typeparser/effect_fn_opportunity.go:574 layerServiceNameFromExpression b87d391426119c29
// layerServiceNameFromExpression resolves an expression to a service name string.
// When the expression is the `this` keyword, it walks up the AST to find the enclosing
// class declaration and returns the class name instead of literal "this".
pub(crate) fn layer_service_name_from_expression(expr: P<Node>) -> String {
    if expr.kind() == Kind::ThisKeyword
        && let Some(class_decl) = ets_shim::ast::find_ancestor_kind(expr, Kind::ClassDeclaration)
        && let Some(name) = class_decl.name()
    {
        return get_text_of_node(name);
    }
    get_text_of_node(expr)
}

impl TypeParser<'_> {
    // go: internal/typeparser/effect_fn_opportunity.go:591 TypeParser.verifyLayerMethodAtCall 67ead27cc2e4b039
    // verifyLayerMethodAtCall checks if a call expression is a Layer method call (direct or curried)
    // and returns the tag text (e.g., "MyService") or empty string.
    // Direct form: Layer.method(tag, impl) where impl === implementationExpr
    // Curried form: Layer.method(tag)(impl) where impl === implementationExpr
    pub(crate) fn verify_layer_method_at_call(
        &mut self,
        call_expr: P<Node>,
        method: &str,
        implementation_expr: P<Node>,
    ) -> String {
        let call = call_expr.as_call_expression();
        let arguments = call.arguments().nodes();

        // Check direct form: Layer.method(tag, impl)
        if arguments.len() >= 2
            && arguments[1] == implementation_expr
            && self.try_get_layer_api_method(call.expression()) == method
        {
            return layer_service_name_from_expression(arguments[0]);
        }

        // Check curried form: Layer.method(tag)(impl)
        if call.expression().kind() == Kind::CallExpression {
            let inner_call = call.expression().as_call_expression();
            let inner_arguments = inner_call.arguments().nodes();
            if !inner_arguments.is_empty()
                && !arguments.is_empty()
                && arguments[0] == implementation_expr
                && self.try_get_layer_api_method(inner_call.expression()) == method
            {
                return layer_service_name_from_expression(inner_arguments[0]);
            }
        }

        String::new()
    }

    // go: internal/typeparser/effect_fn_opportunity.go:619 TypeParser.tryMatchLayerSucceedInference db8611b77e88f2cb
    // tryMatchLayerSucceedInference checks if an object literal is a direct argument to Layer.succeed.
    // Returns the service tag text or empty string.
    pub(crate) fn try_match_layer_succeed_inference(&mut self, object_literal: P<Node>) -> String {
        let Some(parent) = object_literal.parent() else {
            return String::new();
        };
        if parent.kind() != Kind::CallExpression {
            return String::new();
        }
        self.verify_layer_method_at_call(parent, "succeed", object_literal)
    }

    // go: internal/typeparser/effect_fn_opportunity.go:633 TypeParser.tryMatchLayerSyncInference 4a22f67222a99e3e
    // tryMatchLayerSyncInference checks if an object literal is returned from a lazy function
    // that is passed to Layer.sync.
    // Pattern: Layer.sync(tag)(() => { return { ... } }) or Layer.sync(tag, () => { return { ... } })
    pub(crate) fn try_match_layer_sync_inference(&mut self, object_literal: P<Node>) -> String {
        // objectLiteral -> ReturnStatement
        let Some(return_stmt) = object_literal.parent() else {
            return String::new();
        };
        if return_stmt.kind() != Kind::ReturnStatement {
            return String::new();
        }
        // ReturnStatement -> Block
        let Some(block) = return_stmt.parent() else {
            return String::new();
        };
        if block.kind() != Kind::Block {
            return String::new();
        }
        // Block -> ArrowFunction or FunctionExpression (the lazy function)
        let Some(lazy_fn) = block.parent() else {
            return String::new();
        };
        if lazy_fn.kind() != Kind::ArrowFunction && lazy_fn.kind() != Kind::FunctionExpression {
            return String::new();
        }
        // LazyFunction -> CallExpression
        let Some(call_node) = lazy_fn.parent() else {
            return String::new();
        };
        if call_node.kind() != Kind::CallExpression {
            return String::new();
        }
        self.verify_layer_method_at_call(call_node, "sync", lazy_fn)
    }

    // go: internal/typeparser/effect_fn_opportunity.go:667 TypeParser.tryMatchLayerEffectInference 89e701d93e7ef3b8
    // tryMatchLayerEffectInference checks if an object literal is returned from a generator
    // inside Effect.gen that is passed to Layer.effect.
    // Pattern: Layer.effect(tag)(Effect.gen(function*() { return { ... } }))
    pub(crate) fn try_match_layer_effect_inference(&mut self, object_literal: P<Node>) -> String {
        // objectLiteral -> ReturnStatement
        let Some(return_stmt) = object_literal.parent() else {
            return String::new();
        };
        if return_stmt.kind() != Kind::ReturnStatement {
            return String::new();
        }
        // ReturnStatement -> Block (generator body)
        let Some(gen_body) = return_stmt.parent() else {
            return String::new();
        };
        if gen_body.kind() != Kind::Block {
            return String::new();
        }
        // Block -> FunctionExpression (generator function, must have asteriskToken)
        let Some(gen_fn_node) = gen_body.parent() else {
            return String::new();
        };
        if gen_fn_node.kind() != Kind::FunctionExpression {
            return String::new();
        }
        if gen_fn_node.as_function_expression().asterisk_token().is_none() {
            return String::new();
        }
        // GeneratorFunction -> CallExpression (Effect.gen call)
        let Some(gen_call_node) = gen_fn_node.parent() else {
            return String::new();
        };
        if gen_call_node.kind() != Kind::CallExpression {
            return String::new();
        }
        // Effect.gen(...) -> CallExpression (Layer.effect call)
        let Some(layer_call_node) = gen_call_node.parent() else {
            return String::new();
        };
        if layer_call_node.kind() != Kind::CallExpression {
            return String::new();
        }
        // Verify this is actually an Effect.gen call with our generator
        let Some(parsed_gen) = self.effect_gen_call(gen_call_node) else {
            return String::new();
        };
        if parsed_gen.generator_function != gen_fn_node {
            return String::new();
        }
        self.verify_layer_method_at_call(layer_call_node, "effect", gen_call_node)
    }

    // go: internal/typeparser/effect_fn_opportunity.go:714 TypeParser.tryGetLayerInferredTraceName 9a3c0d8db18befc2
    // tryGetLayerInferredTraceName checks if a function is inside a property assignment within
    // an object literal that is a Layer service definition, returning "ServiceTag.memberName" format.
    pub(crate) fn try_get_layer_inferred_trace_name(
        &mut self,
        node: P<Node>,
        suggested_trace_name: &str,
    ) -> String {
        if suggested_trace_name.is_empty() {
            return String::new();
        }
        let Some(parent) = node.parent() else {
            return String::new();
        };
        // The function must be the initializer of a PropertyAssignment
        if parent.kind() != Kind::PropertyAssignment {
            return String::new();
        }
        let pa = parent.as_property_assignment();
        if pa.initializer() != node {
            return String::new();
        }
        // The PropertyAssignment must be inside an ObjectLiteralExpression
        let Some(object_literal) = parent.parent() else {
            return String::new();
        };
        if object_literal.kind() != Kind::ObjectLiteralExpression {
            return String::new();
        }

        // Try each Layer pattern in order
        let service_name = self.try_match_layer_succeed_inference(object_literal);
        if !service_name.is_empty() {
            return service_name + "." + suggested_trace_name;
        }
        let service_name = self.try_match_layer_sync_inference(object_literal);
        if !service_name.is_empty() {
            return service_name + "." + suggested_trace_name;
        }
        let service_name = self.try_match_layer_effect_inference(object_literal);
        if !service_name.is_empty() {
            return service_name + "." + suggested_trace_name;
        }
        let service_name = self.try_match_of_inference(object_literal);
        if !service_name.is_empty() {
            return service_name + "." + suggested_trace_name;
        }
        let service_name = self.try_match_service_map_make_inference(object_literal);
        if !service_name.is_empty() {
            return service_name + "." + suggested_trace_name;
        }
        String::new()
    }

    // go: internal/typeparser/effect_fn_opportunity.go:754 TypeParser.tryMatchOfInference 80ea3f8360ab9cd6
    // tryMatchOfInference checks if an object literal is the first argument to a Service.of({ ... }) call,
    // where the service expression is a ContextTag or ServiceType.
    // Returns the service tag text or empty string.
    pub(crate) fn try_match_of_inference(&mut self, object_literal: P<Node>) -> String {
        let Some(parent) = object_literal.parent() else {
            return String::new();
        };
        if parent.kind() != Kind::CallExpression {
            return String::new();
        }
        let call_expr = parent.as_call_expression();
        let arguments = call_expr.arguments().nodes();
        if arguments.is_empty() {
            return String::new();
        }
        // objectLiteral must be the first argument
        if arguments[0] != object_literal {
            return String::new();
        }
        // The call expression must be a PropertyAccessExpression with name "of"
        if call_expr.expression().kind() != Kind::PropertyAccessExpression {
            return String::new();
        }
        let prop_access = call_expr.expression().as_property_access_expression();
        if get_text_of_node(prop_access.name()) != "of" {
            return String::new();
        }
        // Get the service tag expression (the object before .of)
        let service_tag_expression = prop_access.expression();
        if self.node_cannot_be_effect_library_type(service_tag_expression) {
            return String::new();
        }
        let Some(service_tag_type) = self.get_type_at_location(service_tag_expression) else {
            return String::new();
        };
        if !self.is_context_tag(service_tag_type) && !self.is_service_type(service_tag_type) {
            return String::new();
        }
        layer_service_name_from_expression(service_tag_expression)
    }

    // go: internal/typeparser/effect_fn_opportunity.go:795 TypeParser.tryMatchServiceMapMakeInference 5ea9ac1a17972475
    // tryMatchServiceMapMakeInference checks if an object literal is returned from a generator
    // inside Effect.gen that is the "make" property of a class extending Context.Service.
    // Returns the class name or empty string.
    pub(crate) fn try_match_service_map_make_inference(
        &mut self,
        object_literal: P<Node>,
    ) -> String {
        // objectLiteral -> ReturnStatement
        let Some(return_stmt) = object_literal.parent() else {
            return String::new();
        };
        if return_stmt.kind() != Kind::ReturnStatement {
            return String::new();
        }
        // ReturnStatement -> Block (generator body)
        let Some(gen_body) = return_stmt.parent() else {
            return String::new();
        };
        if gen_body.kind() != Kind::Block {
            return String::new();
        }
        // Block -> FunctionExpression (generator function, must have asteriskToken)
        let Some(gen_fn_node) = gen_body.parent() else {
            return String::new();
        };
        if gen_fn_node.kind() != Kind::FunctionExpression {
            return String::new();
        }
        if gen_fn_node.as_function_expression().asterisk_token().is_none() {
            return String::new();
        }
        // GeneratorFunction -> CallExpression (Effect.gen call)
        let Some(gen_call_node) = gen_fn_node.parent() else {
            return String::new();
        };
        if gen_call_node.kind() != Kind::CallExpression {
            return String::new();
        }
        // Effect.gen(...) -> PropertyAssignment with name "make" and initializer == genCall
        let Some(make_property) = gen_call_node.parent() else {
            return String::new();
        };
        if make_property.kind() != Kind::PropertyAssignment {
            return String::new();
        }
        let pa = make_property.as_property_assignment();
        if pa.initializer() != gen_call_node {
            return String::new();
        }
        if pa.name().kind() != Kind::Identifier || get_text_of_node(pa.name()) != "make" {
            return String::new();
        }
        // Walk ancestors from PropertyAssignment.Parent to find a ClassDeclaration
        let mut current_node = make_property.parent();
        while let Some(current) = current_node {
            if current.kind() == Kind::ClassDeclaration {
                break;
            }
            current_node = current.parent();
        }
        let Some(current_node) = current_node else {
            return String::new();
        };
        let Some(class_name) = current_node.name() else {
            return String::new();
        };
        // Verify this is actually an Effect.gen call with our generator
        let Some(parsed_gen) = self.effect_gen_call(gen_call_node) else {
            return String::new();
        };
        if parsed_gen.generator_function != gen_fn_node {
            return String::new();
        }
        // Verify the class extends Context.Service
        if self.extends_context_service(current_node).is_none() {
            return String::new();
        }
        get_text_of_node(class_name)
    }

    // go: internal/typeparser/effect_fn_opportunity.go:864 TypeParser.getInferredTraceName ec558d4318ba96ab
    // getInferredTraceName computes a context-aware inferred trace name for a function.
    // It checks (in priority order):
    // 1. Layer service context (ServiceTag.memberName)
    // 2. Exported function declarations — returns the function name
    // 3. Exported const variable initializers — returns the variable name
    // Returns "" if no context-aware name can be inferred.
    pub(crate) fn get_inferred_trace_name(
        &mut self,
        node: P<Node>,
        suggested_trace_name: &str,
    ) -> String {
        if suggested_trace_name.is_empty() {
            return String::new();
        }

        // Layer-based inferred trace name takes priority
        let inferred_from_layer =
            self.try_get_layer_inferred_trace_name(node, suggested_trace_name);
        if !inferred_from_layer.is_empty() {
            return inferred_from_layer;
        }

        // Check exported function declaration
        if node.kind() == Kind::FunctionDeclaration
            && ets_shim::ast::has_syntactic_modifier(node, ModifierFlags::Export)
        {
            return suggested_trace_name.to_string();
        }

        // Check exported const variable initializer
        if let Some(parent) = node.parent()
            && parent.kind() == Kind::VariableDeclaration
        {
            let vd = parent.as_variable_declaration();
            if vd.initializer() == Some(node) && vd.name().kind() == Kind::Identifier {
                // Walk up: VariableDeclaration -> VariableDeclarationList -> VariableStatement
                if let Some(decl_list) = parent.parent()
                    && decl_list.kind() == Kind::VariableDeclarationList
                    && let Some(var_stmt) = decl_list.parent()
                    && var_stmt.kind() == Kind::VariableStatement
                    && ets_shim::ast::has_syntactic_modifier(var_stmt, ModifierFlags::Export)
                    && decl_list.flags().intersects(NodeFlags::Const)
                {
                    return suggested_trace_name.to_string();
                }
            }
        }

        String::new()
    }
}
