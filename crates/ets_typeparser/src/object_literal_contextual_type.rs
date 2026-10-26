use std::panic::{AssertUnwindSafe, resume_unwind};

use ets_go::panics::recover;
use ets_shim::ast::{Kind, Node};
use ets_shim::checker::{
    Checker, ContextFlags, InferenceFlags, InferencePriority, Signature, Type, new_type_mapper,
};
use ets_shim::core::P;
use ets_shim::core::ptr::alloc_slice;

use crate::*;

impl TypeParser<'_> {
    // go: internal/typeparser/object_literal_contextual_type.go:10 TypeParser.CachedObjectLiteralContextualType
    pub fn cached_object_literal_contextual_type(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> Option<P<Type>> {
        let node = node.into()?;
        if node.kind() != Kind::ObjectLiteralExpression {
            return None;
        }
        cached(
            self,
            |l| &mut l.object_literal_contextual_type,
            node,
            |tp| tp.get_contextual_type(node, ContextFlags::None),
        )
    }

    // go: internal/typeparser/object_literal_contextual_type.go:33 TypeParser.CachedObjectLiteralConstraintContextualType
    // CachedObjectLiteralConstraintContextualType returns the contextual type of an
    // object literal inside a generic call argument, as if the argument contributed
    // no inferences to the call's resolved signature. Type parameters then take
    // their return type inferences, inferences from the other arguments, their
    // defaults, or their constraints, as the checker infers them. It returns nil
    // when the literal is not inside a call argument or the signature is not
    // generic.
    //
    // The call is never resolved again. An inference-blocked contextual query
    // (ContextFlagsIgnoreNodeInferences) would drop the resolved signatures of the
    // enclosing calls and resolve them with the argument's inferences removed. On
    // the shared checker this can instantiate types the original check never
    // built, for example Match.when patterns over a wide union, and leak
    // TS2589 errors and error types into the program's diagnostics.
    pub fn cached_object_literal_constraint_contextual_type(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> Option<P<Type>> {
        let node = node.into()?;
        if node.kind() != Kind::ObjectLiteralExpression {
            return None;
        }
        cached(
            self,
            |l| &mut l.object_literal_constraint_contextual_type,
            node,
            |tp| tp.object_literal_constraint_contextual_type(node),
        )
    }

    // go: internal/typeparser/object_literal_contextual_type.go:42 TypeParser.objectLiteralConstraintContextualType
    // Rust: Go's deferred pops run on every exit, a panic included (the panic is re-raised after them).
    pub(crate) fn object_literal_constraint_contextual_type(
        &mut self,
        node: P<Node>,
    ) -> Option<P<Type>> {
        let levels = self.inference_replay_levels(node);
        if levels.is_empty() {
            return None;
        }
        // Each enclosing generic call was inferred with the literal's influence, so
        // replay them from the outermost inwards. The pushed argument type of an
        // outer call is what the next call sees as its contextual type.
        let mut pushed = 0;
        let result = recover(AssertUnwindSafe(|| {
            for level in levels.iter().rev() {
                let argument_type = self.argument_type_without_own_inferences(level)?;
                self.checker.push_contextual_type(
                    level.arguments[level.index],
                    Some(argument_type),
                    false, /*isCache*/
                );
                pushed += 1;
            }
            self.get_contextual_type(node, ContextFlags::None)
        }));
        for _ in 0..pushed {
            self.checker.pop_contextual_type();
        }
        match result {
            Ok(t) => t,
            Err(panic) => resume_unwind(panic),
        }
    }
}

// go: internal/typeparser/object_literal_contextual_type.go:71 inferenceReplayLevel
// inferenceReplayLevel is a generic call whose argument at index contains the
// literal, with the checked types of its receiver and other arguments. A call
// with explicit type arguments has a fixed parameter type instead.
pub(crate) struct InferenceReplayLevel {
    call: Option<P<Node>>,
    target: Option<P<Signature>>,
    arguments: Vec<P<Node>>,
    index: usize,
    this_type: Option<P<Type>>,
    argument_types: Vec<Option<P<Type>>>,
    fixed: Option<P<Type>>,
}

impl TypeParser<'_> {
    // go: internal/typeparser/object_literal_contextual_type.go:86 TypeParser.inferenceReplayLevels
    // inferenceReplayLevels returns the generic calls enclosing node, innermost
    // first, stopping at the first enclosing call that is not generic or has
    // explicit type arguments. It returns nil when the nearest call is not generic.
    // The argument types are read before any contextual type is pushed, because the
    // checker caches them.
    pub(crate) fn inference_replay_levels(&mut self, node: P<Node>) -> Vec<InferenceReplayLevel> {
        let mut levels = Vec::new();
        let mut current = node;
        loop {
            let Some((call, arguments, index)) = self.containing_call_argument(current) else {
                return levels;
            };
            // Go: GetResolvedSignature never returns nil.
            let signature = self.checker.get_resolved_signature_exported(call);
            let Some(target) = signature.target() else {
                return levels;
            };
            if target.type_parameters().is_empty() {
                return levels;
            }
            if !call.type_arguments().is_empty() {
                let fixed = self.checker.get_type_at_position_exported(signature, index);
                levels.push(InferenceReplayLevel {
                    call: None,
                    target: None,
                    arguments,
                    index,
                    this_type: None,
                    argument_types: Vec::new(),
                    fixed: Some(fixed),
                });
                return levels;
            }
            let mut this_type = None;
            if self.checker.get_this_type_of_signature(target).is_some() {
                let this_argument = self.checker.get_this_argument_of_call(call);
                this_type = Some(self.checker.get_this_argument_type(this_argument));
            }
            let mut argument_types = vec![None; arguments.len()];
            for (i, &other) in arguments.iter().enumerate() {
                if i != index && other.kind() != Kind::OmittedExpression {
                    argument_types[i] = Some(self.checker.check_expression_cached(other));
                }
            }
            levels.push(InferenceReplayLevel {
                call: Some(call),
                target: Some(target),
                arguments,
                index,
                this_type,
                argument_types,
                fixed: None,
            });
            current = call;
        }
    }

    // go: internal/typeparser/object_literal_contextual_type.go:120 TypeParser.argumentTypeWithoutOwnInferences
    // argumentTypeWithoutOwnInferences replays the checker's inferTypeArguments for
    // the level's call, skipping the argument that contains the literal, and
    // returns that argument's parameter type under the replayed inferences. The
    // mapper resolves only the type parameters the parameter type needs.
    pub(crate) fn argument_type_without_own_inferences(
        &mut self,
        level: &InferenceReplayLevel,
    ) -> Option<P<Type>> {
        if level.fixed.is_some() {
            return level.fixed;
        }
        let target = level.target.expect("a generic level has a target signature");
        let call = level.call.expect("a generic level has a call");
        let type_parameters = target.type_parameters();
        let context = Checker::new_inference_context(
            type_parameters,
            Some(target),
            InferenceFlags::None,
            None,
        );
        let inferences = context.inferences.get();
        let skip_binding_patterns = !type_parameters.iter().any(|&type_parameter| {
            self.checker.get_default_from_type_parameter_exported(type_parameter).is_none()
        });
        let mut context_flags = ContextFlags::None;
        if skip_binding_patterns {
            context_flags = ContextFlags::SkipBindingPatterns;
        }
        if let Some(contextual_type) = self.get_contextual_type(call, context_flags) {
            let is_from_binding_pattern = !skip_binding_patterns
                && self.get_contextual_type(call, ContextFlags::SkipBindingPatterns)
                    != Some(contextual_type);
            if !is_from_binding_pattern {
                let mut source = contextual_type;
                if let Some(contextual_signature) =
                    self.checker.get_single_call_signature(contextual_type)
                    && !contextual_signature.type_parameters().is_empty()
                {
                    let instantiation =
                        self.checker.get_signature_instantiation_without_filling_in_type_arguments(
                            contextual_signature,
                            contextual_signature.type_parameters(),
                        );
                    source = self.checker.get_or_create_type_from_signature(instantiation);
                }
                let return_type = self.checker.get_return_type_of_signature_exported(target);
                self.checker.infer_types(
                    inferences,
                    source,
                    return_type,
                    InferencePriority::RETURN_TYPE,
                    false, /*contravariant*/
                );
            }
        }
        if let Some(this_type) = level.this_type {
            // Go: the level has a this type only when the target signature has one.
            let target_this_type = self
                .checker
                .get_this_type_of_signature(target)
                .expect("a level this type implies a target this type");
            self.checker.infer_types(
                inferences,
                this_type,
                target_this_type,
                InferencePriority::NONE,
                false, /*contravariant*/
            );
        }
        for (i, &argument_type) in level.argument_types.iter().enumerate() {
            let Some(argument_type) = argument_type else {
                continue;
            };
            let parameter_type = self.checker.get_type_at_position_exported(target, i);
            self.checker.infer_types(
                inferences,
                argument_type,
                parameter_type,
                InferencePriority::NONE,
                false, /*contravariant*/
            );
        }
        let parameter_type = self.checker.get_type_at_position_exported(target, level.index);
        let mapper = Checker::get_mapper_from_context(Some(context));
        Some(self.checker.instantiate_type(parameter_type, mapper))
    }

    // go: internal/typeparser/object_literal_contextual_type.go:173 TypeParser.CachedObjectLiteralBareConstraintContextualType
    // CachedObjectLiteralBareConstraintContextualType returns the contextual type of
    // an object literal inside a generic call argument, as if each constrained type
    // parameter of the call's resolved signature were its constraint. Constraints
    // may mention other type parameters, so they are instantiated with the resolved
    // inferences. An unconstrained type parameter may have been inferred from the
    // literal, so it becomes unknown. It returns nil when the literal is not inside
    // a call argument or no type parameter is constrained. Like
    // CachedObjectLiteralConstraintContextualType, it never resolves the call
    // again.
    pub fn cached_object_literal_bare_constraint_contextual_type(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> Option<P<Type>> {
        let node = node.into()?;
        if node.kind() != Kind::ObjectLiteralExpression {
            return None;
        }
        cached(
            self,
            |l| &mut l.object_literal_bare_constraint_contextual_type,
            node,
            |tp| tp.object_literal_bare_constraint_contextual_type(node),
        )
    }

    // go: internal/typeparser/object_literal_contextual_type.go:182 TypeParser.objectLiteralBareConstraintContextualType
    // Rust: Go's deferred pop runs on every exit, a panic included (the panic is re-raised after it).
    pub(crate) fn object_literal_bare_constraint_contextual_type(
        &mut self,
        node: P<Node>,
    ) -> Option<P<Type>> {
        let (call, arguments, index) = self.containing_call_argument(node)?;
        // Go: GetResolvedSignature never returns nil.
        let signature = self.checker.get_resolved_signature_exported(call);
        let target = signature.target()?;
        let type_parameters = target.type_parameters();
        let type_arguments =
            get_type_arguments_for_resolved_signature(self.checker, Some(signature))
                .unwrap_or_default();
        if type_parameters.is_empty() || type_arguments.len() != type_parameters.len() {
            return None;
        }
        let inferred = new_type_mapper(type_parameters, alloc_slice(&type_arguments));
        let mut constrained = Vec::with_capacity(type_parameters.len());
        let mut has_constraint = false;
        for &type_parameter in type_parameters {
            if let Some(constraint) =
                self.checker.get_constraint_of_type_parameter_exported(type_parameter)
            {
                constrained.push(self.checker.instantiate_type(constraint, Some(inferred)));
                has_constraint = true;
            } else {
                constrained.push(self.checker.get_unknown_type());
            }
        }
        if !has_constraint {
            return None;
        }
        let parameter_type = self.checker.get_type_at_position_exported(target, index);
        let argument_type = self.checker.instantiate_type(
            parameter_type,
            Some(new_type_mapper(type_parameters, alloc_slice(&constrained))),
        );
        self.checker.push_contextual_type(
            arguments[index],
            Some(argument_type),
            false, /*isCache*/
        );
        let result =
            recover(AssertUnwindSafe(|| self.get_contextual_type(node, ContextFlags::None)));
        self.checker.pop_contextual_type();
        match result {
            Ok(t) => t,
            Err(panic) => resume_unwind(panic),
        }
    }

    // go: internal/typeparser/object_literal_contextual_type.go:226 TypeParser.containingCallArgument
    // containingCallArgument returns the nearest enclosing call whose arguments
    // contain node, its effective arguments, and the index of the argument that
    // contains node. It returns a nil call when there is none, or when a spread
    // argument comes before or at that argument.
    //
    // Rust: `None` stands for Go's `(nil, nil, -1)`.
    pub(crate) fn containing_call_argument(
        &mut self,
        node: P<Node>,
    ) -> Option<(P<Node>, Vec<P<Node>>, usize)> {
        let call = containing_call(node)?;
        let arguments = self.checker.get_effective_call_arguments(call);
        let argument = ets_shim::ast::find_ancestor(node, |n| n == call || arguments.contains(&n));
        let index = arguments.iter().position(|&a| Some(a) == argument)?;
        if arguments[..=index].iter().any(|&a| ets_shim::ast::is_spread_element(a)) {
            return None;
        }
        Some((call, arguments, index))
    }
}

// go: internal/typeparser/object_literal_contextual_type.go:242 containingCall
// containingCall returns the nearest enclosing call, new expression, or tagged
// template whose arguments contain node.
pub(crate) fn containing_call(node: P<Node>) -> Option<P<Node>> {
    let mut current = node;
    while let Some(parent) = current.parent() {
        if ets_shim::ast::is_call_like_expression(parent) {
            if ets_shim::ast::is_call_or_new_expression(parent)
                && Some(current) != parent.expression()
                || ets_shim::ast::is_tagged_template_expression(parent)
                    && current != parent.as_tagged_template_expression().tag()
            {
                return Some(parent);
            }
            return None;
        }
        current = parent;
    }
    None
}
