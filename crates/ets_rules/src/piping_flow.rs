use ets_typeparser::{PipingFlowTransformation, TransformationKind};

// go: internal/rules/piping_flow.go:7 HasExplicitTypeArguments 91f871a7e72a1e34
// HasExplicitTypeArguments reports whether a piping flow step is called with
// explicit type arguments, which a rewrite to another API cannot carry over.
pub fn has_explicit_type_arguments(transformation: Option<&PipingFlowTransformation>) -> bool {
    transformation
        .and_then(PipingFlowTransformation::type_arguments)
        .is_some_and(|type_arguments| !type_arguments.nodes().is_empty())
}

// go: internal/rules/piping_flow.go:13 IsPipingFlowArgumentKind 9eb296bc5dfbf15f
// IsPipingFlowArgumentKind reports whether a step is written as an argument of
// a pipe, pipeable or Effect.fn call rather than applied to its input directly.
pub fn is_piping_flow_argument_kind(kind: TransformationKind) -> bool {
    matches!(
        kind,
        TransformationKind::Pipe
            | TransformationKind::Pipeable
            | TransformationKind::EffectFn
            | TransformationKind::EffectFnUntraced
    )
}
