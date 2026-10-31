use std::cell::Cell;
use std::rc::Rc;

use ets_shim::ast::{
    self, CallExpression, CommentRange, Kind, Node, NodeFactory, NodeFlags, NodeList, NodeVisitor,
    NodeVisitorHooks, SourceFile,
};
use ets_shim::astnav;
use ets_shim::core::{P, TextRange};
use ets_shim::scanner;
use ets_typeparser::{PipingFlow, PipingFlowTransformation, TransformationKind};
use rustc_hash::FxHashMap;

use crate::*;

// go: internal/rewriter/piping_flow.go:17 PipingFlowTransformationReplacement 7fea11d78d091f40
// PipingFlowTransformationReplacement describes a replacement operator in its
// normalized, pipeable form. A nil Arguments list represents a bare operator;
// a non-nil list represents a call, including an explicit zero-argument call.
#[derive(Clone, Copy, Default)]
pub struct PipingFlowTransformationReplacement {
    pub callee: Option<P<Node>>,
    pub type_arguments: Option<P<NodeList>>,
    pub arguments: Option<P<NodeList>>,
}

/// Rust-only: the message of the parsed-node position reads below.
const PARSED_NODE: &str = "a piping flow argument is a parsed node";

impl Tracker {
    // go: internal/rewriter/piping_flow.go:28 Tracker.ReplacePipingFlowPrefix 535001474c35ec88
    // ReplacePipingFlowPrefix replaces the subject and the requested leading
    // transformations with a complete replacement expression while retaining any
    // transformations that follow the prefix. It edits nothing and returns false
    // when the prefix ends in an Effect.fn step, whose subject is a function body
    // rather than an expression.
    //
    // Rust: Go's nil receiver check has no counterpart (`&mut self`).
    pub fn replace_piping_flow_prefix(
        &mut self,
        source_file: Option<P<SourceFile>>,
        flow: Option<&PipingFlow>,
        transformation_count: i64,
        replacement_subject: Option<P<Node>>,
    ) -> bool {
        let (Some(source_file), Some(flow), Some(replacement_subject)) =
            (source_file, flow, replacement_subject)
        else {
            return false;
        };
        if transformation_count <= 0 || transformation_count > flow.transformations().len() {
            return false;
        }

        let last = flow.transformations().at(transformation_count - 1);
        let transformation_node = piping_flow_transformation_node(last);
        let target = match last.kind() {
            TransformationKind::Call
            | TransformationKind::DataFirst
            | TransformationKind::DataLast
            | TransformationKind::CallWithOptionals => transformation_node,
            TransformationKind::Pipe => {
                return self.replace_function_pipe_prefix(
                    source_file,
                    transformation_node,
                    replacement_subject,
                );
            }
            TransformationKind::Pipeable => {
                return self.replace_method_pipe_prefix(
                    source_file,
                    transformation_node,
                    replacement_subject,
                );
            }
            _ => return false,
        };
        let Some(target) = target else {
            return false;
        };

        ast::set_parent_in_children(replacement_subject);
        self.replace_node(source_file, Some(target), Some(replacement_subject), None);
        true
    }

    // go: internal/rewriter/piping_flow.go:64 Tracker.replaceFunctionPipePrefix 8d690382ad4adba2
    pub(crate) fn replace_function_pipe_prefix(
        &mut self,
        source_file: P<SourceFile>,
        last_transformation: Option<P<Node>>,
        replacement_subject: P<Node>,
    ) -> bool {
        let (call_node, call, argument_index) = containing_call_argument(last_transformation);
        let (Some(call_node), Some(call)) = (call_node, call) else {
            return false;
        };
        if argument_index < 1 {
            return false;
        }

        let f = &self.tracker.node_factory;
        let remaining_arguments =
            clone_node_list_nodes_from(f, Some(call.arguments()), argument_index + 1);
        let mut replacement = replacement_subject;
        if !remaining_arguments.is_empty() {
            let mut arguments = vec![replacement_subject];
            arguments.extend(remaining_arguments);
            replacement = f.new_call_expression(
                f.deep_clone_node(Some(call.expression())).unwrap(),
                f.deep_clone_node(call.question_dot_token()),
                clone_node_list(f, call.type_arguments()),
                f.new_node_list(arguments),
                call_node.flags(),
            );
        }
        ast::set_parent_in_children(replacement);
        self.replace_node(source_file, Some(call_node), Some(replacement), None);
        true
    }

    // go: internal/rewriter/piping_flow.go:87 Tracker.replaceMethodPipePrefix 1f764a2f2ccb2792
    // Rust: Go's `AsPropertyAccessExpression` panics on another kind, as tsrs's does; the name is never nil.
    pub(crate) fn replace_method_pipe_prefix(
        &mut self,
        source_file: P<SourceFile>,
        last_transformation: Option<P<Node>>,
        replacement_subject: P<Node>,
    ) -> bool {
        let (call_node, call, argument_index) = containing_call_argument(last_transformation);
        let (Some(call_node), Some(call)) = (call_node, call) else {
            return false;
        };
        if argument_index < 0 {
            return false;
        }
        let access = call.expression().as_property_access_expression();

        let f = &self.tracker.node_factory;
        let remaining_arguments =
            clone_node_list_nodes_from(f, Some(call.arguments()), argument_index + 1);
        let mut replacement = replacement_subject;
        if !remaining_arguments.is_empty() {
            let replacement_access = f.new_property_access_expression(
                replacement_subject,
                f.deep_clone_node(access.question_dot_token()),
                f.deep_clone_node(Some(access.name())).unwrap(),
                call.expression().flags(),
            );
            replacement = f.new_call_expression(
                replacement_access,
                f.deep_clone_node(call.question_dot_token()),
                clone_node_list(f, call.type_arguments()),
                f.new_node_list(remaining_arguments),
                call_node.flags(),
            );
        }
        ast::set_parent_in_children(replacement);
        self.replace_node(source_file, Some(call_node), Some(replacement), None);
        true
    }
}

// go: internal/rewriter/piping_flow.go:119 containingCallArgument 9b75000f48a347d8
// Rust: the call is returned both as its node and as its `CallExpression` data (Go's `*ast.Node` and
// `*ast.CallExpression`); tsrs's call arguments are never nil.
pub(crate) fn containing_call_argument(
    node: Option<P<Node>>,
) -> (Option<P<Node>>, Option<&'static CallExpression>, i64) {
    let mut child = node;
    while let Some(c) = child {
        let Some(parent) = c.parent() else {
            break;
        };
        if ast::is_call_expression(parent) {
            let call = parent.as_call_expression();
            for (index, &argument) in call.arguments().nodes().iter().enumerate() {
                if argument == c {
                    return (Some(parent), Some(call), index as i64);
                }
            }
        }
        child = Some(parent);
    }
    (None, None, -1)
}

// go: internal/rewriter/piping_flow.go:138 pipingFlowTransformationNode 6de0fb2082bf06ed
// Rust: tsrs's transformation callee is never nil; calls are returned as their nodes.
pub(crate) fn piping_flow_transformation_node(
    transformation: &PipingFlowTransformation,
) -> Option<P<Node>> {
    let mut call = call_expression_for_callee(Some(transformation.callee()));
    if transformation.kind() == TransformationKind::Call {
        // Curried factories decompose into a factory call and a trailing
        // application; the replaced node is the outermost applied call.
        call = applied_call_expression(call);
    }
    if call.is_some() {
        return call;
    }
    Some(transformation.callee())
}

// go: internal/rewriter/piping_flow.go:154 callExpressionForCallee d2506a6002f6283c
// Rust: returns the call node (Go's `*ast.CallExpression`).
pub(crate) fn call_expression_for_callee(callee: Option<P<Node>>) -> Option<P<Node>> {
    let callee = callee?;
    let parent = callee.parent()?;
    if !ast::is_call_expression(parent) {
        return None;
    }
    if parent.as_call_expression().expression() != callee {
        return None;
    }
    Some(parent)
}

// go: internal/rewriter/piping_flow.go:167 appliedCallExpression 723be897b8175f5d
// appliedCallExpression walks out of callee positions, returning the outermost
// call that applies the given call's result to its subject.
pub(crate) fn applied_call_expression(call: Option<P<Node>>) -> Option<P<Node>> {
    let mut current = call?;
    while let Some(parent) = current.parent() {
        if !ast::is_call_expression(parent) {
            break;
        }
        if parent.as_call_expression().expression() != current {
            break;
        }
        current = parent;
    }
    Some(current)
}

impl Tracker {
    // go: internal/rewriter/piping_flow.go:184 Tracker.RemoveTrailingPipingFlowTransformation 4be38ba26b227447
    // RemoveTrailingPipingFlowTransformation removes the last step while retaining
    // its input and any earlier transformations. Unsupported flows produce no edits.
    pub fn remove_trailing_piping_flow_transformation(
        &mut self,
        sf: Option<P<SourceFile>>,
        flow: Option<&PipingFlow>,
    ) -> bool {
        let (Some(sf), Some(flow)) = (sf, flow) else {
            return false;
        };
        if flow.transformations().len() == 0 {
            return false;
        }
        let last = flow.transformations().len() - 1;
        let terminal = flow.transformations().at(last);
        if terminal.callee().flags().contains(NodeFlags::OptionalChain) {
            return false;
        }
        match terminal.kind() {
            TransformationKind::DataFirst
            | TransformationKind::DataLast
            | TransformationKind::Call => {
                let Some(call) = call_expression_for_callee(Some(terminal.callee())) else {
                    return false;
                };
                if call.flags().contains(NodeFlags::OptionalChain) {
                    return false;
                }
                let mut call_node = call;
                if terminal.kind() == TransformationKind::Call {
                    while call_node != flow.node() {
                        let mut applied = call_node;
                        while let Some(parent) = applied.parent() {
                            if parent.kind() != Kind::ParenthesizedExpression {
                                break;
                            }
                            applied = parent;
                        }
                        let Some(parent) = applied.parent() else {
                            break;
                        };
                        if !ast::is_call_expression(parent)
                            || parent.as_call_expression().expression() != applied
                        {
                            break;
                        }
                        call_node = parent;
                        if call_node.flags().contains(NodeFlags::OptionalChain) {
                            return false;
                        }
                    }
                }
                let mut input = PipingFlow::transformation_input_node(Some(flow), last);
                if terminal.kind() == TransformationKind::Call {
                    let call = call_node.as_call_expression();
                    if call.arguments().nodes().len() != 1 {
                        return false;
                    }
                    input = Some(call.arguments().nodes()[0]);
                }
                let Some(input) = input else {
                    return false;
                };
                self.replace_node(sf, Some(call_node), Some(input), None);
                true
            }

            TransformationKind::Pipe | TransformationKind::Pipeable => {
                let Some(node) = piping_flow_transformation_node(terminal) else {
                    return false;
                };
                if node.flags().contains(NodeFlags::OptionalChain) {
                    return false;
                }
                let (call_node, call, index) = containing_call_argument(Some(node));
                let (Some(call_node), Some(call)) = (call_node, call) else {
                    return false;
                };
                if index < 0 || call_node.flags().contains(NodeFlags::OptionalChain) {
                    return false;
                }
                let index = index as usize;
                let arguments = call.arguments().nodes();
                let mut minimum = 1;
                if terminal.kind() == TransformationKind::Pipe {
                    minimum = 2;
                }
                if arguments.len() == minimum {
                    let mut input = None;
                    if terminal.kind() == TransformationKind::Pipe {
                        input = Some(arguments[0]);
                    } else if ast::is_property_access_expression(call.expression()) {
                        input =
                            Some(call.expression().as_property_access_expression().expression());
                    }
                    let Some(input) = input else {
                        return false;
                    };
                    self.replace_node(sf, Some(call_node), Some(input), None);
                    return true;
                }
                let span;
                if index < arguments.len() - 1 {
                    span = TextRange::new(
                        scanner::get_token_pos_of_node(arguments[index], sf, false),
                        scanner::get_token_pos_of_node(arguments[index + 1], sf, false),
                    );
                } else if index > 0 {
                    let span_pos = arguments[index - 1].end().expect_source(PARSED_NODE);
                    let span_end = arguments[index].end().expect_source(PARSED_NODE);
                    span = TextRange::new(span_pos, span_end);
                    let start = scanner::get_token_pos_of_node(arguments[index], sf, false);
                    let text = sf.text();
                    let trivia = &text[span_pos as usize..start as usize];
                    if trivia.contains("//") || trivia.contains("/*") {
                        let separator = scanner::skip_trivia(text, span_pos);
                        if separator >= start || text.as_bytes()[separator as usize] != b',' {
                            return false;
                        }
                        let bytes = text.as_bytes();
                        let mut trim_start = start;
                        while trim_start > span_pos
                            && (trim_start - 1 == separator
                                || bytes[trim_start as usize - 1] == b' '
                                || bytes[trim_start as usize - 1] == b'\t')
                        {
                            trim_start -= 1;
                        }
                        self.delete_range(sf, TextRange::new(trim_start, span_end));
                        if separator < trim_start {
                            self.delete_range(sf, TextRange::new(separator, separator + 1));
                        }
                        return true;
                    }
                } else {
                    return false;
                }
                self.delete_range(sf, span);
                true
            }
            _ => false,
        }
    }

    // go: internal/rewriter/piping_flow.go:293 Tracker.ReplacePipingFlowTransformation 5e90f24f36b631e5
    // ReplacePipingFlowTransformation replaces a transformation while preserving
    // how its input is applied in source: as a pipe argument, a data-first or
    // data-last argument, or a unary/curried application. A call with optional
    // arguments keeps them after the input.
    //
    // Rust: Go's `AsCallExpression` panics on another kind, as tsrs's does; tsrs's call arguments are never nil.
    pub fn replace_piping_flow_transformation(
        &mut self,
        source_file: Option<P<SourceFile>>,
        transformation: Option<&PipingFlowTransformation>,
        replacement: PipingFlowTransformationReplacement,
    ) -> bool {
        let (Some(source_file), Some(transformation), Some(callee)) =
            (source_file, transformation, replacement.callee)
        else {
            return false;
        };
        let Some(transformation_node) = piping_flow_transformation_node(transformation) else {
            return false;
        };

        match transformation.kind() {
            TransformationKind::Pipe
            | TransformationKind::Pipeable
            | TransformationKind::EffectFn
            | TransformationKind::EffectFnUntraced => {
                let replacement_node = self.piping_flow_operator(replacement);
                ast::set_parent_in_children(replacement_node);
                self.replace_node(
                    source_file,
                    Some(transformation_node),
                    Some(replacement_node),
                    None,
                );
                true
            }

            TransformationKind::DataFirst | TransformationKind::DataLast => {
                let call = transformation_node.as_call_expression();
                if call.arguments().nodes().is_empty() {
                    return false;
                }
                let f = &self.tracker.node_factory;
                let mut arguments = clone_node_list_nodes(f, replacement.arguments);
                let mut subject_index = 0;
                if transformation.kind() == TransformationKind::DataLast {
                    subject_index = call.arguments().nodes().len() - 1;
                }
                let subject =
                    f.deep_clone_node(Some(call.arguments().nodes()[subject_index])).unwrap();
                if transformation.kind() == TransformationKind::DataFirst {
                    arguments.insert(0, subject);
                } else {
                    arguments.push(subject);
                }
                let replacement_node = f.new_call_expression(
                    callee,
                    None,
                    replacement.type_arguments,
                    f.new_node_list(arguments),
                    NodeFlags::None,
                );
                ast::set_parent_in_children(replacement_node);
                self.replace_node(
                    source_file,
                    Some(transformation_node),
                    Some(replacement_node),
                    None,
                );
                true
            }

            TransformationKind::Call => {
                let call = transformation_node.as_call_expression();
                if call.arguments().nodes().len() != 1 {
                    return false;
                }
                let operator = self.piping_flow_operator(replacement);
                let f = &self.tracker.node_factory;
                let replacement_node = f.new_call_expression(
                    operator,
                    None,
                    None,
                    f.new_node_list(vec![
                        f.deep_clone_node(Some(call.arguments().nodes()[0])).unwrap(),
                    ]),
                    NodeFlags::None,
                );
                ast::set_parent_in_children(replacement_node);
                self.replace_node(
                    source_file,
                    Some(transformation_node),
                    Some(replacement_node),
                    None,
                );
                true
            }

            TransformationKind::CallWithOptionals => {
                let call = transformation_node.as_call_expression();
                if call.arguments().nodes().len() < 2 {
                    return false;
                }
                let operator = self.piping_flow_operator(replacement);
                let f = &self.tracker.node_factory;
                let replacement_node = f.new_call_expression(
                    operator,
                    None,
                    None,
                    clone_node_list(f, Some(call.arguments())).unwrap(),
                    NodeFlags::None,
                );
                ast::set_parent_in_children(replacement_node);
                self.replace_node(
                    source_file,
                    Some(transformation_node),
                    Some(replacement_node),
                    None,
                );
                true
            }
        }
    }

    // go: internal/rewriter/piping_flow.go:380 Tracker.pipingFlowOperator 0acac1ed78187157
    pub(crate) fn piping_flow_operator(
        &self,
        replacement: PipingFlowTransformationReplacement,
    ) -> P<Node> {
        piping_flow_operator(&self.tracker.node_factory, replacement)
    }
}

// go: internal/rewriter/piping_flow.go:380 Tracker.pipingFlowOperator 0acac1ed78187157
// Rust: the body over the tracker's factory, so that the visitor of `collapse_piping_flow_transformations` (a
// `'static` closure, which cannot borrow the tracker) can call it. Callers check that the callee is set, as Go's
// do. tsrs's call expressions always have an argument list: Go's nil `Arguments` next to type arguments is an
// empty list (both print `()`).
pub(crate) fn piping_flow_operator(
    f: &NodeFactory,
    replacement: PipingFlowTransformationReplacement,
) -> P<Node> {
    let callee = replacement.callee.expect("the replacement callee is checked by the caller");
    if replacement.arguments.is_none() && replacement.type_arguments.is_none() {
        return callee;
    }
    f.new_call_expression(
        callee,
        None,
        replacement.type_arguments,
        replacement.arguments.unwrap_or_else(|| f.new_node_list(Vec::new())),
        NodeFlags::None,
    )
}

// go: internal/rewriter/piping_flow.go:393 cloneNodeListNodes 795bf81e125deeb7
// Rust: takes the tracker's factory (Go: the tracker, for its embedded factory); nil and empty are one `Vec`.
pub(crate) fn clone_node_list_nodes(f: &NodeFactory, list: Option<P<NodeList>>) -> Vec<P<Node>> {
    let Some(list) = list else {
        return Vec::new();
    };
    list.nodes().iter().map(|&node| f.deep_clone_node(Some(node)).unwrap()).collect()
}

// go: internal/rewriter/piping_flow.go:404 cloneNodeListNodesFrom 5397691c927fff14
pub(crate) fn clone_node_list_nodes_from(
    f: &NodeFactory,
    list: Option<P<NodeList>>,
    start: i64,
) -> Vec<P<Node>> {
    let Some(list) = list else {
        return Vec::new();
    };
    if start >= list.nodes().len() as i64 {
        return Vec::new();
    }
    list.nodes()[start as usize..]
        .iter()
        .map(|&node| f.deep_clone_node(Some(node)).unwrap())
        .collect()
}

// go: internal/rewriter/piping_flow.go:415 cloneNodeList a65c5a709c756d5b
pub(crate) fn clone_node_list(f: &NodeFactory, list: Option<P<NodeList>>) -> Option<P<NodeList>> {
    let list = list?;
    Some(f.new_node_list(clone_node_list_nodes(f, Some(list))))
}

impl Tracker {
    // go: internal/rewriter/piping_flow.go:425 Tracker.CollapsePipingFlowTransformations b224986204c116d3
    // CollapsePipingFlowTransformations replaces an inclusive range of adjacent
    // steps with one operator at the last step. Nested applications are rewritten
    // together so removing an input step cannot overlap the replacement edit.
    //
    // Rust: tsrs's visitor callback is a `'static` closure, so it captures what Go's closure reads from the flow
    // (each step's kind and input node) instead of the flow, and the factory instead of the tracker.
    pub fn collapse_piping_flow_transformations(
        &mut self,
        source_file: Option<P<SourceFile>>,
        flow: Option<&PipingFlow>,
        start: i64,
        end: i64,
        replacement: PipingFlowTransformationReplacement,
    ) -> bool {
        let (Some(source_file), Some(flow)) = (source_file, flow) else {
            return false;
        };
        if start < 0
            || end < start
            || end >= flow.transformations().len()
            || replacement.callee.is_none()
        {
            return false;
        }
        let mut steps: FxHashMap<P<Node>, i64> = FxHashMap::default();
        let mut emptied_pipes: FxHashMap<P<Node>, TransformationKind> = FxHashMap::default();
        for i in start..=end {
            let Some(node) = piping_flow_transformation_node(flow.transformations().at(i)) else {
                return false;
            };
            steps.insert(node, i);
            let kind = flow.transformations().at(i).kind();
            if i < end && (kind == TransformationKind::Pipe || kind == TransformationKind::Pipeable)
            {
                let (call, _, _) = containing_call_argument(Some(node));
                // Rust: Go stores a nil key too, which no visited node matches.
                if let Some(call) = call {
                    emptied_pipes.insert(call, kind);
                }
            }
        }
        // Find the smallest source subtree containing all edited steps. Pipe
        // arguments require their containing call so they can be removed as a list.
        let mut target = piping_flow_transformation_node(flow.transformations().at(end));
        for i in start..=end {
            let mut node = piping_flow_transformation_node(flow.transformations().at(i));
            if is_piping_flow_argument(flow.transformations().at(i).kind()) {
                let (call, _, _) = containing_call_argument(node);
                node = call;
            }
            let Some(node) = node else {
                return false;
            };
            while let Some(t) = target {
                if node_within(Some(node), t) {
                    break;
                }
                target = t.parent();
            }
        }
        let Some(target) = target else {
            return false;
        };
        let kinds: Vec<TransformationKind> =
            flow.transformations().all().map(|(_, step)| step.kind()).collect();
        // Rust: Go calls `flow.TransformationInputNode(index)` in the visitor; it only reads the flow, so each
        // step's input is read up front.
        let inputs: Vec<Option<P<Node>>> = (0..kinds.len() as i64)
            .map(|index| PipingFlow::transformation_input_node(Some(flow), index))
            .collect();
        // Rust: Go's closure assigns `replaced`; the `'static` visitor shares it through a cell.
        let replaced: Rc<Cell<Option<P<Node>>>> = Rc::new(Cell::new(None));
        let visit_replaced = Rc::clone(&replaced);
        let factory = self.tracker.node_factory.clone();
        let visit_factory = factory.clone();
        let visit = move |visitor: &mut NodeVisitor, node: P<Node>| -> Option<P<Node>> {
            let replaced = &visit_replaced;
            let f = &visit_factory;
            let Some(&index) = steps.get(&node) else {
                let updated = visitor.visit_each_child(Some(node)).unwrap();
                if let Some(&kind) = emptied_pipes.get(&node) {
                    let call = updated.as_call_expression();
                    if kind == TransformationKind::Pipeable && call.arguments().nodes().is_empty() {
                        return Some(
                            call.expression().as_property_access_expression().expression(),
                        );
                    }
                    if kind == TransformationKind::Pipe && call.arguments().nodes().len() == 1 {
                        return Some(call.arguments().nodes()[0]);
                    }
                }
                return Some(updated);
            };
            let step_kind = kinds[index as usize];
            if is_piping_flow_argument(step_kind) {
                if index < end {
                    return None;
                }
                replaced.set(Some(piping_flow_operator(f, replacement)));
                return replaced.get();
            }
            let Some(input) = inputs[index as usize] else {
                return Some(node);
            };
            let subject = visitor.visit_node(Some(input));
            if index < end {
                return subject;
            }
            // Rust: Go would build a list holding nil; the visited input of a kept step is never removed.
            let subject = subject.expect("the input of the collapsed step is kept");
            let mut arguments = clone_node_list_nodes(f, replacement.arguments);
            match step_kind {
                TransformationKind::DataFirst => arguments.insert(0, subject),
                TransformationKind::DataLast => arguments.push(subject),
                _ => {
                    replaced.set(Some(f.new_call_expression(
                        piping_flow_operator(f, replacement),
                        None,
                        None,
                        f.new_node_list(vec![subject]),
                        NodeFlags::None,
                    )));
                    return replaced.get();
                }
            }
            replaced.set(Some(f.new_call_expression(
                replacement.callee.unwrap(),
                None,
                replacement.type_arguments,
                f.new_node_list(arguments),
                NodeFlags::None,
            )));
            replaced.get()
        };
        let mut visitor =
            ast::new_node_visitor(Some(Rc::new(visit)), Some(factory), NodeVisitorHooks::default());
        let visited =
            visitor.visit_node(Some(target)).expect("the collapse target contains a kept step");
        let result = self.tracker.node_factory.deep_clone_node(Some(visited)).unwrap();
        let mut removed = Vec::with_capacity((end - start + 1) as usize);
        for i in start..=end {
            removed.push(piping_flow_transformation_node(flow.transformations().at(i)));
        }
        let result = self.retain_piping_flow_comments(
            source_file,
            target,
            visited,
            result,
            &removed,
            PipingFlow::transformation_input_node(Some(flow), start),
            replaced.get(),
        );
        ast::set_parent_in_children(result);
        self.replace_node(source_file, Some(target), Some(result), None);
        true
    }

    // go: internal/rewriter/piping_flow.go:525 Tracker.retainPipingFlowComments ca8d43868655dbeb
    // retainPipingFlowComments re-attaches source comments to the synthesized
    // clone of a collapsed flow. Comments inside the removed steps move to the
    // replacement; every other comment stays with the innermost kept node that
    // contains it. Comments outside target stay in the source text, which the
    // replacement leaves untouched.
    //
    // Rust: tsrs's `Block` statements cannot be reassigned, so the placeholders Go appends to a block clone are
    // collected and the clone is rebuilt with them after every comment is attached (`update_block` copies the
    // clone's emit node, and so its comments, to the rebuilt node); the function returns the possibly rebuilt
    // `result`. Go picks the owner by ranging over a map, so between kept nodes of the same extent (a statement
    // and its expression) the choice follows the map order; the Rust takes the innermost one.
    #[expect(clippy::too_many_arguments, reason = "mirrors Go's signature")]
    pub(crate) fn retain_piping_flow_comments(
        &self,
        source_file: P<SourceFile>,
        target: P<Node>,
        visited: P<Node>,
        result: P<Node>,
        removed: &[Option<P<Node>>],
        kept: Option<P<Node>>,
        replaced: Option<P<Node>>,
    ) -> P<Node> {
        let comments = source_comments(
            source_file,
            scanner::get_token_pos_of_node(target, source_file, false),
            target.end().expect_source(PARSED_NODE),
        );
        if comments.is_empty() {
            return result;
        }
        fn children(node: P<Node>) -> Vec<P<Node>> {
            let mut nodes = Vec::new();
            node.for_each_child(&mut |child| {
                nodes.push(child);
                false
            });
            nodes
        }
        // Nodes built for the replacement have no source position; updated and
        // untouched nodes keep theirs, so they own the comments within their range.
        //
        // Rust: `order` keeps the keys of `clones` in pairing (pre-order) order for the owner search.
        let mut clones: FxHashMap<P<Node>, P<Node>> = FxHashMap::default();
        let mut order: Vec<P<Node>> = Vec::new();
        let mut replaced_clone = result;
        fn pair(
            original: P<Node>,
            clone: P<Node>,
            replaced: Option<P<Node>>,
            replaced_clone: &mut P<Node>,
            clones: &mut FxHashMap<P<Node>, P<Node>>,
            order: &mut Vec<P<Node>>,
        ) {
            if Some(original) == replaced {
                *replaced_clone = clone;
            }
            if original.pos().source_offset().is_some() {
                clones.insert(original, clone);
                order.push(original);
            }
            let (originals, cloned) = (children(original), children(clone));
            if originals.len() == cloned.len() {
                for (&original, &clone) in originals.iter().zip(&cloned) {
                    pair(original, clone, replaced, replaced_clone, clones, order);
                }
            }
        }
        pair(visited, result, replaced, &mut replaced_clone, &mut clones, &mut order);
        let text = source_file.text();
        let pos = |node: P<Node>| node.pos().source_offset();
        let end = |node: P<Node>| node.end().expect_source(PARSED_NODE);
        // The kept subject keeps its leading trivia, and a removed step takes its
        // text plus the trivia up to the next token.
        let in_kept = |comment: &CommentRange| {
            kept.is_some_and(|kept| {
                pos(kept).is_some_and(|p| comment.pos() >= p && comment.end() <= end(kept))
            })
        };
        let in_removed = |comment: &CommentRange| {
            removed.iter().any(|node| {
                node.is_some_and(|node| {
                    pos(node).is_some_and(|p| {
                        comment.pos() >= p && comment.end() <= scanner::skip_trivia(text, end(node))
                    })
                })
            })
        };
        // A kept node owns the text from its first token on. Its leading trivia
        // belongs before it in its parent: a comment there must not end up between
        // the node's first token and the rest, as in "return // comment\n value".
        let inside = |comment: &CommentRange, node: P<Node>| {
            pos(node).is_some()
                && comment.pos() >= scanner::get_token_pos_of_node(node, source_file, false)
                && comment.end() <= end(node)
        };
        let size = |node: P<Node>| end(node) - pos(node).unwrap();
        // Rust: the placeholders appended to each block clone, applied after the loop (see above).
        let mut blocks: FxHashMap<P<Node>, Vec<P<Node>>> = FxHashMap::default();
        for comment in &comments {
            if !in_kept(comment) && in_removed(comment) {
                self.add_piping_flow_comment(text, replaced_clone, comment, true);
                continue;
            }
            let mut owner: Option<P<Node>> = None;
            for &original in &order {
                if inside(comment, original)
                    && owner.is_none_or(|owner| size(original) <= size(owner))
                {
                    owner = Some(original);
                }
            }
            let owner = owner.unwrap_or(visited);
            // Attach to the nearest kept child so the comment keeps its place; an
            // owner without kept children around the comment takes it itself.
            let mut before: Option<P<Node>> = None;
            let mut after: Option<P<Node>> = None;
            for child in children(owner) {
                if !clones.contains_key(&child) {
                    continue;
                }
                if end(child) <= comment.pos() {
                    before = Some(child);
                } else if after.is_none()
                    && scanner::get_token_pos_of_node(child, source_file, false) >= comment.end()
                {
                    after = Some(child);
                }
            }
            if let Some(after) = after {
                self.add_piping_flow_comment(text, clones[&after], comment, true);
            } else if let Some(before) = before {
                self.add_piping_flow_comment(text, clones[&before], comment, false);
            } else if let Some(&owner_clone) = clones.get(&owner) {
                if owner_clone.kind() == Kind::Block {
                    // Function bodies skip the comments of their block node, so keep
                    // the comment inside the braces on a placeholder statement.
                    let placeholder = self.tracker.node_factory.new_not_emitted_statement();
                    self.add_piping_flow_comment(text, placeholder, comment, true);
                    blocks.entry(owner_clone).or_default().push(placeholder);
                } else {
                    self.add_piping_flow_comment(text, owner_clone, comment, true);
                }
            } else {
                self.add_piping_flow_comment(text, result, comment, true);
            }
        }
        if blocks.is_empty() {
            return result;
        }
        let factory = self.tracker.node_factory.clone();
        let visit_factory = factory.clone();
        let visit = move |visitor: &mut NodeVisitor, node: P<Node>| -> Option<P<Node>> {
            let updated = visitor.visit_each_child(Some(node)).unwrap();
            let Some(placeholders) = blocks.get(&node) else {
                return Some(updated);
            };
            let block = updated.as_block();
            let mut statements = block.statements().nodes().to_vec();
            statements.extend_from_slice(placeholders);
            Some(visit_factory.update_block(
                updated,
                visit_factory.new_node_list(statements),
                block.multi_line(),
            ))
        };
        let mut visitor =
            ast::new_node_visitor(Some(Rc::new(visit)), Some(factory), NodeVisitorHooks::default());
        visitor.visit_node(Some(result)).unwrap()
    }

    // go: internal/rewriter/piping_flow.go:648 Tracker.addPipingFlowComment ae97aebb26b284aa
    pub(crate) fn add_piping_flow_comment(
        &self,
        text: &str,
        node: P<Node>,
        comment: &CommentRange,
        leading: bool,
    ) {
        let mut body = &text[comment.pos() as usize + 2..comment.end() as usize];
        // A line comment always needs its line break, or it would swallow the code after it.
        let mut new_line = true;
        if comment.kind == Kind::MultiLineCommentTrivia {
            body = body.strip_suffix("*/").unwrap_or(body);
            new_line = comment.has_trailing_new_line;
        }
        if leading {
            self.tracker.emit_context.add_synthetic_leading_comment(
                node,
                comment.kind,
                body,
                new_line,
            );
        } else {
            self.tracker.emit_context.add_synthetic_trailing_comment(
                node,
                comment.kind,
                body,
                new_line,
            );
        }
    }
}

// go: internal/rewriter/piping_flow.go:625 sourceComments 0d63739be11bd7e6
// sourceComments lists the comments between start and end in source order.
// Tokens are skipped by their parsed extent, so comment-like text inside
// literals, templates and regular expressions is not reported.
pub(crate) fn source_comments(
    source_file: P<SourceFile>,
    start: u32,
    end: u32,
) -> Vec<CommentRange> {
    let text = source_file.text();
    let mut s = scanner::Scanner::new();
    s.set_skip_trivia(false);
    s.set_text(&text[..end as usize]);
    s.reset_pos(start);
    let mut comments = Vec::new();
    let mut token = s.scan();
    while token != Kind::EndOfFile {
        match token {
            Kind::SingleLineCommentTrivia | Kind::MultiLineCommentTrivia => {
                let rest = text[s.token_end() as usize..].trim_start_matches([' ', '\t']);
                let new_line = rest.starts_with('\n') || rest.starts_with('\r');
                comments.push(CommentRange {
                    text_range: TextRange::new(s.token_start(), s.token_end()),
                    kind: token,
                    has_trailing_new_line: new_line,
                });
            }
            Kind::WhitespaceTrivia | Kind::NewLineTrivia | Kind::ConflictMarkerTrivia => {}
            _ => {
                let original = astnav::get_token_at_position(source_file, s.token_start());
                if let Some(original_end) = original.end().source_offset()
                    && original_end > s.token_end()
                {
                    s.reset_pos(original_end.min(end));
                }
            }
        }
        token = s.scan();
    }
    comments
}

// go: internal/rewriter/piping_flow.go:663 isPipingFlowArgument e95d8061c1653ef2
pub(crate) fn is_piping_flow_argument(kind: TransformationKind) -> bool {
    matches!(
        kind,
        TransformationKind::Pipe
            | TransformationKind::Pipeable
            | TransformationKind::EffectFn
            | TransformationKind::EffectFnUntraced
    )
}

// go: internal/rewriter/piping_flow.go:671 nodeWithin baedec3b3d887301
pub(crate) fn node_within(node: Option<P<Node>>, ancestor: P<Node>) -> bool {
    let mut node = node;
    while let Some(n) = node {
        if n == ancestor {
            return true;
        }
        node = n.parent();
    }
    false
}
