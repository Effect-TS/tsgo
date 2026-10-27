use ets_typeparser::{PipingFlowTransformation, TransformationKind};

// go: internal/rules/piping_flow.go:7 hasExplicitTypeArguments
// hasExplicitTypeArguments reports whether a piping flow step is called with
// explicit type arguments, which a rewrite to another API cannot carry over.
pub(crate) fn has_explicit_type_arguments(
    transformation: Option<&PipingFlowTransformation>,
) -> bool {
    transformation
        .and_then(PipingFlowTransformation::type_arguments)
        .is_some_and(|type_arguments| !type_arguments.nodes().is_empty())
}

// go: internal/rules/piping_flow.go:13 isPipingFlowArgumentKind
// isPipingFlowArgumentKind reports whether a step is written as an argument of
// a pipe, pipeable or Effect.fn call rather than applied to its input directly.
pub(crate) fn is_piping_flow_argument_kind(kind: TransformationKind) -> bool {
    matches!(
        kind,
        TransformationKind::Pipe
            | TransformationKind::Pipeable
            | TransformationKind::EffectFn
            | TransformationKind::EffectFnUntraced
    )
}
