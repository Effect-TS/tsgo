use std::collections::VecDeque;
use std::sync::Arc;

use ets_shim::ast::{Kind, Node, SourceFile};
use ets_shim::checker::{ContextFlags, SignatureKind, Type};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/expected_and_real_type.go:13 ExpectedAndRealType
// ExpectedAndRealType represents a pair of expected and actual types at an assignment site.
// This is used by diagnostic rules that need to compare what type was expected at a location
// versus what type was actually provided.
pub struct ExpectedAndRealType {
    pub(crate) node: P<Node>, // The location node (for diagnostic reporting)
    pub(crate) expected_type: Option<Lazy<Option<P<Type>>>>, // The type expected at this location
    pub(crate) value_node: P<Node>, // The actual value node
    pub(crate) real_node: P<Node>, // The node whose type is the real type
    pub(crate) real_type: Option<Lazy<Option<P<Type>>>>, // The actual type of the value
}

impl ExpectedAndRealType {
    // go: internal/typeparser/expected_and_real_type.go:21 ExpectedAndRealType.Node
    pub fn node(&self) -> P<Node> {
        self.node
    }

    // go: internal/typeparser/expected_and_real_type.go:23 ExpectedAndRealType.ExpectedType
    // Rust: the lazy resolves through `tp` (see `Lazy`).
    pub fn expected_type(&self, tp: &mut TypeParser<'_>) -> Option<P<Type>> {
        Lazy::get(self.expected_type.as_ref(), tp)
    }

    // go: internal/typeparser/expected_and_real_type.go:25 ExpectedAndRealType.ValueNode
    pub fn value_node(&self) -> P<Node> {
        self.value_node
    }

    // go: internal/typeparser/expected_and_real_type.go:27 ExpectedAndRealType.RealNode
    pub fn real_node(&self) -> P<Node> {
        self.real_node
    }

    // go: internal/typeparser/expected_and_real_type.go:29 ExpectedAndRealType.RealType
    // Rust: the lazy resolves through `tp` (see `Lazy`).
    pub fn real_type(&self, tp: &mut TypeParser<'_>) -> Option<P<Type>> {
        Lazy::get(self.real_type.as_ref(), tp)
    }
}

impl TypeParser<'_> {
    // go: internal/typeparser/expected_and_real_type.go:34 TypeParser.getInferredReturnType
    // getInferredReturnType extracts the return type from a function-like declaration.
    // It handles overloaded functions (multiple call signatures), type predicates,
    // and regular signature return types.
    pub(crate) fn get_inferred_return_type(&mut self, declaration: P<Node>) -> Option<P<Type>> {
        // Check that the declaration has a body
        let body_data = declaration.body_data()?;
        body_data.body?;

        let mut return_type = None;

        // Try overloaded function handling:
        // Get the type at the declaration location, get call signatures,
        // and if there are multiple signatures, union their return types.
        if let Some(decl_type) = self.get_type_at_location(declaration) {
            let signatures =
                self.checker.get_signatures_of_type_exported(decl_type, SignatureKind::Call);
            if signatures.len() > 1 {
                let mut return_types = Vec::new();
                for &sig in signatures {
                    return_types.push(self.checker.get_return_type_of_signature_exported(sig));
                }
                if !return_types.is_empty() {
                    return_type = Some(self.checker.get_union_type_exported(&return_types));
                }
            }
        }

        if return_type.is_none() {
            // Go: GetSignatureFromDeclaration never returns nil for a function-like declaration.
            let sig = self.checker.get_signature_from_declaration_exported(declaration);
            if let Some(type_predicate) = self.checker.get_type_predicate_of_signature_exported(sig)
                && let Some(t) = type_predicate.type_()
            {
                return Some(t);
            }
            return_type = Some(self.checker.get_return_type_of_signature_exported(sig));
        }

        return_type
    }

    // go: internal/typeparser/expected_and_real_type.go:94 TypeParser.ExpectedAndRealTypes
    // ExpectedAndRealTypes walks the AST of a source file using breadth-first traversal
    // and collects pairs of expected vs actual types at assignment sites.
    //
    // It recognizes 8 assignment site patterns:
    // 1. Variable declaration with initializer (const a: T = expr)
    // 2. Call expression arguments (fn(a))
    // 3. Object literal property keys ({ key: expr } as { key: T })
    // 4. Binary assignment (a = expr)
    // 5. Return statement (return expr)
    // 6. Arrow function body without type params ((): T => expr)
    // 7. Arrow function body with type params (<A>(): T => expr)
    // 8. Satisfies expression (expr satisfies T)
    //
    // Rust: Go returns a read-only view of the cached slice; here a shared handle to it (`None` is a nil slice), as
    // `DiscoverPackages` does.
    pub fn expected_and_real_types(
        &mut self,
        sf: impl Into<Option<P<SourceFile>>>,
    ) -> Option<Arc<[ExpectedAndRealType]>> {
        let sf = sf.into()?;

        cached(
            self,
            |l| &mut l.expected_and_real_types,
            sf,
            |tp| tp.collect_expected_and_real_types(sf).map(Arc::from),
        )
    }

    // go: internal/typeparser/expected_and_real_type.go:94 TypeParser.ExpectedAndRealTypes
    // Rust: the cached computation of `ExpectedAndRealTypes` (a Go func literal).
    fn collect_expected_and_real_types(
        &mut self,
        sf: P<SourceFile>,
    ) -> Option<Vec<ExpectedAndRealType>> {
        let mut result: Option<Vec<ExpectedAndRealType>> = None;
        let mut push = |item: ExpectedAndRealType| result.get_or_insert_with(Vec::new).push(item);

        // Initialize BFS queue with the source file node
        let mut queue: VecDeque<P<Node>> = VecDeque::new();
        queue.push_back(sf.as_node());
        fn enqueue_children(queue: &mut VecDeque<P<Node>>, node: P<Node>) {
            node.for_each_child(&mut |child| {
                queue.push_back(child);
                false
            });
        }

        // Dequeue from front (FIFO/breadth-first) to match TypeScript's shift() behavior
        while let Some(node) = queue.pop_front() {
            // Pattern 1: Variable declaration with initializer
            if node.kind() == Kind::VariableDeclaration {
                let vd = node.as_variable_declaration();
                if let Some(initializer) = vd.initializer() {
                    let name_node = vd.name();
                    push(ExpectedAndRealType {
                        node: name_node,
                        expected_type: Some(self.lazy_type_at_location(name_node)),
                        value_node: initializer,
                        real_node: initializer,
                        real_type: Some(self.lazy_type_at_location(initializer)),
                    });
                    queue.push_back(initializer);
                    continue;
                }
            }

            // Pattern 2: Call expression arguments
            if node.kind() == Kind::CallExpression {
                let call = node.as_call_expression();
                let arguments = call.arguments().nodes();
                // Go: GetResolvedSignature never returns nil.
                let resolved_sig = self.checker.get_resolved_signature_exported(node);
                let params = resolved_sig.parameters();
                for (i, &param) in params.iter().enumerate() {
                    if i >= arguments.len() {
                        break;
                    }
                    let arg = arguments[i];
                    push(ExpectedAndRealType {
                        node: arg,
                        expected_type: Some(new_lazy(move |tp| {
                            tp.checker.get_type_of_symbol_at_location(param, Some(node))
                        })),
                        value_node: arg,
                        real_node: arg,
                        real_type: Some(self.lazy_type_at_location(arg)),
                    });
                }
                enqueue_children(&mut queue, node);
                continue;
            }

            // Pattern 3: Object literal property keys
            if node.kind() == Kind::Identifier
                || node.kind() == Kind::StringLiteral
                || node.kind() == Kind::NumericLiteral
                || node.kind() == Kind::NoSubstitutionTemplateLiteral
            {
                if let Some(parent) = node.parent()
                    && ets_shim::ast::is_object_literal_element(parent)
                    && let Some(grandparent) = parent.parent()
                    && grandparent.kind() == Kind::ObjectLiteralExpression
                {
                    // Check that this node is the name of the property (not the value)
                    let name_node = get_object_literal_element_name(Some(parent));
                    if name_node == Some(node) {
                        let name = get_node_text_for_property_lookup(Some(node));
                        if !name.is_empty()
                            && let Some(contextual_type) =
                                self.get_contextual_type(grandparent, ContextFlags::None)
                            && let Some(sym) =
                                self.checker.get_property_of_type_exported(contextual_type, &name)
                        {
                            push(ExpectedAndRealType {
                                node,
                                expected_type: Some(new_lazy(move |tp| {
                                    tp.checker.get_type_of_symbol_at_location(sym, Some(node))
                                })),
                                value_node: node,
                                real_node: node,
                                real_type: Some(self.lazy_type_at_location(node)),
                            });
                        }
                    }
                }
                enqueue_children(&mut queue, node);
                continue;
            }

            // Pattern 4: Binary assignment (a = expr)
            if node.kind() == Kind::BinaryExpression {
                let bin_expr = node.as_binary_expression();
                if bin_expr.operator_token().kind() == Kind::EqualsToken {
                    let left = bin_expr.left();
                    let right = bin_expr.right();
                    push(ExpectedAndRealType {
                        node: left,
                        expected_type: Some(self.lazy_type_at_location(left)),
                        value_node: right,
                        real_node: right,
                        real_type: Some(self.lazy_type_at_location(right)),
                    });
                    queue.push_back(right);
                    continue;
                }
            }

            // Pattern 5: Return statement
            if node.kind() == Kind::ReturnStatement {
                let ret_stmt = node.as_return_statement();
                if let Some(expression) = ret_stmt.expression()
                    && let Some(parent_decl) = ets_shim::ast::get_containing_function(node)
                    && let Some(expected_type) = self.get_inferred_return_type(parent_decl)
                {
                    push(ExpectedAndRealType {
                        node,
                        expected_type: Some(resolved_lazy(Some(expected_type))),
                        value_node: node,
                        real_node: expression,
                        real_type: Some(self.lazy_type_at_location(expression)),
                    });
                }
                enqueue_children(&mut queue, node);
                continue;
            }

            // Pattern 6 & 7: Arrow function body (expression body)
            if node.kind() == Kind::ArrowFunction {
                let f = node.as_arrow_function();
                if let Some(body) = f.body()
                    && body.kind() != Kind::Block
                {
                    let has_type_params =
                        f.type_parameters().is_some_and(|tps| !tps.nodes().is_empty());

                    if !has_type_params {
                        // Pattern 6: No type parameters — use contextual type
                        if let Some(expected_type) =
                            self.get_contextual_type(body, ContextFlags::None)
                        {
                            push(ExpectedAndRealType {
                                node: body,
                                expected_type: Some(resolved_lazy(Some(expected_type))),
                                value_node: body,
                                real_node: body,
                                real_type: Some(self.lazy_type_at_location(body)),
                            });
                        }
                    } else {
                        // Pattern 7: With type parameters — use inferred return type
                        if let Some(expected_type) = self.get_inferred_return_type(node) {
                            push(ExpectedAndRealType {
                                node: body,
                                expected_type: Some(resolved_lazy(Some(expected_type))),
                                value_node: body,
                                real_node: body,
                                real_type: Some(self.lazy_type_at_location(body)),
                            });
                        }
                    }
                    enqueue_children(&mut queue, body);
                    continue;
                }
            }

            // Pattern 8: Satisfies expression
            if node.kind() == Kind::SatisfiesExpression {
                let sat_expr = node.as_satisfies_expression();
                let expression = sat_expr.expression();
                push(ExpectedAndRealType {
                    node: expression,
                    expected_type: Some(self.lazy_type_at_location(sat_expr.type_())),
                    value_node: expression,
                    real_node: expression,
                    real_type: Some(self.lazy_type_at_location(expression)),
                });
                queue.push_back(expression);
                continue;
            }

            // No pattern matched — queue all children for traversal
            enqueue_children(&mut queue, node);
        }

        result
    }
}

// go: internal/typeparser/expected_and_real_type.go:308 getObjectLiteralElementName
// getObjectLiteralElementName returns the name node of an object literal element
// (PropertyAssignment, ShorthandPropertyAssignment, etc.), or nil.
pub(crate) fn get_object_literal_element_name(node: Option<P<Node>>) -> Option<P<Node>> {
    let node = node?;
    match node.kind() {
        Kind::PropertyAssignment => Some(node.as_property_assignment().name()),
        Kind::ShorthandPropertyAssignment => Some(node.as_shorthand_property_assignment().name()),
        _ => None,
    }
}

// go: internal/typeparser/expected_and_real_type.go:329 getNodeTextForPropertyLookup
// getNodeTextForPropertyLookup extracts the text from an identifier or string literal
// node for use in property type lookup.
pub(crate) fn get_node_text_for_property_lookup(node: Option<P<Node>>) -> String {
    let Some(node) = node else {
        return String::new();
    };
    match node.kind() {
        Kind::Identifier => ets_shim::scanner::get_text_of_node(node),
        Kind::StringLiteral => node.as_string_literal().text().to_string(),
        _ => String::new(),
    }
}
