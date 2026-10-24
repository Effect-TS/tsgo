use ets_readonly::Slice;
use ets_shim::ast::{Node, NodeList, SourceFile};
use ets_shim::checker::Type;
use ets_shim::core::P;

use crate::*;

// go: etsgoapi/piping_flow.go:11 TransformationKind
// go: etsgoapi/piping_flow.go:13 TransformationKindPipe
// TransformationKind represents how a transformation was expressed in source code.
//
// Rust: Go's `type TransformationKind string` takes its constants from the type parser's; only those are ever
// stored (docs/PORTING.md, "Naming"). `as_str` is the Go string value.
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

/// Rust-only: Go's conversion `TransformationKind(kind)` from the type parser's kind.
impl From<ets_typeparser::TransformationKind> for TransformationKind {
    fn from(kind: ets_typeparser::TransformationKind) -> Self {
        match kind {
            ets_typeparser::TransformationKind::Pipe => TransformationKind::Pipe,
            ets_typeparser::TransformationKind::Pipeable => TransformationKind::Pipeable,
            ets_typeparser::TransformationKind::DataFirst => TransformationKind::DataFirst,
            ets_typeparser::TransformationKind::DataLast => TransformationKind::DataLast,
            ets_typeparser::TransformationKind::Call => TransformationKind::Call,
            ets_typeparser::TransformationKind::EffectFn => TransformationKind::EffectFn,
            ets_typeparser::TransformationKind::EffectFnUntraced => {
                TransformationKind::EffectFnUntraced
            }
        }
    }
}

// go: etsgoapi/piping_flow.go:24 PipingFlowTransformation
// PipingFlowTransformation represents a single transformation step in a piping flow.
//
// Rust: Go's `args` view shares the type parser's slice; here it is a copy (`None` is Go's nil slice).
#[derive(Clone, Debug)]
pub struct PipingFlowTransformation {
    pub(crate) kind: TransformationKind, // How the transformation was expressed
    pub(crate) callee: P<Node>,          // The function being applied (e.g., Effect.map)
    pub(crate) type_arguments: Option<P<NodeList>>, // Explicit type arguments to the transformation call, if any
    pub(crate) args: Option<Vec<P<Node>>>, // Arguments to the transformation, or nil for constants/single-arg calls
    pub(crate) out_type: Option<P<Type>>, // The resulting type after this transformation (may be nil)
}

impl PipingFlowTransformation {
    // go: etsgoapi/piping_flow.go:32 PipingFlowTransformation.Kind
    pub fn kind(&self) -> TransformationKind {
        self.kind
    }

    // go: etsgoapi/piping_flow.go:34 PipingFlowTransformation.Callee
    pub fn callee(&self) -> P<Node> {
        self.callee
    }

    // go: etsgoapi/piping_flow.go:36 PipingFlowTransformation.TypeArguments
    pub fn type_arguments(&self) -> Option<P<NodeList>> {
        self.type_arguments
    }

    // go: etsgoapi/piping_flow.go:38 PipingFlowTransformation.Args
    pub fn args(&self) -> Slice<'_, P<Node>> {
        ets_readonly::new(self.args.as_deref())
    }

    // go: etsgoapi/piping_flow.go:40 PipingFlowTransformation.OutType
    pub fn out_type(&self) -> Option<P<Type>> {
        self.out_type
    }
}

// go: etsgoapi/piping_flow.go:43 PipingFlowSubject
// PipingFlowSubject is the starting expression of a piping flow.
#[derive(Clone, Copy, Debug)]
pub struct PipingFlowSubject {
    pub(crate) node: Option<P<Node>>,     // The expression node
    pub(crate) out_type: Option<P<Type>>, // The type of the subject expression (may be nil)
}

impl PipingFlowSubject {
    // go: etsgoapi/piping_flow.go:48 PipingFlowSubject.Node
    pub fn node(self) -> Option<P<Node>> {
        self.node
    }

    // go: etsgoapi/piping_flow.go:50 PipingFlowSubject.OutType
    pub fn out_type(self) -> Option<P<Type>> {
        self.out_type
    }
}

// go: etsgoapi/piping_flow.go:53 PipingFlow
// PipingFlow represents a complete piping flow: a subject followed by transformations.
#[derive(Clone, Debug)]
pub struct PipingFlow {
    pub(crate) node: P<Node>, // The outermost expression encompassing the entire flow
    pub(crate) subject: PipingFlowSubject, // The starting expression and its type
    pub(crate) transformations: Vec<PipingFlowTransformation>, // Ordered list of transformations
}

impl PipingFlow {
    // go: etsgoapi/piping_flow.go:59 PipingFlow.Node
    pub fn node(&self) -> P<Node> {
        self.node
    }

    // go: etsgoapi/piping_flow.go:61 PipingFlow.Subject
    pub fn subject(&self) -> PipingFlowSubject {
        self.subject
    }

    // go: etsgoapi/piping_flow.go:63 PipingFlow.Transformations
    pub fn transformations(&self) -> Slice<'_, PipingFlowTransformation> {
        ets_readonly::new(&self.transformations[..])
    }
}

impl TypeParser<'_> {
    // go: etsgoapi/piping_flow.go:71 TypeParser.PipingFlows
    // PipingFlows returns all piping flows found in a source file, sorted by source position.
    // Each source transformation occurrence belongs to at most one returned flow.
    // When includeEffectFn is true, Effect.fn / Effect.fnUntraced calls carrying trailing
    // transformation arguments are surfaced as flows as well.
    //
    // Rust: Go's `[]*PipingFlow` is `Option<Vec<PipingFlow>>` (`None` is a nil slice). Every flow's lazy types are
    // resolved here, in Go's order, before anything is returned: that order decides type ids and so the union order
    // of printed types.
    pub fn piping_flows(
        &mut self,
        sf: impl Into<Option<P<SourceFile>>>,
        include_effect_fn: bool,
    ) -> Option<Vec<PipingFlow>> {
        let flows = self.inner.piping_flows(sf, include_effect_fn);
        piping_flows_from_internal(&mut self.inner, ets_readonly::new(flows.as_deref()))
    }
}

impl TypeParser<'_> {
    // go: etsgoapi/piping_flow.go:80 TypeParser.LongestPipingFlowAt
    // LongestPipingFlowAt returns the longest normalized piping flow rooted at node,
    // without returning a larger flow that merely encloses node.
    pub fn longest_piping_flow_at(
        &mut self,
        node: impl Into<Option<P<Node>>>,
        include_effect_fn: bool,
    ) -> Option<PipingFlow> {
        let flow = self.inner.longest_piping_flow_at(node, include_effect_fn);
        piping_flow_from_internal(&mut self.inner, flow.as_ref())
    }
}

// go: etsgoapi/piping_flow.go:87 pipingFlowsFromInternal
// Rust: the parser resolves the flows' lazy types (Go's lazies capture it).
pub(crate) fn piping_flows_from_internal(
    tp: &mut ets_typeparser::TypeParser<'_>,
    flows: Slice<'_, ets_typeparser::PipingFlow>,
) -> Option<Vec<PipingFlow>> {
    if flows.is_nil() {
        return None;
    }
    let mut result = Vec::new();
    for (_, flow) in flows.all() {
        if let Some(converted) = piping_flow_from_internal(tp, Some(flow)) {
            result.push(converted);
        }
    }
    Some(result)
}

// go: etsgoapi/piping_flow.go:100 pipingFlowFromInternal
// Rust: the parser resolves the flow's lazy types: each transformation's, in order, then the subject's.
pub(crate) fn piping_flow_from_internal(
    tp: &mut ets_typeparser::TypeParser<'_>,
    flow: Option<&ets_typeparser::PipingFlow>,
) -> Option<PipingFlow> {
    let flow = flow?;

    let mut transformations = Vec::new();
    for (_, transformation) in flow.transformations().all() {
        transformations.push(PipingFlowTransformation {
            kind: TransformationKind::from(transformation.kind()),
            callee: transformation.callee(),
            type_arguments: transformation.type_arguments(),
            args: transformation.args().copy(),
            out_type: transformation.out_type(tp),
        });
    }

    let subject = flow.subject();
    Some(PipingFlow {
        node: flow.node(),
        subject: PipingFlowSubject { node: subject.node(), out_type: subject.out_type(tp) },
        transformations,
    })
}

impl PipingFlow {
    // go: etsgoapi/piping_flow.go:128 PipingFlow.TransformationInputType
    // TransformationInputType returns the type immediately before the indexed step.
    // Missing types and indices outside the flow return nil.
    //
    // Rust: Go's nil-receiver check drops (a `&self` is never nil).
    pub fn transformation_input_type(&self, index: i64) -> Option<P<Type>> {
        let Ok(index) = usize::try_from(index) else {
            return None;
        };
        if index >= self.transformations.len() {
            return None;
        }
        if index == 0 {
            return self.subject.out_type;
        }
        self.transformations[index - 1].out_type
    }
}
