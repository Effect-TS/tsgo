// Package typeparser provides Effect type detection and parsing utilities.

use std::sync::Arc;

use ets_readonly::Slice;
use ets_shim::ast::{Kind, Node, NodeList, SourceFile};
use ets_shim::checker::{SignatureKind, Type};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/piping_flow.go:14 TransformationKind 86f1e2de9ad6224c
// go: internal/typeparser/piping_flow.go:16 TransformationKindPipe cd853bdc5982d1de
// TransformationKind represents how a transformation was expressed in source code.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TransformationKind {
    Pipe,
    Pipeable,
    DataFirst,
    DataLast,
    Call,
    EffectFn,
    EffectFnUntraced,
}

// go: internal/typeparser/piping_flow.go:14 TransformationKind 86f1e2de9ad6224c
impl TransformationKind {
    /// Rust-only: the Go string value (`string(kind)`).
    pub fn as_str(self) -> &'static str {
        match self {
            TransformationKind::Pipe => "pipe",
            TransformationKind::Pipeable => "pipeable",
            TransformationKind::DataFirst => "dataFirst",
            TransformationKind::DataLast => "dataLast",
            TransformationKind::Call => "call",
            TransformationKind::EffectFn => "effectFn",
            TransformationKind::EffectFnUntraced => "effectFnUntraced",
        }
    }
}

// go: internal/typeparser/piping_flow.go:27 PipingFlowTransformation 2f9c76c4d0d83efc
// PipingFlowTransformation represents a single transformation step in a piping flow.
#[derive(Clone)]
pub struct PipingFlowTransformation {
    pub(crate) kind: TransformationKind, // How the transformation was expressed
    pub(crate) callee: P<Node>,          // The function being applied (e.g., Effect.map)
    pub(crate) type_arguments: Option<P<NodeList>>, // Explicit type arguments to the transformation call, if any
    pub(crate) args: Option<Vec<P<Node>>>, // Arguments to the transformation, or nil for constants/single-arg calls
    pub(crate) out_type: Option<Lazy<Option<P<Type>>>>, // The resulting type after this transformation (may be nil)
}

impl PipingFlowTransformation {
    // go: internal/typeparser/piping_flow.go:35 PipingFlowTransformation.Kind 654f103dfd4c5533
    pub fn kind(&self) -> TransformationKind {
        self.kind
    }

    // go: internal/typeparser/piping_flow.go:37 PipingFlowTransformation.Callee aa04011c87df5430
    pub fn callee(&self) -> P<Node> {
        self.callee
    }

    // go: internal/typeparser/piping_flow.go:39 PipingFlowTransformation.TypeArguments b6c98c8896fa8a73
    pub fn type_arguments(&self) -> Option<P<NodeList>> {
        self.type_arguments
    }

    // go: internal/typeparser/piping_flow.go:41 PipingFlowTransformation.Args 82ed56773418ef8a
    pub fn args(&self) -> Slice<'_, P<Node>> {
        ets_readonly::new(self.args.as_deref())
    }

    // go: internal/typeparser/piping_flow.go:43 PipingFlowTransformation.OutType 5d629b8e81f85d58
    // Rust: the lazy resolves through `tp` (see `Lazy`).
    pub fn out_type(&self, tp: &mut TypeParser<'_>) -> Option<P<Type>> {
        Lazy::get(self.out_type.as_ref(), tp)
    }
}

// go: internal/typeparser/piping_flow.go:46 PipingFlowSubject 1b53bd6aee29e242
// PipingFlowSubject is the starting expression of a piping flow.
#[derive(Clone, Default)]
pub struct PipingFlowSubject {
    pub(crate) node: Option<P<Node>>, // The expression node
    pub(crate) out_type: Option<Lazy<Option<P<Type>>>>, // The type of the subject expression (may be nil)
}

impl PipingFlowSubject {
    // go: internal/typeparser/piping_flow.go:51 PipingFlowSubject.Node 81554329dcf48028
    pub fn node(&self) -> Option<P<Node>> {
        self.node
    }

    // go: internal/typeparser/piping_flow.go:53 PipingFlowSubject.OutType 979a0d6fe8942f74
    // Rust: the lazy resolves through `tp` (see `Lazy`).
    pub fn out_type(&self, tp: &mut TypeParser<'_>) -> Option<P<Type>> {
        Lazy::get(self.out_type.as_ref(), tp)
    }
}

// go: internal/typeparser/piping_flow.go:57 PartialPipingFlow 651d29dee2527fa9
// PartialPipingFlow represents a logical piping flow that may not correspond to
// a complete source expression, such as a prefix of a pipe call.
#[derive(Clone, Default)]
pub struct PartialPipingFlow {
    pub(crate) subject: PipingFlowSubject, // The starting expression and its type
    pub(crate) transformations: Vec<PipingFlowTransformation>, // Ordered list of transformations
}

impl PartialPipingFlow {
    // go: internal/typeparser/piping_flow.go:62 PartialPipingFlow.Subject 516d995853ff36ae
    pub fn subject(&self) -> PipingFlowSubject {
        self.subject.clone()
    }

    // go: internal/typeparser/piping_flow.go:64 PartialPipingFlow.Transformations bf1608fc66833601
    pub fn transformations(&self) -> Slice<'_, PipingFlowTransformation> {
        ets_readonly::new(&self.transformations[..])
    }

    // go: internal/typeparser/piping_flow.go:71 PartialPipingFlow.MatchesPrefix 8d1c4edcd83ec52b
    // MatchesPrefix checks the subject and leading transformations, allowing extra
    // steps afterward. With no transformation predicates it checks only the subject.
    // A nil flow returns false. Predicates receive local copies.
    //
    // Rust: `this` is Go's receiver, which may be nil. Predicates receive the parser (Go's closures capture it)
    // and a shared reference to the step (Go's local copy).
    pub fn matches_prefix(
        this: Option<&Self>,
        tp: &mut TypeParser<'_>,
        subject: SubjectPredicate<'_>,
        transformations: &[TransformationPredicate<'_>],
    ) -> bool {
        Self::matches_shape(this, tp, false, subject, transformations)
    }

    // go: internal/typeparser/piping_flow.go:78 PartialPipingFlow.MatchesExactly c64e09ecddf0070b
    // MatchesExactly checks the subject and all transformations. The number of
    // transformations must equal the number of transformation predicates.
    // A nil flow returns false. Predicates receive local copies.
    pub fn matches_exactly(
        this: Option<&Self>,
        tp: &mut TypeParser<'_>,
        subject: SubjectPredicate<'_>,
        transformations: &[TransformationPredicate<'_>],
    ) -> bool {
        Self::matches_shape(this, tp, true, subject, transformations)
    }

    // go: internal/typeparser/piping_flow.go:82 PartialPipingFlow.matchesShape 459aef855368b632
    pub(crate) fn matches_shape(
        this: Option<&Self>,
        tp: &mut TypeParser<'_>,
        exact: bool,
        subject: SubjectPredicate<'_>,
        transformations: &[TransformationPredicate<'_>],
    ) -> bool {
        let Some(flow) = this else {
            return false;
        };
        if flow.transformations.len() < transformations.len()
            || exact && flow.transformations.len() != transformations.len()
        {
            return false;
        }
        if !subject(tp, &flow.subject) {
            return false;
        }
        for (i, predicate) in transformations.iter().enumerate() {
            if !predicate(tp, &flow.transformations[i]) {
                return false;
            }
        }
        true
    }
}

/// Rust-only: a subject predicate of the flow matchers (Go's `func(PipingFlowSubject) bool`, whose closures
/// capture the parser).
pub type SubjectPredicate<'p> = &'p dyn Fn(&mut TypeParser<'_>, &PipingFlowSubject) -> bool;

/// Rust-only: a transformation predicate of the flow matchers (Go's `func(PipingFlowTransformation) bool`).
pub type TransformationPredicate<'p> =
    &'p dyn Fn(&mut TypeParser<'_>, &PipingFlowTransformation) -> bool;

// go: internal/typeparser/piping_flow.go:100 TransformationSequenceMatch 42e59ed5a92a10d8
// TransformationSequenceMatch identifies consecutive steps in a piping flow.
// Transformations is a read-only view into the original flow's slice.
#[derive(Clone, Copy)]
pub struct TransformationSequenceMatch<'a> {
    pub(crate) start: i64,
    pub(crate) transformations: &'a [PipingFlowTransformation],
}

impl<'a> TransformationSequenceMatch<'a> {
    // go: internal/typeparser/piping_flow.go:105 TransformationSequenceMatch.Start 6c360b527bb3b1f9
    pub fn start(&self) -> i64 {
        self.start
    }

    // go: internal/typeparser/piping_flow.go:107 TransformationSequenceMatch.Transformations 5a255c6982e3d48a
    pub fn transformations(&self) -> Slice<'a, PipingFlowTransformation> {
        ets_readonly::new(self.transformations)
    }
}

impl PartialPipingFlow {
    // go: internal/typeparser/piping_flow.go:114 PartialPipingFlow.FindTransformationSequences ef09008b07d37d6d
    // FindTransformationSequences returns all consecutive matches in source order,
    // including overlaps. The view is nil for a nil flow, no predicates, or no match.
    // Predicates receive local copies and must not depend on earlier calls.
    //
    // Rust: Go returns a read-only view of a fresh slice; it is returned owned (`None` is the nil view).
    pub fn find_transformation_sequences<'a>(
        this: Option<&'a Self>,
        tp: &mut TypeParser<'_>,
        predicates: &[TransformationPredicate<'_>],
    ) -> Option<Vec<TransformationSequenceMatch<'a>>> {
        let flow = this?;
        if predicates.is_empty() {
            return None;
        }
        let mut matches: Option<Vec<TransformationSequenceMatch<'a>>> = None;
        let mut start = 0;
        while start + predicates.len() <= flow.transformations.len() {
            let mut matched = true;
            for (offset, predicate) in predicates.iter().enumerate() {
                if !predicate(tp, &flow.transformations[start + offset]) {
                    matched = false;
                    break;
                }
            }
            if matched {
                matches.get_or_insert_with(Vec::new).push(TransformationSequenceMatch {
                    start: start as i64,
                    transformations: &flow.transformations[start..start + predicates.len()],
                });
            }
            start += 1;
        }
        matches
    }

    // go: internal/typeparser/piping_flow.go:139 PartialPipingFlow.CopyPrefix 09d55fa1ba62eea3
    // CopyPrefix returns a copy containing the first transformationCount
    // transformations. It returns nil when transformationCount is out of bounds.
    pub fn copy_prefix(
        this: Option<&Self>,
        transformation_count: i64,
    ) -> Option<PartialPipingFlow> {
        let flow = this?;
        if transformation_count < 0 || transformation_count > flow.transformations.len() as i64 {
            return None;
        }
        let transformations = flow.transformations[..transformation_count as usize].to_vec();
        Some(PartialPipingFlow { subject: flow.subject.clone(), transformations })
    }

    // go: internal/typeparser/piping_flow.go:153 PartialPipingFlow.TransformationInputType 9c1cab5e403fae6d
    // TransformationInputType returns the type immediately before the indexed step.
    // Missing types and indices outside the flow return nil.
    //
    // Rust: the lazies resolve through `tp` (see `Lazy`).
    pub fn transformation_input_type(
        this: Option<&Self>,
        tp: &mut TypeParser<'_>,
        index: i64,
    ) -> Option<P<Type>> {
        let flow = this?;
        if index < 0 || index >= flow.transformations.len() as i64 {
            return None;
        }
        if index == 0 {
            return flow.subject.out_type(tp);
        }
        flow.transformations[index as usize - 1].out_type(tp)
    }

    // go: internal/typeparser/piping_flow.go:166 PartialPipingFlow.TransformationInputNode 7e5a5068ada204ca
    // TransformationInputNode returns the standalone source expression that feeds
    // the transformation at index, when that expression is represented directly
    // in the syntax tree.
    pub fn transformation_input_node(this: Option<&Self>, index: i64) -> Option<P<Node>> {
        let flow = this?;
        if index < 0 || index >= flow.transformations.len() as i64 {
            return None;
        }
        let transformation = &flow.transformations[index as usize];
        let mut call = piping_flow_transformation_call(Some(transformation));
        if transformation.kind == TransformationKind::Call {
            // Curried factories decompose into a factory call and a trailing
            // application; the input feeds the outermost applied call.
            call = applied_call_of(call);
        }
        if let Some(call) = call {
            let arguments = call.as_call_expression().arguments().nodes();
            match transformation.kind {
                TransformationKind::Call => {
                    if arguments.len() == 1 {
                        return Some(arguments[0]);
                    }
                }
                TransformationKind::DataFirst => {
                    if let Some(&first) = arguments.first() {
                        return Some(first);
                    }
                }
                TransformationKind::DataLast => {
                    if let Some(&last) = arguments.last() {
                        return Some(last);
                    }
                }
                _ => {}
            }
        }
        if index == 0 {
            return flow.subject.node;
        }
        None
    }
}

// go: internal/typeparser/piping_flow.go:201 appliedCallOf e34de1eb07fa1079
// appliedCallOf walks out of callee positions, returning the outermost call
// that applies the given call's result to its subject.
pub(crate) fn applied_call_of(call: Option<P<Node>>) -> Option<P<Node>> {
    let mut current = call?;
    while let Some(parent) = current.parent()
        && ets_shim::ast::is_call_expression(parent)
    {
        if parent.as_call_expression().expression() != current {
            break;
        }
        current = parent;
    }
    Some(current)
}

// go: internal/typeparser/piping_flow.go:219 pipingFlowTransformationCall 2d2a7af555bfbae2
pub(crate) fn piping_flow_transformation_call(
    transformation: Option<&PipingFlowTransformation>,
) -> Option<P<Node>> {
    let transformation = transformation?;
    let parent = transformation.callee.parent()?;
    if !ets_shim::ast::is_call_expression(parent) {
        return None;
    }
    if parent.as_call_expression().expression() != transformation.callee {
        return None;
    }
    Some(parent)
}

// go: internal/typeparser/piping_flow.go:232 PipingFlow 0aad216fa8e62a33
// PipingFlow represents a complete piping flow rooted at a source expression.
#[derive(Clone)]
pub struct PipingFlow {
    pub(crate) node: P<Node>, // The outermost expression encompassing the entire flow
    pub(crate) partial: PartialPipingFlow,
}

impl PipingFlow {
    // go: internal/typeparser/piping_flow.go:237 PipingFlow.Node defc885b023f0ac0
    pub fn node(&self) -> P<Node> {
        self.node
    }

    // go: internal/typeparser/piping_flow.go:239 PipingFlow.Partial da0a5c36cba50e53
    pub fn partial(&self) -> PartialPipingFlow {
        self.partial.clone()
    }

    // go: internal/typeparser/piping_flow.go:241 PipingFlow.Subject 225a465cefaa1235
    pub fn subject(&self) -> PipingFlowSubject {
        self.partial.subject.clone()
    }

    // go: internal/typeparser/piping_flow.go:243 PipingFlow.Transformations 5ccef62cc9eb92e5
    pub fn transformations(&self) -> Slice<'_, PipingFlowTransformation> {
        ets_readonly::new(&self.partial.transformations[..])
    }

    // go: internal/typeparser/piping_flow.go:249 PipingFlow.MatchesPrefix 4f69b1408bebe5fe
    // MatchesPrefix checks the subject and leading transformations, allowing extra steps.
    // It is defined explicitly to support a nil *PipingFlow.
    pub fn matches_prefix(
        this: Option<&Self>,
        tp: &mut TypeParser<'_>,
        subject: SubjectPredicate<'_>,
        transformations: &[TransformationPredicate<'_>],
    ) -> bool {
        let Some(flow) = this else {
            return false;
        };
        PartialPipingFlow::matches_prefix(Some(&flow.partial), tp, subject, transformations)
    }

    // go: internal/typeparser/piping_flow.go:258 PipingFlow.MatchesExactly 2580e5be554489e4
    // MatchesExactly checks the subject and all transformations, requiring an exact length.
    // It is defined explicitly to support a nil *PipingFlow.
    pub fn matches_exactly(
        this: Option<&Self>,
        tp: &mut TypeParser<'_>,
        subject: SubjectPredicate<'_>,
        transformations: &[TransformationPredicate<'_>],
    ) -> bool {
        let Some(flow) = this else {
            return false;
        };
        PartialPipingFlow::matches_exactly(Some(&flow.partial), tp, subject, transformations)
    }

    // go: internal/typeparser/piping_flow.go:267 PipingFlow.FindTransformationSequences e414340c1c9b6c0e
    // FindTransformationSequences returns all consecutive matches, including overlaps.
    // It is defined explicitly to support a nil *PipingFlow.
    pub fn find_transformation_sequences<'a>(
        this: Option<&'a Self>,
        tp: &mut TypeParser<'_>,
        predicates: &[TransformationPredicate<'_>],
    ) -> Option<Vec<TransformationSequenceMatch<'a>>> {
        let flow = this?;
        PartialPipingFlow::find_transformation_sequences(Some(&flow.partial), tp, predicates)
    }

    // go: internal/typeparser/piping_flow.go:276 PipingFlow.CopyPrefix 6753a2854f641da4
    // CopyPrefix returns a partial copy containing the first transformationCount
    // transformations. It is defined explicitly to support a nil *PipingFlow.
    pub fn copy_prefix(
        this: Option<&Self>,
        transformation_count: i64,
    ) -> Option<PartialPipingFlow> {
        let flow = this?;
        PartialPipingFlow::copy_prefix(Some(&flow.partial), transformation_count)
    }

    // go: internal/typeparser/piping_flow.go:284 PipingFlow.TransformationInputType bd902de0f3b75b6d
    // TransformationInputType returns the type immediately before the indexed step.
    pub fn transformation_input_type(
        this: Option<&Self>,
        tp: &mut TypeParser<'_>,
        index: i64,
    ) -> Option<P<Type>> {
        let flow = this?;
        PartialPipingFlow::transformation_input_type(Some(&flow.partial), tp, index)
    }

    // go: internal/typeparser/piping_flow.go:293 PipingFlow.TransformationInputNode a0a205da38f7dab1
    // TransformationInputNode returns the source expression that feeds the
    // transformation at index.
    pub fn transformation_input_node(this: Option<&Self>, index: i64) -> Option<P<Node>> {
        let flow = this?;
        PartialPipingFlow::transformation_input_node(Some(&flow.partial), index)
    }
}

// go: internal/typeparser/piping_flow.go:301 ParsedPipeCallResult 1bd698470b04f494
// ParsedPipeCallResult is the result of parsing a pipe or pipeable call.
pub struct ParsedPipeCallResult {
    pub(crate) node: P<Node>,
    pub(crate) subject: P<Node>,
    pub(crate) args: &'static [P<Node>],
    pub(crate) kind: TransformationKind,
    pub(crate) subject_type: Lazy<Option<P<Type>>>,
    pub(crate) args_out_type: Lazy<Vec<Option<P<Type>>>>,
}

impl ParsedPipeCallResult {
    // go: internal/typeparser/piping_flow.go:310 ParsedPipeCallResult.Node 903fc35fc08da0ec
    // Rust: the call expression node (Go's `*ast.CallExpression`).
    pub fn node(&self) -> P<Node> {
        self.node
    }

    // go: internal/typeparser/piping_flow.go:312 ParsedPipeCallResult.Subject 79e43e235e75952e
    pub fn subject(&self) -> P<Node> {
        self.subject
    }

    // go: internal/typeparser/piping_flow.go:314 ParsedPipeCallResult.Args 90daf93d2aa684be
    pub fn args(&self) -> Slice<'_, P<Node>> {
        ets_readonly::new(self.args)
    }

    // go: internal/typeparser/piping_flow.go:316 ParsedPipeCallResult.Kind ad85fdfa5471c892
    pub fn kind(&self) -> TransformationKind {
        self.kind
    }

    // go: internal/typeparser/piping_flow.go:318 ParsedPipeCallResult.SubjectType 976496d5ede6793c
    // Rust: the lazy resolves through `tp` (see `Lazy`).
    pub fn subject_type(&self, tp: &mut TypeParser<'_>) -> Option<P<Type>> {
        Lazy::get(Some(&self.subject_type), tp)
    }

    // go: internal/typeparser/piping_flow.go:320 ParsedPipeCallResult.ArgsOutType fe27aa7ca94e7232
    // Rust: the lazy resolves through `tp` (see `Lazy`); Go's view of the resolved slice is a copy of it.
    pub fn args_out_type(&self, tp: &mut TypeParser<'_>) -> Vec<Option<P<Type>>> {
        Lazy::get(Some(&self.args_out_type), tp)
    }
}

// go: internal/typeparser/piping_flow.go:325 parsedSingleArgCallResult 4b1611a2642d753f
// parsedSingleArgCallResult is the internal result of parsing a single-argument call.
pub(crate) struct ParsedSingleArgCallResult {
    node: P<Node>,               // the applied call
    callee: P<Node>,             // the applied function, decomposed for curried factories
    args: Option<Vec<P<Node>>>,  // curried factory arguments, or nil
    type_arguments_src: P<Node>, // the node carrying the transformation type arguments
    subject: P<Node>,
}

// go: internal/typeparser/piping_flow.go:524 parsedEffectFnCallResult e4c724ad6f9a6bde
// parsedEffectFnCallResult is the internal result of parsing an Effect.fn or Effect.fnUntraced call
// with trailing transformation arguments.
pub(crate) struct ParsedEffectFnCallResult {
    node: P<Node>,                     // the outer call expression
    body_node: Option<P<Node>>,        // function or generator argument node
    trailing_args: &'static [P<Node>], // arguments after the function body
    trailing_args_out_type: Vec<Lazy<Option<P<Type>>>>,
    trailing_start_index: usize, // starting arg index of trailingArgs in node.Arguments
    kind: TransformationKind,    // effectFn or effectFnUntraced
}

// go: internal/typeparser/piping_flow.go:562 workItem 3e0a3d36c69e4595
// workItem represents a node to process in the PipingFlows work queue.
pub(crate) struct WorkItem {
    node: P<Node>,
    parent_flow: Option<Box<PipingFlow>>, // non-nil when traversing subject chain for flattening
}

/// Rust-only: Go's `enqueueChild` closure over the work queue, applied to each child of `node`.
fn enqueue_children(queue: &mut Vec<WorkItem>, node: P<Node>) {
    node.for_each_child(&mut |child| {
        queue.push(WorkItem { node: child, parent_flow: None });
        false
    });
}

impl TypeParser<'_> {
    // go: internal/typeparser/piping_flow.go:335 TypeParser.ParsePipeCall 5a97e6314fb3f1cb
    // ParsePipeCall detects pipe() and .pipe() call patterns.
    // Returns nil when the node is not a recognized pipe call.
    pub fn parse_pipe_call(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> Option<Arc<ParsedPipeCallResult>> {
        let node = node.into()?;
        if node.kind() != Kind::CallExpression {
            return None;
        }

        cached(
            self,
            |l| &mut l.parse_pipe_call,
            node,
            |tp| {
                let call = node.as_call_expression();
                let expression = call.expression();
                let arguments = call.arguments().nodes();

                // Case 1: PropertyAccessExpression — either Namespace.pipe(...) or expr.pipe(...)
                if expression.kind() == Kind::PropertyAccessExpression {
                    let prop_access = expression.as_property_access_expression();

                    let name_text = ets_shim::scanner::get_text_of_node(prop_access.name());
                    if name_text != "pipe" {
                        return None;
                    }

                    // Check if this is Function.pipe from "effect" package
                    if tp.is_node_reference_to_effect_package_export(expression, "pipe") {
                        // This is pipe(subject, f1, f2, ...) via namespace access (e.g., Function.pipe)
                        if arguments.is_empty() {
                            return None;
                        }
                        return Some(tp.build_parsed_pipe_call_result(
                            node,
                            arguments[0],
                            &arguments[1..],
                            TransformationKind::Pipe,
                        ));
                    }

                    // Not from "effect" package — this is a .pipe() pipeable method call
                    // Any .pipe() call is treated as a pipeable chain
                    let subject = prop_access.expression();
                    return Some(tp.build_parsed_pipe_call_result(
                        node,
                        subject,
                        arguments,
                        TransformationKind::Pipeable,
                    ));
                }

                // Case 2: Identifier — bare pipe(subject, f1, f2, ...)
                if expression.kind() == Kind::Identifier {
                    if arguments.is_empty() {
                        return None;
                    }

                    if !tp.is_node_reference_to_effect_package_export(expression, "pipe") {
                        return None;
                    }
                    return Some(tp.build_parsed_pipe_call_result(
                        node,
                        arguments[0],
                        &arguments[1..],
                        TransformationKind::Pipe,
                    ));
                }

                None
            },
        )
    }

    // go: internal/typeparser/piping_flow.go:396 TypeParser.buildParsedPipeCallResult bb8da395f1f2783d
    pub(crate) fn build_parsed_pipe_call_result(
        &self,
        call: P<Node>,
        subject: P<Node>,
        args: &'static [P<Node>],
        kind: TransformationKind,
    ) -> Arc<ParsedPipeCallResult> {
        let arg_count = args.len();
        Arc::new(ParsedPipeCallResult {
            node: call,
            subject,
            args,
            kind,
            subject_type: self.lazy_type_at_location(subject),
            args_out_type: new_lazy(move |tp| tp.build_pipe_args_out_type(call, arg_count)),
        })
    }

    // go: internal/typeparser/piping_flow.go:412 TypeParser.buildPipeArgsOutType f0ef335404baa86e
    pub(crate) fn build_pipe_args_out_type(
        &mut self,
        call: P<Node>,
        arg_count: usize,
    ) -> Vec<Option<P<Type>>> {
        let mut out_types = vec![None; arg_count];
        // Go: GetResolvedSignature never returns nil.
        let sig = self.checker.get_resolved_signature_exported(call);
        let type_args =
            get_type_arguments_for_resolved_signature(self.checker, Some(sig)).unwrap_or_default();
        for (i, out_type) in out_types.iter_mut().enumerate() {
            if i + 1 < type_args.len() {
                *out_type = Some(type_args[i + 1]);
            }
        }
        out_types
    }

    // go: internal/typeparser/piping_flow.go:429 TypeParser.parseSingleArgCall aa8bd49e7eaca65a
    // parseSingleArgCall detects single-argument call patterns like f(arg).
    // Returns nil when the node is not a single-argument call.
    pub(crate) fn parse_single_arg_call(
        &mut self,
        node: P<Node>,
    ) -> Option<ParsedSingleArgCallResult> {
        if node.kind() != Kind::CallExpression {
            return None;
        }

        let call = node.as_call_expression();
        let arguments = call.arguments().nodes();

        if arguments.len() != 1 {
            return None;
        }

        // Spread elements like f(...args) should not be treated as single-arg calls
        if arguments[0].kind() == Kind::SpreadElement {
            return None;
        }

        let mut result = ParsedSingleArgCallResult {
            node,
            callee: call.expression(),
            args: None,
            type_arguments_src: node,
            subject: arguments[0],
        };
        self.decompose_curried_pipeable(&mut result);
        Some(result)
    }

    // go: internal/typeparser/piping_flow.go:460 TypeParser.decomposeCurriedPipeable c6f76541985927f9
    // decomposeCurriedPipeable exposes the factory arguments only when the inner
    // call is the pipeable counterpart of a data-first overload of the same symbol.
    pub(crate) fn decompose_curried_pipeable(&mut self, result: &mut ParsedSingleArgCallResult) {
        let factory =
            ets_shim::ast::skip_parentheses(result.node.as_call_expression().expression());
        if factory.kind() != Kind::CallExpression {
            return;
        }
        let factory_call = factory.as_call_expression();
        let factory_arguments = factory_call.arguments().nodes();
        if factory_arguments.is_empty() {
            return;
        }

        // Go: GetResolvedSignature never returns nil.
        let pipeable = self.checker.get_resolved_signature_exported(factory);
        let Some(pipeable_declaration) =
            raw_signature(Some(pipeable)).and_then(|s| s.declaration())
        else {
            return;
        };
        let Some(pipeable_symbol) = get_symbol_of_declaration(self.checker, pipeable_declaration)
        else {
            return;
        };

        let callee_type = self.get_type_at_location(factory_call.expression());
        let subject_type = self.get_type_at_location(result.subject);
        let (Some(callee_type), Some(subject_type)) = (callee_type, subject_type) else {
            return;
        };
        let mut argument_types = Vec::with_capacity(factory_arguments.len());
        for &argument in factory_arguments {
            argument_types.push(self.get_type_at_location(argument));
        }
        let witness = PipeableSignatureWitness { argument_types, subject_type: Some(subject_type) };

        for &data_first in
            self.checker.get_signatures_of_type_exported(callee_type, SignatureKind::Call)
        {
            if data_first == pipeable {
                continue;
            }
            let Some(declaration) = raw_signature(Some(data_first)).and_then(|s| s.declaration())
            else {
                continue;
            };
            let Some(symbol) = get_symbol_of_declaration(self.checker, declaration) else {
                continue;
            };
            if get_symbol_if_same_reference(self.checker, Some(pipeable_symbol), Some(symbol))
                .is_none()
            {
                continue;
            }
            let parameters = data_first.parameters();
            for subject_index in [0, parameters.len() as i64 - 1] {
                if matches_pipeable_signature(
                    self,
                    Some(data_first),
                    Some(pipeable),
                    subject_index,
                    Some(&witness),
                ) {
                    result.callee = factory_call.expression();
                    result.args = Some(factory_arguments.to_vec());
                    result.type_arguments_src = factory;
                    return;
                }
            }
        }
    }

    // go: internal/typeparser/piping_flow.go:535 TypeParser.parseEffectFnCall d52ec751a9f44bce
    // parseEffectFnCall detects Effect.fn-family calls with trailing transformation arguments.
    // It reuses the dedicated Effect.fn parsers and only adapts their results for piping-flow analysis.
    pub(crate) fn parse_effect_fn_call(
        &mut self,
        node: P<Node>,
    ) -> Option<ParsedEffectFnCallResult> {
        if node.kind() != Kind::CallExpression {
            return None;
        }

        let call = node.as_call_expression();
        if call.arguments().nodes().is_empty() {
            return None;
        }
        if let Some(result) = self.effect_fn_call(node)
            && !result.pipe_arguments.unwrap_or_default().is_empty()
        {
            let mut kind = TransformationKind::EffectFn;
            if result.variant == EffectFnVariant::FnUntraced
                || result.variant == EffectFnVariant::FnUntracedEager
            {
                kind = TransformationKind::EffectFnUntraced;
            }
            let pipe_arguments = result.pipe_arguments.unwrap_or_default();
            return Some(ParsedEffectFnCallResult {
                node: result.call,
                body_node: Some(result.function_node),
                trailing_start_index: result.call.as_call_expression().arguments().nodes().len()
                    - pipe_arguments.len(),
                trailing_args: pipe_arguments,
                trailing_args_out_type: result.pipe_args_out_type.clone(),
                kind,
            });
        }
        None
    }

    // go: internal/typeparser/piping_flow.go:569 TypeParser.PipingFlows 3f2a1b11ce9d79ea
    // PipingFlows returns all piping flows found in a source file, sorted by source position.
    // Each source transformation occurrence belongs to at most one returned flow.
    //
    // Rust: Go returns a read-only view of the cached slice; here the cache is behind the links' lock, so it hands
    // out a shared handle (`ets_readonly::new(flows.as_deref())` is Go's view; `None` is a nil slice).
    pub fn piping_flows(
        &mut self,
        sf: impl Into<Option<P<SourceFile>>>,
        include_effect_fn: bool,
    ) -> Option<Arc<[PipingFlow]>> {
        let sf = sf.into()?;

        let store: fn(&mut EffectLinks) -> &mut _ = if include_effect_fn {
            |l| &mut l.piping_flows_with_effect_fn
        } else {
            |l| &mut l.piping_flows_without_effect_fn
        };

        cached(self, store, sf, |tp| {
            let mut result: Vec<PipingFlow> = Vec::new();

            // Initialize work queue with all children of the source file
            let mut queue: Vec<WorkItem> = Vec::new();
            enqueue_children(&mut queue, sf.as_node());

            // Pop from end (stack behavior for depth-first)
            while let Some(item) = queue.pop() {
                let node = item.node;
                let mut parent_flow = item.parent_flow;
                let unwrapped = ets_shim::ast::skip_parentheses(node);
                if unwrapped != node {
                    queue.push(WorkItem { node: unwrapped, parent_flow });
                    continue;
                }

                if node.kind() == Kind::CallExpression {
                    // Try Effect.fn call first (must be before pipe and singleArg)
                    if include_effect_fn && let Some(efn_result) = tp.parse_effect_fn_call(node) {
                        let (transformations, subject_type) =
                            tp.build_effect_fn_transformations(&efn_result);
                        let flow = PipingFlow {
                            node,
                            partial: PartialPipingFlow {
                                subject: PipingFlowSubject {
                                    node: Some(node),
                                    out_type: Some(subject_type),
                                },
                                transformations,
                            },
                        };
                        result.push(flow);

                        // If we were building a parent flow, finalize it with this node as subject
                        if let Some(mut parent) = parent_flow {
                            parent.partial.subject = PipingFlowSubject {
                                node: Some(node),
                                out_type: Some(tp.lazy_type_at_location(node)),
                            };
                            result.push(*parent);
                        }

                        // Queue function body argument children for independent inner flow traversal
                        if let Some(body_node) = efn_result.body_node {
                            enqueue_children(&mut queue, body_node);
                        }
                        // Queue trailing arg children for independent inner flow traversal
                        for &arg in efn_result.trailing_args {
                            enqueue_children(&mut queue, arg);
                        }
                        continue;
                    }

                    // Try pipe call
                    if let Some(pipe_result) = tp.parse_pipe_call(node) {
                        let transformations = tp.build_pipe_transformations(&pipe_result);
                        let flow_node = pipe_result.node;

                        if let Some(mut parent) = parent_flow {
                            // Extend parent flow: prepend our transformations
                            let mut extended = transformations;
                            extended.append(&mut parent.partial.transformations);
                            parent.partial.transformations = extended;
                            // Continue traversing the subject for further flattening
                            queue.push(WorkItem {
                                node: pipe_result.subject,
                                parent_flow: Some(parent),
                            });
                        } else {
                            // Start a new flow
                            let new_flow = PipingFlow {
                                node: flow_node,
                                partial: PartialPipingFlow {
                                    subject: PipingFlowSubject::default(),
                                    transformations,
                                },
                            };
                            queue.push(WorkItem {
                                node: pipe_result.subject,
                                parent_flow: Some(Box::new(new_flow)),
                            });
                        }

                        // Queue transformation argument children for independent inner flow traversal
                        for &arg in pipe_result.args {
                            enqueue_children(&mut queue, arg);
                        }
                        continue;
                    }

                    // Try single-arg call
                    if let Some(data_first_result) = tp.data_first_or_last_call(node) {
                        let mut kind = TransformationKind::DataFirst;
                        if data_first_result.subject_index != 0 {
                            kind = TransformationKind::DataLast;
                        }
                        let transformation = PipingFlowTransformation {
                            kind,
                            callee: data_first_result.callee,
                            type_arguments: piping_flow_type_arguments(
                                node,
                                data_first_result.callee,
                            ),
                            args: Some(data_first_result.args.clone()),
                            out_type: Some(tp.lazy_type_at_location(node)),
                        };

                        if let Some(mut parent) = parent_flow {
                            parent.partial.transformations.insert(0, transformation);
                            queue.push(WorkItem {
                                node: data_first_result.subject,
                                parent_flow: Some(parent),
                            });
                        } else {
                            let new_flow = PipingFlow {
                                node,
                                partial: PartialPipingFlow {
                                    subject: PipingFlowSubject::default(),
                                    transformations: vec![transformation],
                                },
                            };
                            queue.push(WorkItem {
                                node: data_first_result.subject,
                                parent_flow: Some(Box::new(new_flow)),
                            });
                        }

                        enqueue_children(&mut queue, data_first_result.callee);
                        for &arg in &data_first_result.args {
                            enqueue_children(&mut queue, arg);
                        }
                        continue;
                    }

                    // Try single-arg call
                    if let Some(single_result) = tp.parse_single_arg_call(node) {
                        let transformation = PipingFlowTransformation {
                            kind: TransformationKind::Call,
                            callee: single_result.callee,
                            type_arguments: piping_flow_type_arguments(
                                single_result.type_arguments_src,
                                single_result.callee,
                            ),
                            args: single_result.args.clone(),
                            out_type: Some(tp.lazy_call_return_type(node)),
                        };

                        if let Some(mut parent) = parent_flow {
                            // Extend parent flow: prepend this transformation
                            parent.partial.transformations.insert(0, transformation);
                            // Continue traversing the subject
                            queue.push(WorkItem {
                                node: single_result.subject,
                                parent_flow: Some(parent),
                            });
                        } else {
                            // Start a new flow
                            let new_flow = PipingFlow {
                                node,
                                partial: PartialPipingFlow {
                                    subject: PipingFlowSubject::default(),
                                    transformations: vec![transformation],
                                },
                            };
                            queue.push(WorkItem {
                                node: single_result.subject,
                                parent_flow: Some(Box::new(new_flow)),
                            });
                        }

                        // Queue callee children for independent inner flow traversal
                        if let Some(args) =
                            single_result.args.as_ref().filter(|args| !args.is_empty())
                        {
                            // Curried factory: the applied function and the factory
                            // arguments belong to this transformation's subtree.
                            queue.push(WorkItem { node: single_result.callee, parent_flow: None });
                            for &arg in args {
                                queue.push(WorkItem { node: arg, parent_flow: None });
                            }
                        } else {
                            enqueue_children(&mut queue, single_result.callee);
                        }
                        continue;
                    }
                }

                // Node is not a parseable pipe/call
                if let Some(mut parent) = parent_flow.take() {
                    // Subject chain terminated — finalize the flow
                    parent.partial.subject = PipingFlowSubject {
                        node: Some(node),
                        out_type: Some(tp.lazy_type_at_location(node)),
                    };
                    result.push(*parent);
                }
                // Queue children for further traversal (independent inner flows)
                enqueue_children(&mut queue, node);
            }

            // Sort by source position
            ets_go::sort::slice(&mut result, |a, b| a.node.pos() < b.node.pos());

            // Go: a file without flows caches a nil slice.
            if result.is_empty() { None } else { Some(Arc::from(result)) }
        })
    }

    // go: internal/typeparser/piping_flow.go:786 TypeParser.LongestPipingFlowAt 2d7492d6628f5fe1
    // LongestPipingFlowAt returns the longest normalized piping flow rooted at node.
    // Unlike PipingFlows, which discovers complete flows across a source file, this
    // method can start at an expression nested inside another flow without returning
    // the enclosing flow.
    pub fn longest_piping_flow_at(
        &mut self,
        node: impl Into<Option<P<Node>>>,
        include_effect_fn: bool,
    ) -> Option<PipingFlow> {
        let node = node.into()?;
        if !ets_shim::ast::is_expression(node) {
            return None;
        }

        let node = ets_shim::ast::skip_parentheses(node);
        if include_effect_fn && let Some(result) = self.parse_effect_fn_call(node) {
            let (transformations, subject_type) = self.build_effect_fn_transformations(&result);
            return Some(PipingFlow {
                node,
                partial: PartialPipingFlow {
                    subject: PipingFlowSubject { node: Some(node), out_type: Some(subject_type) },
                    transformations,
                },
            });
        }

        if let Some(result) = self.parse_pipe_call(node) {
            let mut flow = self.piping_flow_subject_at(result.subject, include_effect_fn);
            flow.node = node;
            flow.partial.transformations.extend(self.build_pipe_transformations(&result));
            return Some(flow);
        }

        if let Some(result) = self.data_first_or_last_call(node) {
            let mut flow = self.piping_flow_subject_at(result.subject, include_effect_fn);
            let mut kind = TransformationKind::DataFirst;
            if result.subject_index != 0 {
                kind = TransformationKind::DataLast;
            }
            flow.node = node;
            flow.partial.transformations.push(PipingFlowTransformation {
                kind,
                callee: result.callee,
                type_arguments: piping_flow_type_arguments(node, result.callee),
                args: Some(result.args.clone()),
                out_type: Some(self.lazy_type_at_location(node)),
            });
            return Some(flow);
        }

        if let Some(result) = self.parse_single_arg_call(node) {
            let mut flow = self.piping_flow_subject_at(result.subject, include_effect_fn);
            flow.node = node;
            flow.partial.transformations.push(PipingFlowTransformation {
                kind: TransformationKind::Call,
                callee: result.callee,
                type_arguments: piping_flow_type_arguments(
                    result.type_arguments_src,
                    result.callee,
                ),
                args: result.args,
                out_type: Some(self.lazy_call_return_type(node)),
            });
            return Some(flow);
        }

        Some(PipingFlow {
            node,
            partial: PartialPipingFlow {
                subject: PipingFlowSubject {
                    node: Some(node),
                    out_type: Some(self.lazy_type_at_location(node)),
                },
                transformations: Vec::new(),
            },
        })
    }

    // go: internal/typeparser/piping_flow.go:856 TypeParser.pipingFlowSubjectAt 292214e8d471e9d3
    pub(crate) fn piping_flow_subject_at(
        &mut self,
        node: P<Node>,
        include_effect_fn: bool,
    ) -> PipingFlow {
        if let Some(flow) = self.longest_piping_flow_at(node, include_effect_fn) {
            return flow;
        }
        PipingFlow {
            node,
            partial: PartialPipingFlow {
                subject: PipingFlowSubject {
                    node: Some(node),
                    out_type: Some(self.lazy_type_at_location(node)),
                },
                transformations: Vec::new(),
            },
        }
    }

    // go: internal/typeparser/piping_flow.go:872 TypeParser.buildPipeTransformations d4bae32887740224
    // buildPipeTransformations builds PipingFlowTransformation slices from pipe call arguments.
    #[expect(
        clippy::unused_self,
        reason = "a TypeParser method in Go; its lazies resolve through the parser they are read with"
    )]
    pub(crate) fn build_pipe_transformations(
        &self,
        result: &Arc<ParsedPipeCallResult>,
    ) -> Vec<PipingFlowTransformation> {
        let mut transformations = Vec::with_capacity(result.args.len());
        for (i, &arg) in result.args.iter().enumerate() {
            let shared = Arc::clone(result);
            let out_type = new_lazy(move |tp| {
                let out_types = Lazy::get(Some(&shared.args_out_type), tp);
                if i < out_types.len() {
                    return out_types[i];
                }
                None
            });

            let callee;
            let mut args = None;

            if arg.kind() == Kind::CallExpression {
                let call_expr = arg.as_call_expression();
                callee = call_expr.expression();
                let arguments = call_expr.arguments().nodes();
                if !arguments.is_empty() {
                    args = Some(arguments.to_vec());
                }
            } else {
                // Constant (e.g., Effect.asVoid used as a bare reference)
                callee = arg;
            }

            transformations.push(PipingFlowTransformation {
                kind: result.kind,
                callee,
                type_arguments: piping_flow_type_arguments(arg, callee),
                args,
                out_type: Some(out_type),
            });
        }

        transformations
    }

    // go: internal/typeparser/piping_flow.go:914 TypeParser.buildEffectFnTransformations 2a27058e0df45ca0
    // buildEffectFnTransformations builds PipingFlowTransformation slices from Effect.fn trailing arguments.
    // It also returns the subject type (from the first transformation's input parameter type).
    #[expect(
        clippy::unused_self,
        reason = "a TypeParser method in Go; its lazies resolve through the parser they are read with"
    )]
    pub(crate) fn build_effect_fn_transformations(
        &self,
        result: &ParsedEffectFnCallResult,
    ) -> (Vec<PipingFlowTransformation>, Lazy<Option<P<Type>>>) {
        let mut transformations = Vec::with_capacity(result.trailing_args.len());
        let node = result.node;
        let first_trailing_arg = result.trailing_args.first().copied();
        let trailing_start_index = result.trailing_start_index;
        let subject_type = new_lazy(move |tp| {
            tp.effect_fn_subject_type(node, first_trailing_arg, trailing_start_index)
        });

        for (i, &arg) in result.trailing_args.iter().enumerate() {
            let mut out_type = None;
            if i < result.trailing_args_out_type.len() {
                out_type = Some(result.trailing_args_out_type[i].clone());
            }

            let callee;
            let mut args = None;

            if arg.kind() == Kind::CallExpression {
                let call_expr = arg.as_call_expression();
                callee = call_expr.expression();
                let arguments = call_expr.arguments().nodes();
                if !arguments.is_empty() {
                    args = Some(arguments.to_vec());
                }
            } else {
                callee = arg;
            }

            transformations.push(PipingFlowTransformation {
                kind: result.kind,
                callee,
                type_arguments: piping_flow_type_arguments(arg, callee),
                args,
                out_type,
            });
        }

        (transformations, subject_type)
    }

    // go: internal/typeparser/piping_flow.go:955 TypeParser.effectFnSubjectType cc1619dbecdc1047
    // effectFnSubjectType is the first parameter type of the contextual signature
    // of the first trailing transformation.
    //
    // Rust: Go reads the call, the first trailing argument and its index from the parsed result; the lazy that
    // calls this captures those three values.
    pub(crate) fn effect_fn_subject_type(
        &mut self,
        node: P<Node>,
        first_trailing_arg: Option<P<Node>>,
        trailing_start_index: usize,
    ) -> Option<P<Type>> {
        first_trailing_arg?;
        let contextual_type = self
            .checker
            .get_contextual_type_for_argument_at_index_exported(node, trailing_start_index);
        let call_sigs =
            self.checker.get_signatures_of_type_exported(contextual_type, SignatureKind::Call);
        if call_sigs.is_empty() {
            return None;
        }
        let params = call_sigs[0].parameters();
        if params.is_empty() {
            return None;
        }
        Some(self.checker.get_type_of_symbol_exported(params[0]))
    }
}

// go: internal/typeparser/piping_flow.go:975 callTypeArguments 9918c7ce636290a7
pub(crate) fn call_type_arguments(node: P<Node>) -> Option<P<NodeList>> {
    // Parenthesized call arguments (e.g. pipe(x, (Effect.as<...>(v)))) keep the
    // type arguments of the wrapped call.
    let node = ets_shim::ast::skip_parentheses(node);
    if node.kind() != Kind::CallExpression {
        return None;
    }
    node.as_call_expression().type_arguments()
}

// go: internal/typeparser/piping_flow.go:985 pipingFlowTypeArguments 14420671cde7b020
pub(crate) fn piping_flow_type_arguments(node: P<Node>, callee: P<Node>) -> Option<P<NodeList>> {
    if let Some(type_arguments) = call_type_arguments(node) {
        return Some(type_arguments);
    }
    call_type_arguments(callee)
}
