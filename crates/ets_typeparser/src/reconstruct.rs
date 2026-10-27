use ets_shim::ast::{Node, SourceFile};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/reconstruct.go:11 nodeText 10e7b1d9ea8a6af6
// nodeText extracts the source text of a node from the source file.
//
// Rust: Go slices the text's bytes; a node always spans whole characters, so the lossy conversion never changes
// the text.
pub(crate) fn node_text(sf: Option<P<SourceFile>>, node: Option<P<Node>>) -> String {
    let (Some(node), Some(sf)) = (node, sf) else {
        return String::new();
    };
    let text = sf.text().as_bytes();
    let pos = node.pos().source_offset().map_or(-1, i64::from);
    let end = node.end().source_offset().map_or(-1, i64::from);
    if pos >= 0 && end >= pos && end <= text.len() as i64 {
        return String::from_utf8_lossy(&text[pos as usize..end as usize]).into_owned();
    }
    String::new()
}

// go: internal/typeparser/reconstruct.go:31 ReconstructPipingFlow 1d015cd11dce391f
// ReconstructPipingFlow reconstructs a piping flow into a string expression
// by applying transformations sequentially.
// For example: subject with transformations [f, g] becomes "g(f(subject))".
//
// Note: Effect.fn and Effect.fnUntraced transformations cannot be reconstructed
// as a chain since they are part of the Effect.fn call itself. In this case,
// the original subject node text is returned.
pub fn reconstruct_piping_flow(
    sf: impl Into<Option<P<SourceFile>>>,
    subject: Option<&PipingFlowSubject>,
    transformations: &[&PipingFlowTransformation],
) -> String {
    let (Some(sf), Some(subject)) = (sf.into(), subject) else {
        return String::new();
    };

    // Check if all transformations are effectFn or effectFnUntraced.
    // In this case, reconstruction is not possible - return the original node text.
    if !transformations.is_empty() {
        let mut all_effect_fn = true;
        for t in transformations {
            if t.kind != TransformationKind::EffectFn
                && t.kind != TransformationKind::EffectFnUntraced
            {
                all_effect_fn = false;
                break;
            }
        }
        if all_effect_fn {
            return node_text(Some(sf), subject.node);
        }
    }

    let mut result = node_text(Some(sf), subject.node);

    for t in transformations {
        if t.kind == TransformationKind::EffectFn || t.kind == TransformationKind::EffectFnUntraced
        {
            // Effect.fn transformations cannot be reconstructed as part of a chain
            continue;
        }

        let callee_text = node_text(Some(sf), Some(t.callee));

        if t.kind == TransformationKind::Call {
            // Single-arg call: callee(result)
            result = callee_text + "(" + &result + ")";
        } else {
            // Pipe or pipeable: apply the transformation
            let args = t.args.as_deref().unwrap_or_default();
            if !args.is_empty() {
                // Curried form: callee(args...)(result)
                let mut args_text = String::new();
                for (i, &arg) in args.iter().enumerate() {
                    if i > 0 {
                        args_text.push_str(", ");
                    }
                    args_text.push_str(&node_text(Some(sf), Some(arg)));
                }
                result = callee_text + "(" + &args_text + ")(" + &result + ")";
            } else {
                // Constant: callee(result)
                result = callee_text + "(" + &result + ")";
            }
        }
    }

    result
}
