use std::sync::Arc;

use ets_readonly::Slice;
use ets_shim::ast::{Kind, Node, Symbol};
use ets_shim::checker::{Checker, ObjectFlags, Signature, SignatureKind, Type, TypeFlags};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/data_first_signature.go:11 ParsedDataFirstOrLastCall 41c03a6bd61a0cde
pub struct ParsedDataFirstOrLastCall {
    pub(crate) node: P<Node>,
    pub(crate) callee: P<Node>,
    pub(crate) subject: P<Node>,
    pub(crate) args: Vec<P<Node>>,
    pub(crate) subject_index: usize,
}

impl ParsedDataFirstOrLastCall {
    // go: internal/typeparser/data_first_signature.go:19 ParsedDataFirstOrLastCall.Node ada09a476c8f5283
    // Rust: the call expression node (Go's `*ast.CallExpression`).
    pub fn node(&self) -> P<Node> {
        self.node
    }

    // go: internal/typeparser/data_first_signature.go:21 ParsedDataFirstOrLastCall.Callee 1154e0d94f58db96
    pub fn callee(&self) -> P<Node> {
        self.callee
    }

    // go: internal/typeparser/data_first_signature.go:23 ParsedDataFirstOrLastCall.Subject 7c89cadf0e25764f
    pub fn subject(&self) -> P<Node> {
        self.subject
    }

    // go: internal/typeparser/data_first_signature.go:25 ParsedDataFirstOrLastCall.Args b5fd1698dd351504
    pub fn args(&self) -> Slice<'_, P<Node>> {
        ets_readonly::new(&self.args[..])
    }

    // go: internal/typeparser/data_first_signature.go:27 ParsedDataFirstOrLastCall.SubjectIndex 461bd6962b37f94c
    pub fn subject_index(&self) -> i64 {
        self.subject_index as i64
    }
}

// go: internal/typeparser/data_first_signature.go:29 PipeableSignatureWitness b1cd9c8b3992a63e
#[derive(Clone, Default)]
pub struct PipeableSignatureWitness {
    pub argument_types: Vec<Option<P<Type>>>,
    pub subject_type: Option<P<Type>>,
}

impl TypeParser<'_> {
    // go: internal/typeparser/data_first_signature.go:34 TypeParser.DataFirstOrLastCall c7bf5ce414f57c15
    pub fn data_first_or_last_call(
        &mut self,
        node: impl Into<Option<P<Node>>>,
    ) -> Option<Arc<ParsedDataFirstOrLastCall>> {
        let node = node.into()?;
        if node.kind() != Kind::CallExpression {
            return None;
        }
        let call = node.as_call_expression();
        let arguments = call.arguments().nodes();
        if arguments.len() < 2 {
            return None;
        }

        for &arg in arguments {
            if arg.kind() == Kind::SpreadElement {
                return None;
            }
        }

        cached_node(
            self,
            |l| &mut l.data_first_or_last_call,
            node,
            |tp| tp.parse_data_first_or_last_call(node).map(Arc::new),
        )
    }

    // go: internal/typeparser/data_first_signature.go:54 TypeParser.parseDataFirstOrLastCall 038ffd003294b97d
    // Rust: Go's `call` argument is `node.AsCallExpression()`.
    pub(crate) fn parse_data_first_or_last_call(
        &mut self,
        node: P<Node>,
    ) -> Option<ParsedDataFirstOrLastCall> {
        let call = node.as_call_expression();
        let arguments = call.arguments().nodes();
        // Go: GetResolvedSignature never returns nil.
        let resolved = self.checker.get_resolved_signature_exported(node);
        if arguments.len() < resolved.min_argument_count()
            || (arguments.len() > resolved.parameters().len() && !resolved.has_rest_parameter())
        {
            return None;
        }

        let resolved_declaration = raw_signature(Some(resolved)).and_then(|s| s.declaration());
        let mut resolved_symbol = None;
        if let Some(resolved_declaration) = resolved_declaration {
            resolved_symbol = get_symbol_of_declaration(self.checker, resolved_declaration);
        }
        if resolved_declaration.is_some() && resolved_symbol.is_none() {
            return None;
        }
        let callee_type = self.get_type_at_location(call.expression())?;
        let candidates =
            self.checker.get_signatures_of_type_exported(callee_type, SignatureKind::Call);

        let mut subject_indexes = vec![0];
        if arguments.len() == 2 {
            let last = arguments.len() - 1;
            let mut prefer_first = false;
            let params = resolved.parameters();
            if !params.is_empty() {
                prefer_first = is_likely_self_parameter(Some(params[0]));
            }
            if prefer_first {
                subject_indexes = vec![0, last];
            } else {
                subject_indexes = vec![last, 0];
            }
        }

        let mut matched: Option<ParsedDataFirstOrLastCall> = None;
        for subject_index in subject_indexes {
            let args = omit_arg_at(arguments, subject_index);
            let witness_args = args.clone();
            let witness = new_lazy(move |tp| {
                let subject_type = tp.get_type_at_location(arguments[subject_index])?;
                let mut argument_types = Vec::with_capacity(witness_args.len());
                for &arg in &witness_args {
                    argument_types.push(tp.get_type_at_location(arg));
                }
                Some(Arc::new(PipeableSignatureWitness {
                    argument_types,
                    subject_type: Some(subject_type),
                }))
            });

            for &candidate in candidates {
                if resolved_symbol.is_some() {
                    let Some(candidate_declaration) =
                        raw_signature(Some(candidate)).and_then(|s| s.declaration())
                    else {
                        continue;
                    };
                    let candidate_symbol =
                        get_symbol_of_declaration(self.checker, candidate_declaration);
                    if candidate_symbol.is_none()
                        || get_symbol_if_same_reference(
                            self.checker,
                            resolved_symbol,
                            candidate_symbol,
                        )
                        .is_none()
                    {
                        continue;
                    }
                }
                // The optional tail only applies when the call stops at the subject,
                // leaving the trailing optional parameters to the returned function.
                let mut optional_tail = false;
                if !pipeable_signature_shapes_match(
                    self,
                    Some(resolved),
                    Some(candidate),
                    subject_index as i64,
                    false,
                ) {
                    if subject_index != arguments.len() - 1
                        || !pipeable_signature_shapes_match(
                            self,
                            Some(resolved),
                            Some(candidate),
                            subject_index as i64,
                            true,
                        )
                    {
                        continue;
                    }
                    optional_tail = true;
                }
                let Some(witness_value) = Lazy::get(Some(&witness), self) else {
                    break;
                };
                if !matches_pipeable_signature(
                    self,
                    Some(resolved),
                    Some(candidate),
                    subject_index as i64,
                    Some(&witness_value),
                    optional_tail,
                ) {
                    continue;
                }

                if let Some(previous) = matched.take_if(|m| m.subject_index != subject_index) {
                    if previous.subject_index < resolved.parameters().len()
                        && is_likely_self_parameter(Some(
                            resolved.parameters()[previous.subject_index],
                        ))
                    {
                        return Some(previous);
                    }
                    return None;
                }
                matched = Some(ParsedDataFirstOrLastCall {
                    node,
                    callee: call.expression(),
                    subject: arguments[subject_index],
                    args,
                    subject_index,
                });
                break;
            }
        }

        matched
    }
}

// go: internal/typeparser/data_first_signature.go:161 MatchesPipeableSignature 15241d84eae54af6
// MatchesPipeableSignature reports whether candidate is the pipeable form of
// dataFirst with the parameter at subjectIndex moved into a returned unary function.
// When witness is nil, parameter types from dataFirst are used for comparison.
pub fn matches_pipeable_signature_exported(
    tp: &mut TypeParser<'_>,
    data_first: Option<P<Signature>>,
    candidate: Option<P<Signature>>,
    subject_index: i64,
    witness: Option<&PipeableSignatureWitness>,
) -> bool {
    matches_pipeable_signature(tp, data_first, candidate, subject_index, witness, false)
}

// go: internal/typeparser/data_first_signature.go:165 matchesPipeableSignature d607ff0f8e6d21bb
pub(crate) fn matches_pipeable_signature(
    tp: &mut TypeParser<'_>,
    data_first: Option<P<Signature>>,
    candidate: Option<P<Signature>>,
    subject_index: i64,
    witness: Option<&PipeableSignatureWitness>,
    optional_tail: bool,
) -> bool {
    let (Some(data_first), Some(candidate)) = (data_first, candidate) else {
        return false;
    };
    let params = data_first.parameters();
    if subject_index < 0 || subject_index >= params.len() as i64 {
        return false;
    }

    let argument_types;
    let subject_type;
    if let Some(witness) = witness {
        argument_types = witness.argument_types.clone();
        subject_type = witness.subject_type;
    } else {
        let mut types = Vec::with_capacity(params.len() - 1);
        let mut subject = None;
        for (i, &param) in params.iter().enumerate() {
            if i as i64 == subject_index {
                subject = Some(tp.checker.get_type_of_symbol_exported(param));
                continue;
            }
            types.push(Some(tp.checker.get_type_of_symbol_exported(param)));
        }
        argument_types = types;
        subject_type = subject;
    }

    let mut argument_types = argument_types;
    let mut tail = 0;
    if optional_tail {
        tail = optional_tail_length(raw_signature(Some(data_first)), subject_index);
        if tail == 0 {
            return false;
        }
        argument_types.truncate(subject_index as usize);
    }
    argument_types_match_parameters(tp.checker, &argument_types, Some(candidate))
        && candidate_accepts_subject(tp.checker, Some(candidate), subject_type, tail)
        && pipeable_signature_shapes_match(
            tp,
            Some(data_first),
            Some(candidate),
            subject_index,
            optional_tail,
        )
}

// go: internal/typeparser/data_first_signature.go:203 argumentTypesMatchParameters bf343d1c0a71879d
pub(crate) fn argument_types_match_parameters(
    c: &mut Checker,
    args: &[Option<P<Type>>],
    sig: Option<P<Signature>>,
) -> bool {
    let Some(sig) = sig else {
        return false;
    };
    if args.len() < sig.min_argument_count() {
        return false;
    }
    let params = sig.parameters();
    if params.is_empty() {
        return args.is_empty();
    }
    if args.len() > params.len() && !sig.has_rest_parameter() {
        return false;
    }
    for (i, &arg_type) in args.iter().enumerate() {
        let mut param_index = i;
        if param_index >= params.len() {
            param_index = params.len() - 1;
        }
        let param_type = c.get_type_of_symbol_exported(params[param_index]);
        if !type_accepts_argument(c, arg_type, Some(param_type))
            && (sig.type_parameters().is_empty()
                || !has_compatible_call_shape(c, arg_type, Some(param_type)))
        {
            return false;
        }
    }
    true
}

// go: internal/typeparser/data_first_signature.go:229 candidateAcceptsSubject 6557ce6edb589f91
// candidateAcceptsSubject reports whether a function returned by candidate
// takes subjectType as its first parameter, followed by exactly tail more.
pub(crate) fn candidate_accepts_subject(
    c: &mut Checker,
    candidate: Option<P<Signature>>,
    subject_type: Option<P<Type>>,
    tail: i64,
) -> bool {
    let (Some(candidate), Some(_)) = (candidate, subject_type) else {
        return false;
    };
    let candidate_return = c.get_return_type_of_signature_exported(candidate);
    for &returned in c.get_signatures_of_type_exported(candidate_return, SignatureKind::Call) {
        if returned.parameters().len() as i64 != 1 + tail || returned.has_rest_parameter() {
            continue;
        }
        let parameter_type = c.get_type_of_symbol_exported(returned.parameters()[0]);
        if type_accepts_argument(c, subject_type, Some(parameter_type))
            || (!candidate.type_parameters().is_empty() || !returned.type_parameters().is_empty())
                && has_compatible_call_shape(c, subject_type, Some(parameter_type))
        {
            return true;
        }
    }
    false
}

// go: internal/typeparser/data_first_signature.go:250 hasCompatibleCallShape a70bd0311fe326bb
pub(crate) fn has_compatible_call_shape(
    c: &mut Checker,
    left: Option<P<Type>>,
    right: Option<P<Type>>,
) -> bool {
    let (Some(left), Some(right)) = (left, right) else {
        return false;
    };
    let left_callable = !c.get_signatures_of_type_exported(left, SignatureKind::Call).is_empty();
    let right_callable = !c.get_signatures_of_type_exported(right, SignatureKind::Call).is_empty();
    left_callable == right_callable
}

// go: internal/typeparser/data_first_signature.go:259 typeAcceptsArgument 5b927248530928b3
pub(crate) fn type_accepts_argument(
    c: &mut Checker,
    argument: Option<P<Type>>,
    parameter: Option<P<Type>>,
) -> bool {
    let (Some(argument), Some(parameter)) = (argument, parameter) else {
        return false;
    };
    if parameter.flags().intersects(TypeFlags::TypeParameter) {
        let constraint = c.get_constraint_of_type_parameter_exported(parameter);
        return constraint.is_none() || type_accepts_argument(c, Some(argument), constraint);
    }
    // Go: Checker_isTypeAssignableTo is the unexported isTypeAssignableTo, public in tsrs.
    same_shallow_type_origin(c, Some(argument), Some(parameter))
        || c.is_type_assignable_to(argument, parameter)
}

// go: internal/typeparser/data_first_signature.go:270 sameShallowTypeOrigin ddee47897b1d1fe6
pub(crate) fn same_shallow_type_origin(
    c: &mut Checker,
    left: Option<P<Type>>,
    right: Option<P<Type>>,
) -> bool {
    if left == right {
        return true;
    }
    let (Some(left), Some(right)) = (left, right) else {
        return false;
    };

    if left.object_flags().intersects(ObjectFlags::Reference)
        || right.object_flags().intersects(ObjectFlags::Reference)
    {
        if !left.object_flags().intersects(ObjectFlags::Reference)
            || !right.object_flags().intersects(ObjectFlags::Reference)
        {
            return false;
        }
        let (left_target, right_target) = (left.target(), right.target());
        return left_target == right_target
            || same_symbol_reference(
                c,
                left_target.and_then(|t| t.symbol()),
                right_target.and_then(|t| t.symbol()),
            );
    }

    let (left_alias, right_alias) = (left.alias(), right.alias());
    if left_alias.is_some() || right_alias.is_some() {
        return match (left_alias, right_alias) {
            (Some(left_alias), Some(right_alias)) => {
                same_symbol_reference(c, left_alias.symbol(), right_alias.symbol())
            }
            _ => false,
        };
    }

    if left.symbol().is_some() || right.symbol().is_some() {
        return same_symbol_reference(c, left.symbol(), right.symbol());
    }

    left.flags() == right.flags() && left.flags().intersects(TypeFlags::Singleton)
}

// go: internal/typeparser/data_first_signature.go:298 sameSymbolReference 4dcf04f307e4dade
pub(crate) fn same_symbol_reference(
    c: &mut Checker,
    left: Option<P<Symbol>>,
    right: Option<P<Symbol>>,
) -> bool {
    left.is_some() && right.is_some() && get_symbol_if_same_reference(c, left, right).is_some()
}

// go: internal/typeparser/data_first_signature.go:302 isLikelySelfParameter 8ef23c8d4686debe
pub(crate) fn is_likely_self_parameter(sym: Option<P<Symbol>>) -> bool {
    let Some(sym) = sym else {
        return false;
    };
    let name = go_to_lower(sym.name());
    name == "self" || name.starts_with("self") || name == "this"
}

/// Rust-only: Go's `strings.ToLower`, rune by rune with Go's simple lowercase mapping. Rust's `char::to_lowercase`
/// is the same mapping except for U+0130 (Latin capital I with dot), which Go maps to a plain `i` and Rust to `i`
/// plus a combining dot.
fn go_to_lower(s: &str) -> String {
    s.chars()
        .map(|r| if r == '\u{130}' { 'i' } else { r.to_lowercase().next().unwrap_or(r) })
        .collect()
}

// go: internal/typeparser/data_first_signature.go:310 omitArgAt a7472eff4597285f
pub(crate) fn omit_arg_at(nodes: &[P<Node>], index: usize) -> Vec<P<Node>> {
    let mut result = Vec::with_capacity(nodes.len().saturating_sub(1));
    for (i, &node) in nodes.iter().enumerate() {
        if i == index {
            continue;
        }
        result.push(node);
    }
    result
}
