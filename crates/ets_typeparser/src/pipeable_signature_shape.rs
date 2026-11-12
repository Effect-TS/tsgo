use std::cell::Cell;
use std::rc::Rc;

use ets_shim::ast::{Kind, Node, Symbol, SymbolFlags};
use ets_shim::checker::{Checker, ObjectFlags, Signature, SignatureKind, Type, TypeFlags};
use ets_shim::core::P;
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};

use crate::*;

// go: internal/typeparser/pipeable_signature_shape.go:12 pipeableShapeMaxDepth 88979040545db693
pub(crate) const PIPEABLE_SHAPE_MAX_DEPTH: i64 = 64;
pub(crate) const PIPEABLE_SHAPE_MAX_WORK: i64 = 4096;
pub(crate) const PIPEABLE_SHAPE_MAX_UNION_SEARCH: i64 = 128;

// go: internal/typeparser/pipeable_signature_shape.go:18 pipeableSignatureShapeCacheKey 565d03bf86053072
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PipeableSignatureShapeCacheKey {
    data_first: P<Signature>,
    candidate: P<Signature>,
    subject_index: i64,
    self_dual: bool,
}

// go: internal/typeparser/pipeable_signature_shape.go:25 pipeableTypePair dde1dfa590ca7595
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PipeableTypePair {
    left: P<Type>,
    right: P<Type>,
}

// go: internal/typeparser/pipeable_signature_shape.go:30 rawSignature 51a04715268b67a4
pub(crate) fn raw_signature(signature: Option<P<Signature>>) -> Option<P<Signature>> {
    let mut signature = signature;
    while let Some(s) = signature
        && let Some(target) = s.target()
    {
        signature = Some(target);
    }
    signature
}

// go: internal/typeparser/pipeable_signature_shape.go:37 pipeableShapeMatch 333bcf459b326f37
// go: internal/typeparser/pipeable_signature_shape.go:39 pipeableShapeMismatch ac67d1f957313660
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PipeableShapeMatch {
    Mismatch,
    Exact,
    SelfDualUnconfirmed,
}

// go: internal/typeparser/pipeable_signature_shape.go:45 pipeableSignatureShape 48e1977e9f25d9e3
pub(crate) fn pipeable_signature_shape(
    tp: &mut TypeParser<'_>,
    data_first: Option<P<Signature>>,
    candidate: Option<P<Signature>>,
    subject_index: i64,
) -> PipeableShapeMatch {
    if pipeable_signature_shapes_match(tp, data_first, candidate, subject_index) {
        return PipeableShapeMatch::Exact;
    }
    if cached_pipeable_signature_comparison(tp, data_first, candidate, subject_index, true) {
        return PipeableShapeMatch::SelfDualUnconfirmed;
    }
    PipeableShapeMatch::Mismatch
}

// go: internal/typeparser/pipeable_signature_shape.go:55 pipeableSignatureShapesMatch 7cdb14f77a895176
pub(crate) fn pipeable_signature_shapes_match(
    tp: &mut TypeParser<'_>,
    data_first: Option<P<Signature>>,
    candidate: Option<P<Signature>>,
    subject_index: i64,
) -> bool {
    cached_pipeable_signature_comparison(tp, data_first, candidate, subject_index, false)
}

// go: internal/typeparser/pipeable_signature_shape.go:59 cachedPipeableSignatureComparison 464ff1b0ace31139
// Rust: Go reads the links from the checker (`c.EffectLinks`), which are the parser's links.
pub(crate) fn cached_pipeable_signature_comparison(
    tp: &mut TypeParser<'_>,
    data_first: Option<P<Signature>>,
    candidate: Option<P<Signature>>,
    subject_index: i64,
    self_dual: bool,
) -> bool {
    let data_first = raw_signature(data_first);
    let candidate = raw_signature(candidate);
    let (Some(data_first), Some(candidate)) = (data_first, candidate) else {
        return false;
    };
    let key = PipeableSignatureShapeCacheKey { data_first, candidate, subject_index, self_dual };
    cached(
        tp,
        |l| &mut l.pipeable_signature_shape,
        key,
        |tp| {
            compare_pipeable_signature_types_with(
                tp,
                Some(data_first),
                Some(candidate),
                subject_index,
                self_dual,
            )
        },
    )
}

// go: internal/typeparser/pipeable_signature_shape.go:79 signatureTypeMatcher f15e25b6413e0e20
// signatureTypeMatcher zips existing, uninstantiated compiler type graphs. It
// never asks the checker to relate two signatures and never creates a mapper.
// Generic binders are paired by structural occurrence rather than declaration
// order, with maps in both directions to preserve binder identity.
//
// Rust: Go's `tp` field is passed to each method (the parser borrows the checker mutably); the two counters are
// shared by every clone, as Go's `*int` fields are.
pub(crate) struct SignatureTypeMatcher {
    left_to_right: FxHashMap<P<Type>, P<Type>>,
    right_to_left: FxHashMap<P<Type>, P<Type>>,
    left_binders: FxHashSet<P<Type>>,
    right_binders: FxHashSet<P<Type>>,
    active: FxHashSet<PipeableTypePair>,
    work: Rc<Cell<i64>>,
    union_search: Rc<Cell<i64>>,
    self_dual: bool,

    extra_binder: Option<P<Type>>,
    extra_binder_target: Option<P<Type>>,
    extra_binder_matches_any: bool,
}

// go: internal/typeparser/pipeable_signature_shape.go:95 comparePipeableSignatureTypes a6347c8b1d3986d0
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "Go calls it from the data-first signature tests only")
)]
pub(crate) fn compare_pipeable_signature_types(
    tp: &mut TypeParser<'_>,
    data_first: Option<P<Signature>>,
    candidate: Option<P<Signature>>,
    subject_index: i64,
) -> bool {
    compare_pipeable_signature_types_with(tp, data_first, candidate, subject_index, false)
}

// go: internal/typeparser/pipeable_signature_shape.go:99 comparePipeableSignatureTypesWith b1b3ec15a7716182
pub(crate) fn compare_pipeable_signature_types_with(
    tp: &mut TypeParser<'_>,
    data_first: Option<P<Signature>>,
    candidate: Option<P<Signature>>,
    subject_index: i64,
    self_dual: bool,
) -> bool {
    let (Some(data_first), Some(candidate)) = (data_first, candidate) else {
        return false;
    };
    let left_parameters = data_first.parameters();
    if subject_index < 0 || subject_index >= left_parameters.len() as i64 {
        return false;
    }
    let subject_index = subject_index as usize;
    if self_dual && !is_likely_self_parameter(Some(left_parameters[subject_index])) {
        return false;
    }
    let right_parameters = candidate.parameters();
    if right_parameters.len() != left_parameters.len() - 1 {
        return false;
    }

    let candidate_return = tp.checker.get_return_type_of_signature_exported(candidate);
    let returned_signatures =
        tp.checker.get_signatures_of_type_exported(candidate_return, SignatureKind::Call);
    if returned_signatures.len() != 1 || returned_signatures[0].target().is_some() {
        // A target here means that discovering the callable shape instantiated a
        // generic alias. Dropping its mapper would compare the wrong raw graph.
        return false;
    }
    let returned = returned_signatures[0];
    if returned.parameters().len() != 1 || returned.has_rest_parameter() {
        return false;
    }
    if self_dual && !is_likely_self_parameter(Some(returned.parameters()[0])) {
        return false;
    }

    let mut m = SignatureTypeMatcher {
        left_to_right: FxHashMap::default(),
        right_to_left: FxHashMap::default(),
        left_binders: FxHashSet::default(),
        right_binders: FxHashSet::default(),
        active: FxHashSet::default(),
        work: Rc::new(Cell::new(0)),
        union_search: Rc::new(Cell::new(0)),
        self_dual,
        extra_binder: None,
        extra_binder_target: None,
        extra_binder_matches_any: false,
    };
    m.register_binders(data_first.type_parameters(), true);
    m.register_binders(candidate.type_parameters(), false);
    m.register_binders(returned.type_parameters(), false);
    if self_dual {
        if m.right_binders.len() != m.left_binders.len() + 1 {
            return false;
        }
    } else if m.left_binders.len() != m.right_binders.len() {
        return false;
    }

    let mut right_index = 0;
    for (left_index, &left_parameter) in left_parameters.iter().enumerate() {
        if left_index == subject_index {
            continue;
        }
        if !m.compare_parameter(
            tp,
            Some(left_parameter),
            left_index == left_parameters.len() - 1 && data_first.has_rest_parameter(),
            Some(right_parameters[right_index]),
            right_index == right_parameters.len() - 1 && candidate.has_rest_parameter(),
            0,
        ) {
            return false;
        }
        right_index += 1;
    }
    if !m.compare_subject_parameter(
        tp,
        Some(left_parameters[subject_index]),
        Some(returned.parameters()[0]),
    ) {
        return false;
    }
    let left_return = tp.checker.get_return_type_of_signature_exported(data_first);
    let right_return = tp.checker.get_return_type_of_signature_exported(returned);
    if !m.compare_type(tp, Some(left_return), Some(right_return), 0) {
        return false;
    }
    m.validate_binders(tp)
}

impl SignatureTypeMatcher {
    // go: internal/typeparser/pipeable_signature_shape.go:182 signatureTypeMatcher.registerBinders c8c70191e2866b64
    pub(crate) fn register_binders(&mut self, types: &[P<Type>], left: bool) {
        for &t in types {
            if !t.flags().intersects(TypeFlags::TypeParameter) {
                continue;
            }
            if left {
                self.left_binders.insert(t);
            } else {
                self.right_binders.insert(t);
            }
        }
    }

    // go: internal/typeparser/pipeable_signature_shape.go:195 signatureTypeMatcher.compareParameter 23dc484ee23042a5
    pub(crate) fn compare_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        left: Option<P<Symbol>>,
        left_rest: bool,
        right: Option<P<Symbol>>,
        right_rest: bool,
        depth: i64,
    ) -> bool {
        let (Some(left), Some(right)) = (left, right) else {
            return false;
        };
        if left_rest != right_rest
            || symbol_is_optional(Some(left)) != symbol_is_optional(Some(right))
        {
            return false;
        }
        let left_type = tp.checker.get_type_of_symbol_exported(left);
        let right_type = tp.checker.get_type_of_symbol_exported(right);
        self.compare_type(tp, Some(left_type), Some(right_type), depth + 1)
    }

    // go: internal/typeparser/pipeable_signature_shape.go:202 signatureTypeMatcher.compareSubjectParameter 216d9cbbcfeb2aed
    // Rust: Go evaluates `selfTypeIgnoringGuard(...)` before the left type (an argument of `compareType`), so the
    // right type is read first.
    pub(crate) fn compare_subject_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        left: Option<P<Symbol>>,
        right: Option<P<Symbol>>,
    ) -> bool {
        let (true, Some(left), Some(right)) = (self.self_dual, left, right) else {
            return self.compare_parameter(tp, left, false, right, false, 0);
        };
        if symbol_is_optional(Some(left)) != symbol_is_optional(Some(right)) {
            return self.compare_parameter(tp, Some(left), false, Some(right), false, 0);
        }
        let right_symbol_type = tp.checker.get_type_of_symbol_exported(right);
        let right_type = self_type_ignoring_guard(tp.checker, Some(right_symbol_type));
        let left_type = tp.checker.get_type_of_symbol_exported(left);
        self.compare_type(tp, Some(left_type), right_type, 1)
    }
}

// go: internal/typeparser/pipeable_signature_shape.go:210 selfTypeIgnoringGuard e4cc521728bca72c
pub(crate) fn self_type_ignoring_guard(
    c: &mut Checker,
    self_type: Option<P<Type>>,
) -> Option<P<Type>> {
    if let Some(t) = self_type
        && t.flags().intersects(TypeFlags::Conditional)
        && t.alias().is_none()
    {
        return Some(c.get_false_type_of_conditional_type(t));
    }
    self_type
}

// go: internal/typeparser/pipeable_signature_shape.go:217 symbolIsOptional 43bfa655f3c6ce02
pub(crate) fn symbol_is_optional(symbol: Option<P<Symbol>>) -> bool {
    symbol.is_some_and(|symbol| symbol.flags().intersects(SymbolFlags::Optional))
}

impl SignatureTypeMatcher {
    // go: internal/typeparser/pipeable_signature_shape.go:221 signatureTypeMatcher.compareType 7c20ece5d808328e
    pub(crate) fn compare_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        left: Option<P<Type>>,
        right: Option<P<Type>>,
        depth: i64,
    ) -> bool {
        let left = get_non_distributed_type_parameter(left);
        let right = get_non_distributed_type_parameter(right);
        if left == right {
            return left.is_some();
        }
        let (Some(left), Some(right)) = (left, right) else {
            return false;
        };
        if depth > PIPEABLE_SHAPE_MAX_DEPTH {
            return false;
        }
        self.work.set(self.work.get() + 1);
        if self.work.get() > PIPEABLE_SHAPE_MAX_WORK {
            return false;
        }

        if let Some(base) = no_infer_base_type(tp, Some(left)) {
            return self.compare_type(tp, Some(base), Some(right), depth + 1);
        }
        if let Some(base) = no_infer_base_type(tp, Some(right)) {
            return self.compare_type(tp, Some(left), Some(base), depth + 1);
        }
        if let Some(matched) = self.compare_extra_binder(left, right) {
            return matched;
        }

        let pair = PipeableTypePair { left, right };
        if self.active.contains(&pair) {
            return true;
        }
        self.active.insert(pair);
        // Go: `defer delete(m.active, pair)`.
        let result = self.compare_type_active(tp, left, right, depth);
        self.active.remove(&pair);
        result
    }

    /// Rust-only: the part of Go's `compareType` that runs while `pair` is active (its deferred delete runs after
    /// every return below).
    fn compare_type_active(
        &mut self,
        tp: &mut TypeParser<'_>,
        left: P<Type>,
        right: P<Type>,
        depth: i64,
    ) -> bool {
        let (left_alias, right_alias) = (left.alias(), right.alias());
        if left_alias.is_some() || right_alias.is_some() {
            return match (left_alias, right_alias) {
                (Some(left_alias), Some(right_alias)) => {
                    same_symbol_reference(tp.checker, left_alias.symbol(), right_alias.symbol())
                        && self.compare_type_list(
                            tp,
                            left_alias.type_arguments(),
                            right_alias.type_arguments(),
                            depth + 1,
                        )
                }
                _ => false,
            };
        }

        let left_is_binder = left.flags().intersects(TypeFlags::TypeParameter);
        let right_is_binder = right.flags().intersects(TypeFlags::TypeParameter);
        if left_is_binder || right_is_binder {
            return left_is_binder
                && right_is_binder
                && self.compare_type_parameter(tp, left, right);
        }
        if left.flags() != right.flags() {
            return false;
        }

        let flags = left.flags();
        if flags.intersects(TypeFlags::Union) {
            self.compare_union(tp, left.types(), right.types(), depth + 1)
        } else if flags.intersects(TypeFlags::Intersection) {
            // Preserve compiler order for intersections; unlike unions, their order
            // can affect overloaded callable behavior.
            self.compare_type_list(tp, left.types(), right.types(), depth + 1)
        } else if flags.intersects(TypeFlags::Conditional) {
            let (left_conditional, right_conditional) =
                (left.as_conditional_type(), right.as_conditional_type());
            self.compare_type(
                tp,
                left_conditional.check_type.get(),
                right_conditional.check_type.get(),
                depth + 1,
            ) && self.compare_type(
                tp,
                left_conditional.extends_type.get(),
                right_conditional.extends_type.get(),
                depth + 1,
            ) && {
                let left_true = tp.checker.get_true_type_of_conditional_type(left);
                let right_true = tp.checker.get_true_type_of_conditional_type(right);
                self.compare_type(tp, Some(left_true), Some(right_true), depth + 1)
            } && {
                let left_false = tp.checker.get_false_type_of_conditional_type(left);
                let right_false = tp.checker.get_false_type_of_conditional_type(right);
                self.compare_type(tp, Some(left_false), Some(right_false), depth + 1)
            }
        } else if flags.intersects(TypeFlags::IndexedAccess) {
            let (left_indexed, right_indexed) =
                (left.as_indexed_access_type(), right.as_indexed_access_type());
            self.compare_type(
                tp,
                left_indexed.object_type.get(),
                right_indexed.object_type.get(),
                depth + 1,
            ) && self.compare_type(
                tp,
                left_indexed.index_type.get(),
                right_indexed.index_type.get(),
                depth + 1,
            )
        } else if flags.intersects(TypeFlags::Index) {
            self.compare_type(
                tp,
                left.as_index_type().target.get(),
                right.as_index_type().target.get(),
                depth + 1,
            )
        } else if flags.intersects(TypeFlags::TemplateLiteral) {
            left.as_template_literal_type().texts.get()
                == right.as_template_literal_type().texts.get()
                && self.compare_type_list(
                    tp,
                    left.as_template_literal_type().types.get(),
                    right.as_template_literal_type().types.get(),
                    depth + 1,
                )
        } else if flags.intersects(TypeFlags::StringMapping) {
            same_symbol_reference(tp.checker, left.symbol(), right.symbol())
                && self.compare_type(
                    tp,
                    left.as_string_mapping_type().target.get(),
                    right.as_string_mapping_type().target.get(),
                    depth + 1,
                )
        } else if flags.intersects(TypeFlags::Substitution) {
            let (left_substitution, right_substitution) =
                (left.as_substitution_type(), right.as_substitution_type());
            self.compare_type(
                tp,
                left_substitution.base_type.get(),
                right_substitution.base_type.get(),
                depth + 1,
            ) && self.compare_type(
                tp,
                left_substitution.constraint.get(),
                right_substitution.constraint.get(),
                depth + 1,
            )
        } else if flags.intersects(TypeFlags::Object) {
            self.compare_object(tp, left, right, depth + 1)
        } else if flags.intersects(TypeFlags::Literal) {
            // Go: reflect.DeepEqual of the two literal values.
            left.as_literal_type().value() == right.as_literal_type().value()
                && same_optional_symbol(tp, left.symbol(), right.symbol())
        } else if flags.intersects(TypeFlags::UniqueESSymbol) || flags.intersects(TypeFlags::Enum) {
            same_symbol_reference(tp.checker, left.symbol(), right.symbol())
        } else {
            flags.intersects(TypeFlags::Singleton)
        }
    }

    // go: internal/typeparser/pipeable_signature_shape.go:312 signatureTypeMatcher.compareExtraBinder 55484082ba95c0f5
    // Rust: Go's `(matched, handled)` is `Option<matched>`, `None` when not handled.
    pub(crate) fn compare_extra_binder(&mut self, left: P<Type>, right: P<Type>) -> Option<bool> {
        if !self.self_dual
            || !right.flags().intersects(TypeFlags::TypeParameter)
            || left.flags().intersects(TypeFlags::TypeParameter)
        {
            return None;
        }
        if !self.right_binders.contains(&right) {
            return None;
        }
        if self.right_to_left.contains_key(&right) {
            return None;
        }
        if self.extra_binder_matches_any {
            return Some(Some(right) == self.extra_binder);
        }
        if self.extra_binder.is_some() {
            return Some(
                Some(right) == self.extra_binder && Some(left) == self.extra_binder_target,
            );
        }
        (self.extra_binder, self.extra_binder_target) = (Some(right), Some(left));
        Some(true)
    }

    // go: internal/typeparser/pipeable_signature_shape.go:332 signatureTypeMatcher.compareTypeParameter 8f2df0c18cec81a8
    pub(crate) fn compare_type_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        left: P<Type>,
        right: P<Type>,
    ) -> bool {
        if Some(right) == self.extra_binder {
            return false;
        }
        let left_local = self.left_binders.contains(&left);
        let right_local = self.right_binders.contains(&right);
        if left_local || right_local {
            if !left_local || !right_local {
                return false;
            }
            if let Some(&mapped) = self.left_to_right.get(&left) {
                return mapped == right;
            }
            if let Some(&mapped) = self.right_to_left.get(&right) {
                return mapped == left;
            }
            self.left_to_right.insert(left, right);
            self.right_to_left.insert(right, left);
            return true;
        }
        left == right || same_symbol_reference(tp.checker, left.symbol(), right.symbol())
    }

    // go: internal/typeparser/pipeable_signature_shape.go:355 signatureTypeMatcher.compareObject 03324aa11b07c530
    pub(crate) fn compare_object(
        &mut self,
        tp: &mut TypeParser<'_>,
        left: P<Type>,
        right: P<Type>,
        depth: i64,
    ) -> bool {
        let (left_flags, right_flags) = (left.object_flags(), right.object_flags());
        let left_mapped = left_flags.intersects(ObjectFlags::Mapped);
        let right_mapped = right_flags.intersects(ObjectFlags::Mapped);
        if left_mapped || right_mapped {
            return left_mapped
                && right_mapped
                && self.compare_mapped_type(tp, left, right, depth + 1);
        }
        if left_flags.intersects(ObjectFlags::ReverseMapped)
            || right_flags.intersects(ObjectFlags::ReverseMapped)
            || left_flags.intersects(ObjectFlags::EvolvingArray)
            || right_flags.intersects(ObjectFlags::EvolvingArray)
            || left_flags.intersects(ObjectFlags::InstantiationExpressionType)
            || right_flags.intersects(ObjectFlags::InstantiationExpressionType)
        {
            return false;
        }

        let left_reference = left_flags.intersects(ObjectFlags::Reference);
        let right_reference = right_flags.intersects(ObjectFlags::Reference);
        if left_reference || right_reference {
            if !left_reference || !right_reference {
                return false;
            }
            let (left_target, right_target) = (left.target(), right.target());
            if !(left_target == right_target
                || same_symbol_reference(
                    tp.checker,
                    left_target.and_then(|t| t.symbol()),
                    right_target.and_then(|t| t.symbol()),
                ))
            {
                return false;
            }
            let left_arguments = tp.checker.get_type_arguments(left);
            let right_arguments = tp.checker.get_type_arguments(right);
            return self.compare_type_list(tp, left_arguments, right_arguments, depth + 1);
        }

        let left_anonymous = left_flags.intersects(ObjectFlags::Anonymous);
        let right_anonymous = right_flags.intersects(ObjectFlags::Anonymous);
        if !left_anonymous || !right_anonymous {
            return !left_anonymous
                && !right_anonymous
                && same_symbol_reference(tp.checker, left.symbol(), right.symbol());
        }
        self.compare_anonymous_object(tp, left, right, depth + 1)
    }

    // go: internal/typeparser/pipeable_signature_shape.go:387 signatureTypeMatcher.compareMappedType bf32692ed1114380
    pub(crate) fn compare_mapped_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        left: P<Type>,
        right: P<Type>,
        depth: i64,
    ) -> bool {
        let left_binder = get_type_parameter_from_mapped_type(tp.checker, left);
        let right_binder = get_type_parameter_from_mapped_type(tp.checker, right);
        if ets_shim::checker::get_mapped_type_modifiers(left)
            != ets_shim::checker::get_mapped_type_modifiers(right)
        {
            return false;
        }
        self.register_binders(&[left_binder], true);
        self.register_binders(&[right_binder], false);
        self.compare_type(tp, Some(left_binder), Some(right_binder), depth + 1)
            && {
                let left_constraint = get_constraint_type_from_mapped_type(tp.checker, left);
                let right_constraint = get_constraint_type_from_mapped_type(tp.checker, right);
                self.compare_type(tp, Some(left_constraint), Some(right_constraint), depth + 1)
            }
            && {
                let left_name = get_name_type_from_mapped_type(tp.checker, left);
                let right_name = get_name_type_from_mapped_type(tp.checker, right);
                self.compare_optional_type_at(tp, left_name, right_name, depth + 1)
            }
            && {
                let left_template = get_template_type_from_mapped_type(tp.checker, left);
                let right_template = get_template_type_from_mapped_type(tp.checker, right);
                self.compare_type(tp, Some(left_template), Some(right_template), depth + 1)
            }
    }

    // go: internal/typeparser/pipeable_signature_shape.go:413 signatureTypeMatcher.compareAnonymousObject 2af1a88d532311a1
    pub(crate) fn compare_anonymous_object(
        &mut self,
        tp: &mut TypeParser<'_>,
        left: P<Type>,
        right: P<Type>,
        depth: i64,
    ) -> bool {
        let left_properties = tp.checker.get_properties_of_type_exported(left);
        let right_properties = tp.checker.get_properties_of_type_exported(right);
        if left_properties.len() != right_properties.len() {
            return false;
        }
        let mut right_by_name: FxHashMap<&str, P<Symbol>> =
            FxHashMap::with_capacity_and_hasher(right_properties.len(), FxBuildHasher);
        for &property in right_properties {
            if right_by_name.contains_key(property.name()) {
                return false;
            }
            right_by_name.insert(property.name(), property);
        }
        for &left_property in left_properties {
            let Some(&right_property) = right_by_name.get(left_property.name()) else {
                return false;
            };
            if symbol_is_optional(Some(left_property)) != symbol_is_optional(Some(right_property))
                || tp.checker.is_readonly_symbol(left_property)
                    != tp.checker.is_readonly_symbol(right_property)
            {
                return false;
            }
            let left_type = tp.checker.get_type_of_symbol_exported(left_property);
            let right_type = tp.checker.get_type_of_symbol_exported(right_property);
            if !self.compare_type(tp, Some(left_type), Some(right_type), depth + 1) {
                return false;
            }
        }

        let left_indexes = tp.checker.get_index_infos_of_type_exported(left);
        let right_indexes = tp.checker.get_index_infos_of_type_exported(right);
        if left_indexes.len() != right_indexes.len() {
            return false;
        }
        for i in 0..left_indexes.len() {
            if left_indexes[i].is_readonly.get() != right_indexes[i].is_readonly.get()
                || !self.compare_type(
                    tp,
                    left_indexes[i].key_type.get(),
                    right_indexes[i].key_type.get(),
                    depth + 1,
                )
                || !self.compare_type(
                    tp,
                    left_indexes[i].value_type.get(),
                    right_indexes[i].value_type.get(),
                    depth + 1,
                )
            {
                return false;
            }
        }

        if !tp.checker.get_signatures_of_type_exported(left, SignatureKind::Construct).is_empty()
            || !tp
                .checker
                .get_signatures_of_type_exported(right, SignatureKind::Construct)
                .is_empty()
        {
            return false;
        }
        let left_calls = tp.checker.get_signatures_of_type_exported(left, SignatureKind::Call);
        let right_calls = tp.checker.get_signatures_of_type_exported(right, SignatureKind::Call);
        if left_calls.len() != right_calls.len() {
            return false;
        }
        for i in 0..left_calls.len() {
            if left_calls[i].target().is_some()
                || right_calls[i].target().is_some()
                || !self.compare_signature(tp, Some(left_calls[i]), Some(right_calls[i]), depth + 1)
            {
                return false;
            }
        }
        true
    }

    // go: internal/typeparser/pipeable_signature_shape.go:467 signatureTypeMatcher.compareSignature ca0fe42b4aa7b47a
    pub(crate) fn compare_signature(
        &mut self,
        tp: &mut TypeParser<'_>,
        left: Option<P<Signature>>,
        right: Option<P<Signature>>,
        depth: i64,
    ) -> bool {
        let (Some(left), Some(right)) = (left, right) else {
            return false;
        };
        if left.has_rest_parameter() != right.has_rest_parameter()
            || left.min_argument_count() != right.min_argument_count()
            || (tp.checker.get_type_predicate_of_signature(left).is_some()
                || tp.checker.get_type_predicate_of_signature(right).is_some())
        {
            return false;
        }
        self.register_binders(left.type_parameters(), true);
        self.register_binders(right.type_parameters(), false);
        let (left_parameters, right_parameters) = (left.parameters(), right.parameters());
        if left_parameters.len() != right_parameters.len() {
            return false;
        }
        if left.this_parameter().is_none() != right.this_parameter().is_none() {
            return false;
        }
        if left.this_parameter().is_some()
            && !self.compare_parameter(
                tp,
                left.this_parameter(),
                false,
                right.this_parameter(),
                false,
                depth + 1,
            )
        {
            return false;
        }
        for i in 0..left_parameters.len() {
            if !self.compare_parameter(
                tp,
                Some(left_parameters[i]),
                i == left_parameters.len() - 1 && left.has_rest_parameter(),
                Some(right_parameters[i]),
                i == right_parameters.len() - 1 && right.has_rest_parameter(),
                depth + 1,
            ) {
                return false;
            }
        }
        let left_return = tp.checker.get_return_type_of_signature_exported(left);
        let right_return = tp.checker.get_return_type_of_signature_exported(right);
        self.compare_type(tp, Some(left_return), Some(right_return), depth + 1)
    }

    // go: internal/typeparser/pipeable_signature_shape.go:497 signatureTypeMatcher.compareTypeList 8c063421b7a47227
    pub(crate) fn compare_type_list(
        &mut self,
        tp: &mut TypeParser<'_>,
        left: &[P<Type>],
        right: &[P<Type>],
        depth: i64,
    ) -> bool {
        if left.len() != right.len() {
            return false;
        }
        for i in 0..left.len() {
            if !self.compare_type(tp, Some(left[i]), Some(right[i]), depth + 1) {
                return false;
            }
        }
        true
    }

    // go: internal/typeparser/pipeable_signature_shape.go:509 signatureTypeMatcher.compareUnion e7bf39568966648c
    pub(crate) fn compare_union(
        &mut self,
        tp: &mut TypeParser<'_>,
        left: &[P<Type>],
        right: &[P<Type>],
        depth: i64,
    ) -> bool {
        if left.len() != right.len() {
            return false;
        }
        self.compare_union_at(tp, left, right, &vec![false; right.len()], 0, depth + 1)
    }

    // go: internal/typeparser/pipeable_signature_shape.go:516 signatureTypeMatcher.compareUnionAt 327181aeb877706a
    pub(crate) fn compare_union_at(
        &mut self,
        tp: &mut TypeParser<'_>,
        left: &[P<Type>],
        right: &[P<Type>],
        used: &[bool],
        index: usize,
        depth: i64,
    ) -> bool {
        if index == left.len() {
            return true;
        }
        for right_index in 0..right.len() {
            if used[right_index] {
                continue;
            }
            self.union_search.set(self.union_search.get() + 1);
            if self.union_search.get() > PIPEABLE_SHAPE_MAX_UNION_SEARCH {
                return false;
            }
            let mut branch = self.clone();
            if !branch.compare_type(tp, Some(left[index]), Some(right[right_index]), depth + 1) {
                continue;
            }
            let mut branch_used = used.to_vec();
            branch_used[right_index] = true;
            if branch.compare_union_at(tp, left, right, &branch_used, index + 1, depth + 1) {
                self.commit(branch);
                return true;
            }
        }
        false
    }
}

// go: internal/typeparser/pipeable_signature_shape.go:542 signatureTypeMatcher.clone 6142736005478c4a
// Rust: Go copies the struct (its `*int` counters stay shared) and clones each map.
impl Clone for SignatureTypeMatcher {
    fn clone(&self) -> Self {
        SignatureTypeMatcher {
            left_to_right: self.left_to_right.clone(),
            right_to_left: self.right_to_left.clone(),
            left_binders: self.left_binders.clone(),
            right_binders: self.right_binders.clone(),
            active: self.active.clone(),
            work: Rc::clone(&self.work),
            union_search: Rc::clone(&self.union_search),
            self_dual: self.self_dual,
            extra_binder: self.extra_binder,
            extra_binder_target: self.extra_binder_target,
            extra_binder_matches_any: self.extra_binder_matches_any,
        }
    }
}

impl SignatureTypeMatcher {
    // go: internal/typeparser/pipeable_signature_shape.go:552 signatureTypeMatcher.commit 0de2fa7d8be0308d
    pub(crate) fn commit(&mut self, branch: SignatureTypeMatcher) {
        self.left_to_right = branch.left_to_right;
        self.right_to_left = branch.right_to_left;
        self.left_binders = branch.left_binders;
        self.right_binders = branch.right_binders;
        self.extra_binder = branch.extra_binder;
        self.extra_binder_target = branch.extra_binder_target;
    }

    // go: internal/typeparser/pipeable_signature_shape.go:561 signatureTypeMatcher.validateBinders cb9f7de94f876263
    // Rust: the binder pairs are compared through `bounds`, which owns its state (see `bounds_comparer`); `self`
    // is not read again after the pairs are listed.
    pub(crate) fn validate_binders(&self, tp: &mut TypeParser<'_>) -> bool {
        if self.left_binders.len() as i64
            != self.right_binders.len() as i64 - self.extra_binder_count()
            || self.left_to_right.len() != self.left_binders.len()
        {
            return false;
        }
        if self.self_dual && !self.has_unconstrained_extra_binder(tp) {
            return false;
        }
        let mut bounds = self.bounds_comparer();
        // Go ranges over the map; every pair must pass, so the order only changes which failing pair is found
        // first.
        let pairs: Vec<(P<Type>, P<Type>)> =
            self.left_to_right.iter().map(|(&l, &r)| (l, r)).collect();
        for (left, right) in pairs {
            if get_type_parameter_modifiers(left) != get_type_parameter_modifiers(right) {
                return false;
            }
            let left_constraint = tp.checker.get_constraint_of_type_parameter_exported(left);
            let right_constraint = tp.checker.get_constraint_of_type_parameter_exported(right);
            if !bounds.compare_optional_type(tp, left_constraint, right_constraint) {
                return false;
            }
            let mut left_default = tp.checker.get_default_from_type_parameter_exported(left);
            let mut right_default = tp.checker.get_default_from_type_parameter_exported(right);
            if self.self_dual && left_default.is_none() != right_default.is_none() {
                (left_default, right_default) = (
                    default_or_constraint(left_default, left_constraint),
                    default_or_constraint(right_default, right_constraint),
                );
            }
            if !bounds.compare_optional_type(tp, left_default, right_default) {
                return false;
            }
        }
        true
    }

    // go: internal/typeparser/pipeable_signature_shape.go:588 signatureTypeMatcher.extraBinderCount aabc71f5c341f2b9
    pub(crate) fn extra_binder_count(&self) -> i64 {
        if self.extra_binder.is_none() {
            return 0;
        }
        1
    }

    // go: internal/typeparser/pipeable_signature_shape.go:595 signatureTypeMatcher.hasUnconstrainedExtraBinder 4bb3b280dea5b2f9
    pub(crate) fn has_unconstrained_extra_binder(&self, tp: &mut TypeParser<'_>) -> bool {
        let Some(extra_binder) = self.extra_binder else {
            return false;
        };
        tp.checker.get_constraint_of_type_parameter_exported(extra_binder).is_none()
            && tp.checker.get_default_from_type_parameter_exported(extra_binder).is_none()
    }

    // go: internal/typeparser/pipeable_signature_shape.go:601 signatureTypeMatcher.boundsComparer 35078ee1d7fe27b1
    // Rust: Go's struct copy shares the binder maps with `m` until a union branch commits; `validateBinders` reads
    // `m` only for the binder pairs, listed before any comparison, so a clone (whose counters stay shared) behaves
    // the same.
    pub(crate) fn bounds_comparer(&self) -> SignatureTypeMatcher {
        let mut bounds = self.clone();
        bounds.extra_binder_matches_any = true;
        bounds
    }
}

// go: internal/typeparser/pipeable_signature_shape.go:607 defaultOrConstraint dc11c47a632ac7fb
pub(crate) fn default_or_constraint(
    default_type: Option<P<Type>>,
    constraint: Option<P<Type>>,
) -> Option<P<Type>> {
    if default_type.is_some() {
        return default_type;
    }
    constraint
}

impl SignatureTypeMatcher {
    // go: internal/typeparser/pipeable_signature_shape.go:614 signatureTypeMatcher.compareOptionalType 49b35c450d67e6f5
    pub(crate) fn compare_optional_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        left: Option<P<Type>>,
        right: Option<P<Type>>,
    ) -> bool {
        self.compare_optional_type_at(tp, left, right, 0)
    }

    // go: internal/typeparser/pipeable_signature_shape.go:618 signatureTypeMatcher.compareOptionalTypeAt 1629d1827c0d2487
    pub(crate) fn compare_optional_type_at(
        &mut self,
        tp: &mut TypeParser<'_>,
        left: Option<P<Type>>,
        right: Option<P<Type>>,
        depth: i64,
    ) -> bool {
        if left.is_none() || right.is_none() {
            return left == right;
        }
        self.compare_type(tp, left, right, depth + 1)
    }
}

// go: internal/typeparser/pipeable_signature_shape.go:625 sameOptionalSymbol 3c934e5c82765a47
// Rust: Go's checker argument is `tp.checker`.
pub(crate) fn same_optional_symbol(
    tp: &mut TypeParser<'_>,
    left: Option<P<Symbol>>,
    right: Option<P<Symbol>>,
) -> bool {
    if left.is_none() || right.is_none() {
        return left == right;
    }
    same_symbol_reference(tp.checker, left, right)
}

// go: internal/typeparser/pipeable_signature_shape.go:632 noInferBaseType 7a5798d14505ea63
pub(crate) fn no_infer_base_type(tp: &mut TypeParser<'_>, t: Option<P<Type>>) -> Option<P<Type>> {
    let t = t?;
    if is_no_infer_type(t) {
        return t.as_substitution_type().base_type.get();
    }
    let alias = t.alias()?;
    if alias.type_arguments().len() != 1 {
        return None;
    }
    let symbol = alias.symbol()?;
    for &declaration in symbol.declarations() {
        if is_no_infer_alias_declaration(tp, Some(declaration)) {
            return Some(alias.type_arguments()[0]);
        }
    }
    None
}

// go: internal/typeparser/pipeable_signature_shape.go:654 isNoInferAliasDeclaration 306c6a0200eb9781
// Effect supported NoInfer before it became a TypeScript intrinsic with the
// standard tuple/indexed-access encoding. Recognize only those two exact
// declarations so an unrelated alias named NoInfer is not made transparent.
pub(crate) fn is_no_infer_alias_declaration(
    tp: &mut TypeParser<'_>,
    declaration: Option<P<Node>>,
) -> bool {
    let Some(declaration) = declaration else {
        return false;
    };
    if declaration.kind() != Kind::TypeAliasDeclaration || declaration.type_parameters().len() != 1
    {
        return false;
    }
    let Some(body) = declaration.type_node() else {
        return false;
    };
    if body.kind() == Kind::IntrinsicKeyword {
        return true;
    }
    if body.kind() != Kind::IndexedAccessType {
        return false;
    }
    let indexed = body.as_indexed_access_type_node();
    let object_type = indexed.object_type;
    let index_type = indexed.index_type;
    if object_type.kind() != Kind::TupleType
        || object_type.as_tuple_type_node().elements().nodes().len() != 1
        || index_type.kind() != Kind::ConditionalType
    {
        return false;
    }
    let type_parameter = declaration.type_parameters()[0];
    let mut binder = type_parameter.symbol();
    if binder.is_none() {
        binder = tp.get_symbol_at_location(type_parameter.name());
    }
    let conditional = index_type.as_conditional_type_node();
    binder.is_some()
        && conditional.extends_type.kind() == Kind::AnyKeyword
        && is_numeric_literal_type_node(Some(conditional.true_type), "0")
        && conditional.false_type.kind() == Kind::NeverKeyword
        && is_type_parameter_type_node(
            tp,
            Some(object_type.as_tuple_type_node().elements().nodes()[0]),
            binder,
        )
        && is_type_parameter_type_node(tp, Some(conditional.check_type), binder)
}

// go: internal/typeparser/pipeable_signature_shape.go:687 isTypeParameterTypeNode f3c297dd656e5b8d
pub(crate) fn is_type_parameter_type_node(
    tp: &mut TypeParser<'_>,
    node: Option<P<Node>>,
    binder: Option<P<Symbol>>,
) -> bool {
    let Some(node) = node else {
        return false;
    };
    if node.kind() != Kind::TypeReference {
        return false;
    }
    let reference = node.as_type_reference_node();
    if reference.type_arguments().is_some() {
        return false;
    }
    let symbol = tp.get_symbol_at_location(reference.type_name);
    same_symbol_reference(tp.checker, symbol, binder)
}

// go: internal/typeparser/pipeable_signature_shape.go:695 isNumericLiteralTypeNode 824dc65cf8141862
pub(crate) fn is_numeric_literal_type_node(node: Option<P<Node>>, value: &str) -> bool {
    let Some(node) = node else {
        return false;
    };
    node.kind() == Kind::LiteralType
        && node.as_literal_type_node().literal.kind() == Kind::NumericLiteral
        && node.as_literal_type_node().literal.text() == value
}
