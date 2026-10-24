use std::cell::RefCell;
use std::rc::Rc;

use ets_shim::ast::{ClassLikeBase, FunctionLikeBase, Kind, Node, NodeList, Symbol, SymbolFlags};
use ets_shim::checker::{Checker, ObjectFlags, Signature, Type, TypeFlags};
use ets_shim::core::P;
use rustc_hash::FxHashSet;

use crate::*;

// go: internal/typeparser/api_stability_safety.go:17 apiStabilitySafety
// apiStabilitySafety is the verdict of the pre-flight recursion guard that runs
// before a potentially recursive lazy checker read. The guard inspects the
// represented compiler type graph and the raw declaration structure without
// performing the guarded resolution itself. Safe authorizes the ordinary lazy
// read, Recursive and Unknown never do.
//
// The guard is a pure safety analysis: it decides only whether a read may
// expand a recursive alias without a lazy memo boundary. It never contributes
// stability findings and it never suppresses compiler diagnostics.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum ApiStabilitySafety {
    // go: internal/typeparser/api_stability_safety.go:19 apiStabilitySafetySafe
    Safe,
    Recursive,
    Unknown,
}

// go: internal/typeparser/api_stability_safety.go:25 apiStabilitySafetyMaxWork
// apiStabilitySafetyMaxWork bounds one guard run. Exhausting it yields
// Unknown, which blocks the read; it is never memoized as safe.
pub(crate) const API_STABILITY_SAFETY_MAX_WORK: i64 = 16_384;
// apiStabilitySafetyMaxDepth bounds nested alias applications on one guard
// path. Exceeding it yields Unknown.
pub(crate) const API_STABILITY_SAFETY_MAX_DEPTH: usize = 64;

// go: internal/typeparser/api_stability_safety.go:36 apiStabilitySafetyOperation
// apiStabilitySafetyOperation identifies the guarded read family a completed
// verdict belongs to, so a Safe result is only reused for the same operation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum ApiStabilitySafetyOperation {
    // go: internal/typeparser/api_stability_safety.go:38 apiStabilitySafetyOperationMemberTable
    MemberTable,
    BaseTypes,
    SymbolType,
    SignatureMembers,
    SignatureReturn,
    DeclaredType,
    Annotation,
}

// go: internal/typeparser/api_stability_safety.go:56 apiStabilitySafetyVerdictKey
// apiStabilitySafetyVerdictKey is the concrete identity of one guard request:
// the read operation, the represented compiler object or raw node it resolves,
// and the interned carrier of the substitution it is read under. The carrier
// is the identity of the whole substitution chain, so two requests that would
// resolve different represented components never share a verdict. Relation
// operand and projection scans remain internal to a request, so they cannot
// collide with a completed top-level verdict. No serialized context, string or
// raw declaration target is part of a key.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct ApiStabilitySafetyVerdictKey {
    pub(crate) operation: ApiStabilitySafetyOperation,
    pub(crate) t: Option<P<Type>>,
    pub(crate) signature: Option<P<Signature>>,
    pub(crate) symbol: Option<P<Symbol>>,
    pub(crate) node: Option<P<Node>>,
    pub(crate) carrier: Option<ApiStabilityCarrierRef>,
}

impl ApiStabilitySafetyVerdictKey {
    /// Rust-only: Go's keyed composite literal with the omitted fields nil.
    fn new(operation: ApiStabilitySafetyOperation) -> Self {
        ApiStabilitySafetyVerdictKey {
            operation,
            t: None,
            signature: None,
            symbol: None,
            node: None,
            carrier: None,
        }
    }
}

// go: internal/typeparser/api_stability_safety.go:69 apiStabilitySafetyArg
// apiStabilitySafetyArg is one concrete argument of a represented generic
// application. Either the checker already represented the argument type, or the
// raw argument node is kept together with the environment it was written in so
// a type parameter argument can still be followed through an outer binding.
#[derive(Clone, Debug, Default)]
pub(crate) struct ApiStabilitySafetyArg {
    pub(crate) t: Option<P<Type>>,
    pub(crate) node: Option<P<Node>>,
    pub(crate) subst: ApiStabilitySubstitution,
    pub(crate) bindings: Option<Rc<ApiStabilitySafetyBinding>>,
}

impl ApiStabilitySafetyArg {
    // go: internal/typeparser/api_stability_safety.go:76 apiStabilitySafetyArg.empty
    pub(crate) fn empty(&self) -> bool {
        self.t.is_none() && self.node.is_none()
    }
}

// go: internal/typeparser/api_stability_safety.go:80 safetyArgsEqual
pub(crate) fn safety_args_equal(
    left: &[ApiStabilitySafetyArg],
    right: &[ApiStabilitySafetyArg],
) -> bool {
    if left.len() != right.len() {
        return false;
    }
    for index in 0..left.len() {
        if left[index].t.is_some() || right[index].t.is_some() {
            if left[index].t != right[index].t {
                return false;
            }
            continue;
        }
        if left[index].node != right[index].node {
            return false;
        }
    }
    true
}

// go: internal/typeparser/api_stability_safety.go:101 apiStabilitySafetyBinding
// apiStabilitySafetyBinding is one generic-application frame: the declared type
// parameters of a class, interface or alias bound to the arguments of one
// application. Frames chain so a nested application keeps the outer bindings.
//
// Rust: Go shares the args slice with the code that built the frame, which keeps filling it after the frame
// exists (omittedArgumentDefaults binds each default as it goes), so the args are a `RefCell`.
#[derive(Debug, Default)]
pub(crate) struct ApiStabilitySafetyBinding {
    pub(crate) parent: Option<Rc<ApiStabilitySafetyBinding>>,
    pub(crate) params: Vec<P<Type>>,
    pub(crate) args: RefCell<Vec<ApiStabilitySafetyArg>>,
}

// go: internal/typeparser/api_stability_safety.go:111 apiStabilitySafetyApplication
// apiStabilitySafetyApplication is one alias application on the current guard
// path. The concrete argument list is compared by identity so a repeated
// application of the same alias with the same represented arguments is
// recognized as an evaluated cycle.
#[derive(Clone, Debug)]
pub(crate) struct ApiStabilitySafetyApplication {
    pub(crate) symbol: P<Symbol>,
    pub(crate) args: Vec<ApiStabilitySafetyArg>,
}

// go: internal/typeparser/api_stability_safety.go:121 apiStabilitySafetyScan
// apiStabilitySafetyScan is the mutable state of one guard run. The verdict is
// computed by traversing raw annotations, represented types and declaration
// structure; a work and depth bound makes the traversal total and fails closed.
// All nested scans share one analysis-level work counter so a chain of nested
// argument verifications cannot run away.
pub(crate) struct ApiStabilitySafetyScan<'s> {
    pub(crate) a: &'s mut ApiStabilityAnalysis,

    pub(crate) stack: Vec<ApiStabilitySafetyApplication>,

    // relationOperands marks a nested scan that verifies a conditional operand
    // before the compiler relation is asked to compare it. The relation compares
    // the full projected member surface of both operands (properties, call and
    // construct returns, index infos) and instantiates generic members, so the
    // operand scan evaluates return annotations and refuses any component the
    // relation could instantiate (an unbound type parameter, a deferral, an
    // inferred return).
    pub(crate) relation_operands: bool,

    pub(crate) active_declarations: FxHashSet<P<Symbol>>,
    pub(crate) materializing: FxHashSet<P<Type>>,
    pub(crate) materializing_nodes: FxHashSet<P<Node>>,
}

impl ApiStabilitySafetyScan<'_> {
    // go: internal/typeparser/api_stability_safety.go:140 apiStabilitySafetyScan.exceeded
    pub(crate) fn exceeded(&mut self) -> bool {
        self.a.safety_work += 1;
        self.a.safety_work > API_STABILITY_SAFETY_MAX_WORK
    }
}

// go: internal/typeparser/api_stability_safety.go:147 combineSafety
// combineSafety merges two verdicts of one read: any recursion makes the whole
// read recursive, and any unknown makes it unknown.
pub(crate) fn combine_safety(
    left: ApiStabilitySafety,
    right: ApiStabilitySafety,
) -> ApiStabilitySafety {
    if left == ApiStabilitySafety::Recursive || right == ApiStabilitySafety::Recursive {
        return ApiStabilitySafety::Recursive;
    }
    if left == ApiStabilitySafety::Unknown || right == ApiStabilitySafety::Unknown {
        return ApiStabilitySafety::Unknown;
    }
    ApiStabilitySafety::Safe
}

impl ApiStabilityAnalysis {
    // go: internal/typeparser/api_stability_safety.go:165 apiStabilityAnalysis.symbolTypeResolutionIsSafe
    // symbolTypeResolutionIsSafe reports whether resolving an unmaterialized
    // symbol's type through the ordinary lazy accessor is bounded. An annotation or
    // a function-like declaration is read through the recursion guard; an inferred
    // declaration (an initializer, a parameter default or an accessor body) is read
    // through the checker's ordinary lazy inference. Inference is the compiler's own
    // lazy read: it attributes any diagnostics it produces to the declaring file's
    // own expressions, so a later check of that file observes exactly the same
    // diagnostics and types.
    pub(crate) fn symbol_type_resolution_is_safe(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: Option<P<Symbol>>,
    ) -> bool {
        let Some(symbol) = symbol else {
            return false;
        };
        if symbol.declarations().is_empty() {
            return false;
        }
        let mut subst = ApiStabilitySubstitution::default();
        if let Some(mapper) = get_instantiated_symbol_mapper(tp.checker, Some(symbol)) {
            subst =
                self.extend_mapper_substitution(&ApiStabilitySubstitution::default(), None, mapper);
        }
        let key = ApiStabilitySafetyVerdictKey {
            symbol: Some(symbol),
            carrier: subst.carrier(),
            ..ApiStabilitySafetyVerdictKey::new(ApiStabilitySafetyOperation::SymbolType)
        };
        if self.safety_safe_verdict(&key) {
            return true;
        }
        if self.safety_budget_exhausted() {
            return false;
        }
        for &declaration in symbol.declarations() {
            if declaration.function_like_data().is_some() {
                // Resolving a function-like value creates its signature: parameter
                // types and type parameter constraints are resolved eagerly, the
                // return type stays lazy and is gated by its own read. An accessor
                // that must infer its type from its body checks the body exactly
                // like function and initializer inference does: its diagnostics
                // belong to the declaring file's own expressions, and it is only
                // read through the native source-check lifecycle.
                if (declaration.kind() == Kind::GetAccessor
                    || declaration.kind() == Kind::SetAccessor)
                    && !self.inferred_component_resolution_is_safe(tp, Some(declaration))
                {
                    return false;
                }
                let verdict =
                    self.new_safety_scan().function_like_resolution(tp, declaration, &subst, None);
                if verdict != ApiStabilitySafety::Safe {
                    return false;
                }
                continue;
            }
            let Some(annotation) = declaration_annotation_node_of(Some(declaration)) else {
                // An inferred declaration is read through the ordinary lazy
                // accessor. The checker attributes the diagnostics inference
                // produces to the declaring file's expressions, so the checked
                // program keeps them exactly. Inference is only read while the
                // checker is inside its source-file check lifecycle; outside it a
                // speculative inference is refused.
                if !self.inferred_component_resolution_is_safe(tp, Some(declaration)) {
                    return false;
                }
                continue;
            };
            let verdict =
                self.new_safety_scan().type_node(tp, Some(annotation), &subst, None, false);
            if verdict != ApiStabilitySafety::Safe {
                return false;
            }
        }
        self.record_safety_safe_verdict(key);
        true
    }

    // go: internal/typeparser/api_stability_safety.go:227 apiStabilityAnalysis.signatureMemberResolutionIsSafe
    // signatureMemberResolutionIsSafe reports whether materializing the raw
    // signatures of a signature member symbol is bounded. It only inspects
    // parameter annotations, which signature creation resolves eagerly.
    pub(crate) fn signature_member_resolution_is_safe(
        &mut self,
        tp: &mut TypeParser<'_>,
        member: Option<P<Symbol>>,
        subst: &ApiStabilitySubstitution,
    ) -> bool {
        let Some(member) = member else {
            return false;
        };
        let key = ApiStabilitySafetyVerdictKey {
            symbol: Some(member),
            carrier: subst.carrier(),
            ..ApiStabilitySafetyVerdictKey::new(ApiStabilitySafetyOperation::SignatureMembers)
        };
        if self.safety_safe_verdict(&key) {
            return true;
        }
        if self.safety_budget_exhausted() {
            return false;
        }
        let mut s = self.new_safety_scan();
        for &declaration in member.declarations() {
            if declaration.function_like_data().is_none() {
                continue;
            }
            if s.function_like_resolution(tp, declaration, subst, None) != ApiStabilitySafety::Safe
            {
                return false;
            }
        }
        self.record_safety_safe_verdict(key);
        true
    }

    // go: internal/typeparser/api_stability_safety.go:255 apiStabilityAnalysis.annotationResolutionIsSafe
    // annotationResolutionIsSafe reports whether resolving a raw annotation node
    // through the ordinary lazy type accessor is bounded. The substitution context
    // is the concrete enclosing application whose mapper will be applied when the
    // annotation is instantiated.
    pub(crate) fn annotation_resolution_is_safe(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Option<P<Node>>,
        subst: &ApiStabilitySubstitution,
    ) -> bool {
        let Some(node) = node else {
            return false;
        };
        let key = ApiStabilitySafetyVerdictKey {
            node: Some(node),
            carrier: subst.carrier(),
            ..ApiStabilitySafetyVerdictKey::new(ApiStabilitySafetyOperation::Annotation)
        };
        if self.safety_safe_verdict(&key) {
            return true;
        }
        if self.safety_budget_exhausted() {
            return false;
        }
        if self.new_safety_scan().type_node(tp, Some(node), subst, None, false)
            != ApiStabilitySafety::Safe
        {
            return false;
        }
        self.record_safety_safe_verdict(key);
        true
    }

    // go: internal/typeparser/api_stability_safety.go:278 apiStabilityAnalysis.declaredTypeResolutionIsSafe
    // declaredTypeResolutionIsSafe reports whether resolving a symbol's declared
    // type through the ordinary lazy accessor is bounded. Only a type alias
    // actually resolves its right-hand side; a class or interface declared type is
    // created without resolving members.
    pub(crate) fn declared_type_resolution_is_safe(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: Option<P<Symbol>>,
    ) -> bool {
        let Some(symbol) = symbol else {
            return false;
        };
        if symbol.flags().intersects(SymbolFlags::Alias) {
            if has_alias_declaration(symbol) {
                let target = api_stability_immediate_aliased_symbol(tp.checker, Some(symbol));
                if let Some(target) = target
                    && target != symbol
                {
                    return self.declared_type_resolution_is_safe(tp, Some(target));
                }
            }
            return true;
        }
        if !symbol.flags().intersects(SymbolFlags::TypeAlias) {
            return true;
        }
        let key = ApiStabilitySafetyVerdictKey {
            symbol: Some(symbol),
            ..ApiStabilitySafetyVerdictKey::new(ApiStabilitySafetyOperation::DeclaredType)
        };
        if self.safety_safe_verdict(&key) {
            return true;
        }
        if self.safety_budget_exhausted() {
            return false;
        }
        let Some(declaration) = api_stability_type_alias_declaration(Some(symbol)) else {
            return false;
        };
        if self.new_safety_scan().type_node(
            tp,
            declaration.type_node(),
            &ApiStabilitySubstitution::default(),
            None,
            false,
        ) != ApiStabilitySafety::Safe
        {
            return false;
        }
        self.record_safety_safe_verdict(key);
        true
    }

    // go: internal/typeparser/api_stability_safety.go:321 apiStabilityAnalysis.signatureReturnResolutionIsSafe
    // signatureReturnResolutionIsSafe reports whether materializing an
    // unmaterialized signature return through the ordinary lazy accessor is
    // bounded. An inferred return has no annotation: it is read through the
    // checker's ordinary lazy inference, which attributes the diagnostics it
    // produces to the declaring file's own expressions, so a later check of that
    // file observes exactly the same diagnostics and types. An accessor's inferred
    // return checks the accessor body exactly like an inferred component, so it is
    // read through the same native source-check lifecycle.
    pub(crate) fn signature_return_resolution_is_safe(
        &mut self,
        tp: &mut TypeParser<'_>,
        signature: Option<P<Signature>>,
        subst: &ApiStabilitySubstitution,
    ) -> bool {
        let Some(signature) = signature else {
            return false;
        };
        let Some(node) = signature_return_type_node(Some(signature)) else {
            let Some(declaration) = signature.declaration() else {
                return false;
            };
            return self.inferred_component_resolution_is_safe(tp, Some(declaration));
        };
        let key = ApiStabilitySafetyVerdictKey {
            signature: Some(signature),
            carrier: subst.carrier(),
            ..ApiStabilitySafetyVerdictKey::new(ApiStabilitySafetyOperation::SignatureReturn)
        };
        if self.safety_safe_verdict(&key) {
            return true;
        }
        if self.safety_budget_exhausted() {
            return false;
        }
        if self.new_safety_scan().type_node(tp, Some(node), subst, None, false)
            != ApiStabilitySafety::Safe
        {
            return false;
        }
        self.record_safety_safe_verdict(key);
        true
    }

    // go: internal/typeparser/api_stability_safety.go:355 apiStabilityAnalysis.inferredComponentResolutionIsSafe
    // inferredComponentResolutionIsSafe reports whether the native lazy inference
    // of an unmaterialized component declared in a source file may be read. The
    // ordinary lazy accessors attribute the diagnostics inference produces to the
    // declaring file's own expressions, exactly as that file's ordinary check
    // would, but that attribution is only established while the checker is inside
    // its source-file check lifecycle (the after-check callback). A speculative
    // read outside that lifecycle keeps refusing inferred components.
    #[expect(clippy::unused_self, reason = "a method of Go's apiStabilityAnalysis")]
    pub(crate) fn inferred_component_resolution_is_safe(
        &self,
        tp: &TypeParser<'_>,
        declaration: Option<P<Node>>,
    ) -> bool {
        let Some(declaration) = declaration else {
            return false;
        };
        if let Some(source_file) = ets_shim::ast::get_source_file_of_node(declaration)
            && is_source_file_type_checked(tp.checker, source_file)
        {
            return true;
        }
        is_checking_source_file(tp.checker)
    }

    // go: internal/typeparser/api_stability_safety.go:369 apiStabilityAnalysis.baseTypesResolutionIsSafe
    // baseTypesResolutionIsSafe reports whether materializing an unmaterialized
    // heritage surface is bounded. Resolving base types evaluates the heritage type
    // argument annotations; the base declarations' own member surfaces are gated by
    // their own reads.
    pub(crate) fn base_types_resolution_is_safe(
        &mut self,
        tp: &mut TypeParser<'_>,
        declaration: Option<P<Type>>,
        subst: &ApiStabilitySubstitution,
    ) -> bool {
        let Some(declaration) = declaration else {
            return false;
        };
        let Some(symbol) = declaration.symbol() else {
            return false;
        };
        let key = ApiStabilitySafetyVerdictKey {
            t: Some(declaration),
            carrier: subst.carrier(),
            ..ApiStabilitySafetyVerdictKey::new(ApiStabilitySafetyOperation::BaseTypes)
        };
        if self.safety_safe_verdict(&key) {
            return true;
        }
        if self.safety_budget_exhausted() {
            return false;
        }
        if self.new_safety_scan().heritage(tp, Some(symbol), subst, None, false)
            != ApiStabilitySafety::Safe
        {
            return false;
        }
        self.record_safety_safe_verdict(key);
        true
    }

    // go: internal/typeparser/api_stability_safety.go:397 apiStabilityAnalysis.memberTableResolutionIsSafe
    // memberTableResolutionIsSafe reports whether resolving a structured type's
    // member table (properties, signatures or index infos) through the ordinary
    // lazy accessors is bounded. Member resolution eagerly instantiates declared
    // index signature annotations, creates the declared call and construct
    // signatures, and recursively resolves inherited base member tables.
    pub(crate) fn member_table_resolution_is_safe(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: Option<P<Type>>,
        subst: &ApiStabilitySubstitution,
    ) -> bool {
        let Some(t) = t else {
            return false;
        };
        if !t.flags().intersects(TypeFlags::Object) {
            return true;
        }
        let key = ApiStabilitySafetyVerdictKey {
            t: Some(t),
            carrier: subst.carrier(),
            ..ApiStabilitySafetyVerdictKey::new(ApiStabilitySafetyOperation::MemberTable)
        };
        if self.safety_safe_verdict(&key) {
            return true;
        }
        if self.safety_budget_exhausted() {
            return false;
        }
        if self.new_safety_scan().member_table(tp, Some(t), subst, None) != ApiStabilitySafety::Safe
        {
            return false;
        }
        self.record_safety_safe_verdict(key);
        true
    }

    // go: internal/typeparser/api_stability_safety.go:423 apiStabilityAnalysis.safetyBudgetExhausted
    // safetyBudgetExhausted reports whether this analysis has already consumed its
    // shared guard budget. A guard request after exhaustion is refused immediately
    // without allocating scan state; the traversal still collects represented
    // dependencies and declaration metadata.
    pub(crate) fn safety_budget_exhausted(&self) -> bool {
        self.safety_work > API_STABILITY_SAFETY_MAX_WORK
    }

    // go: internal/typeparser/api_stability_safety.go:431 apiStabilityAnalysis.safetySafeVerdict
    // safetySafeVerdict reports whether an identical guard request already
    // completed safely in this analysis. Only completed Safe verdicts are reused:
    // Unknown, recursion and exhaustion stay retryable by later scans and later
    // analyses.
    pub(crate) fn safety_safe_verdict(&self, key: &ApiStabilitySafetyVerdictKey) -> bool {
        self.safety_safe_memo.contains(key)
    }

    // go: internal/typeparser/api_stability_safety.go:440 apiStabilityAnalysis.recordSafetySafeVerdict
    // recordSafetySafeVerdict records one completed Safe guard verdict.
    pub(crate) fn record_safety_safe_verdict(&mut self, key: ApiStabilitySafetyVerdictKey) {
        self.safety_safe_memo.insert(key);
    }

    // go: internal/typeparser/api_stability_safety.go:452 apiStabilityAnalysis.symbolAtTypeNameNode
    // symbolAtTypeNameNode returns the symbol named by a raw type-name, heritage,
    // qualifier or declaration-name node, caching successful lookups by AST
    // identity for this analysis. A failed lookup is never cached. Only the symbol
    // lookup is cached: no resolved type, binding-dependent member identity or
    // context string is ever stored here.
    pub(crate) fn symbol_at_type_name_node(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Option<P<Node>>,
    ) -> Option<P<Symbol>> {
        let node = node?;
        if let Some(&symbol) = self.type_name_symbols.get(&node) {
            return Some(symbol);
        }
        let symbol = tp.get_symbol_at_location(node);
        if let Some(symbol) = symbol {
            self.type_name_symbols.insert(node, symbol);
        }
        symbol
    }

    // go: internal/typeparser/api_stability_safety.go:469 apiStabilityAnalysis.newSafetyScan
    pub(crate) fn new_safety_scan(&mut self) -> ApiStabilitySafetyScan<'_> {
        ApiStabilitySafetyScan {
            a: self,
            stack: Vec::new(),
            relation_operands: false,
            active_declarations: FxHashSet::default(),
            materializing: FxHashSet::default(),
            materializing_nodes: FxHashSet::default(),
        }
    }
}

impl ApiStabilitySafetyScan<'_> {
    // go: internal/typeparser/api_stability_safety.go:477 apiStabilitySafetyScan.typeNode
    // typeNode scans one raw type annotation node. project marks a position whose
    // members are evaluated because the surrounding construct projects into them
    // (an indexed access or a keyof target), so nested property annotations are
    // evaluated as well.
    pub(crate) fn type_node(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Option<P<Node>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        let Some(node) = node else {
            return ApiStabilitySafety::Safe;
        };
        if self.exceeded() {
            return ApiStabilitySafety::Unknown;
        }
        match node.kind() {
            Kind::AnyKeyword
            | Kind::UnknownKeyword
            | Kind::NeverKeyword
            | Kind::VoidKeyword
            | Kind::UndefinedKeyword
            | Kind::NullKeyword
            | Kind::StringKeyword
            | Kind::NumberKeyword
            | Kind::BigIntKeyword
            | Kind::BooleanKeyword
            | Kind::SymbolKeyword
            | Kind::ObjectKeyword
            | Kind::ThisType
            | Kind::IntrinsicKeyword
            | Kind::LiteralType => ApiStabilitySafety::Safe,
            Kind::ParenthesizedType => self.type_node(
                tp,
                Some(node.as_parenthesized_type_node().type_()),
                subst,
                bindings,
                project,
            ),
            Kind::ArrayType => self.type_node(
                tp,
                Some(node.as_array_type_node().element_type()),
                subst,
                bindings,
                project,
            ),
            Kind::OptionalType => self.type_node(
                tp,
                Some(node.as_optional_type_node().type_()),
                subst,
                bindings,
                project,
            ),
            Kind::RestType => {
                self.type_node(tp, Some(node.as_rest_type_node().type_()), subst, bindings, project)
            }
            Kind::TypeOperator => {
                let operator = node.as_type_operator_node();
                if operator.operator() == Kind::KeyOfKeyword {
                    // `keyof T` resolves the keys of T's member surface.
                    return self.type_node(tp, Some(operator.type_()), subst, bindings, true);
                }
                self.type_node(tp, Some(operator.type_()), subst, bindings, project)
            }
            Kind::NamedTupleMember => self.type_node(
                tp,
                Some(node.as_named_tuple_member().type_()),
                subst,
                bindings,
                project,
            ),
            Kind::TupleType => {
                let mut verdict = ApiStabilitySafety::Safe;
                for &element in node.as_tuple_type_node().elements().nodes() {
                    let element_verdict =
                        self.type_node(tp, Some(element), subst, bindings, project);
                    verdict = combine_safety(verdict, element_verdict);
                }
                verdict
            }
            Kind::UnionType => {
                let mut verdict = ApiStabilitySafety::Safe;
                for &member in node.as_union_type_node().types().nodes() {
                    let member_verdict = self.type_node(tp, Some(member), subst, bindings, project);
                    verdict = combine_safety(verdict, member_verdict);
                }
                verdict
            }
            Kind::IntersectionType => {
                let mut verdict = ApiStabilitySafety::Safe;
                for &member in node.as_intersection_type_node().types().nodes() {
                    let member_verdict = self.type_node(tp, Some(member), subst, bindings, project);
                    verdict = combine_safety(verdict, member_verdict);
                }
                verdict
            }
            Kind::TemplateLiteralType => {
                let mut verdict = ApiStabilitySafety::Safe;
                for &span in node.as_template_literal_type_node().template_spans().nodes() {
                    if span.kind() != Kind::TemplateLiteralTypeSpan {
                        continue;
                    }
                    let span_verdict = self.type_node(
                        tp,
                        Some(span.as_template_literal_type_span().type_()),
                        subst,
                        bindings,
                        project,
                    );
                    verdict = combine_safety(verdict, span_verdict);
                }
                verdict
            }
            Kind::FunctionType | Kind::ConstructorType => {
                self.function_like_resolution(tp, node, subst, bindings)
            }
            Kind::TypeLiteral => self.type_literal(tp, node, subst, bindings, project),
            Kind::MappedType => self.mapped_type_node(tp, node, subst, bindings, project),
            Kind::ConditionalType => self.conditional_node(tp, node, subst, bindings, project),
            Kind::IndexedAccessType => {
                let indexed = node.as_indexed_access_type_node();
                let verdict =
                    self.type_node(tp, Some(indexed.object_type()), subst, bindings, true);
                let index_verdict =
                    self.type_node(tp, Some(indexed.index_type()), subst, bindings, project);
                combine_safety(verdict, index_verdict)
            }
            Kind::TypeReference => self.type_reference(tp, node, subst, bindings, project),
            Kind::ExpressionWithTypeArguments => {
                let expression = node.as_expression_with_type_arguments();
                let verdict = self.type_arguments(tp, expression.type_arguments(), subst, bindings);
                if !project {
                    return verdict;
                }
                let symbol = self.a.symbol_at_type_name_node(tp, Some(expression.expression()));
                let members = self.declaration_members(
                    tp,
                    symbol,
                    expression.type_arguments(),
                    subst,
                    bindings,
                    true,
                );
                combine_safety(verdict, members)
            }
            Kind::TypeQuery => self.type_query(tp, node, subst, bindings, project),
            Kind::ImportType => {
                let import_type = node.as_import_type_node();
                let verdict =
                    self.type_arguments(tp, import_type.type_arguments(), subst, bindings);
                let Some(qualifier) = import_type.qualifier() else {
                    return combine_safety(verdict, ApiStabilitySafety::Unknown);
                };
                let Some(symbol) = self.a.symbol_at_type_name_node(tp, Some(qualifier)) else {
                    return combine_safety(verdict, ApiStabilitySafety::Unknown);
                };
                if !project {
                    return verdict;
                }
                let members = self.declaration_members(
                    tp,
                    Some(symbol),
                    import_type.type_arguments(),
                    subst,
                    bindings,
                    true,
                );
                combine_safety(verdict, members)
            }
            Kind::InferType => ApiStabilitySafety::Safe,
            _ => ApiStabilitySafety::Unknown,
        }
    }

    // go: internal/typeparser/api_stability_safety.go:588 apiStabilitySafetyScan.typeArguments
    // typeArguments scans the eagerly evaluated type arguments of a reference.
    pub(crate) fn type_arguments(
        &mut self,
        tp: &mut TypeParser<'_>,
        arguments: Option<P<NodeList>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
    ) -> ApiStabilitySafety {
        let mut verdict = ApiStabilitySafety::Safe;
        let Some(arguments) = arguments else {
            return verdict;
        };
        for &argument in arguments.nodes() {
            let argument_verdict = self.type_node(tp, Some(argument), subst, bindings, false);
            verdict = combine_safety(verdict, argument_verdict);
        }
        verdict
    }

    // go: internal/typeparser/api_stability_safety.go:604 apiStabilitySafetyScan.typeReference
    // typeReference scans an application or declaration reference. A reference to a
    // bound type parameter follows its concrete binding; a type alias application
    // follows the alias declaration with a new binding frame and detects an
    // evaluated cycle; a class or interface reference evaluates its type arguments
    // and, when projected, its declaration member surface.
    pub(crate) fn type_reference(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: P<Node>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        let reference = node.as_type_reference_node();
        let Some(symbol) = self.a.symbol_at_type_name_node(tp, Some(reference.type_name())) else {
            return ApiStabilitySafety::Unknown;
        };
        let flags = symbol.flags();
        if flags.intersects(SymbolFlags::TypeParameter) {
            return self.type_parameter_reference(tp, symbol, subst, bindings, project);
        }
        if flags.intersects(SymbolFlags::Alias) {
            if has_alias_declaration(symbol) {
                let target = api_stability_immediate_aliased_symbol(tp.checker, Some(symbol));
                if let Some(target) = target
                    && target != symbol
                {
                    return self.reference_to_symbol(
                        tp,
                        target,
                        reference.type_arguments(),
                        subst,
                        bindings,
                        project,
                    );
                }
            }
            return ApiStabilitySafety::Unknown;
        }
        if flags.intersects(SymbolFlags::TypeAlias) {
            return self.alias_application(
                tp,
                symbol,
                reference.type_arguments(),
                subst,
                bindings,
                project,
            );
        }
        if flags.intersects(SymbolFlags::Class | SymbolFlags::Interface | SymbolFlags::Enum) {
            let mut verdict = self.type_arguments(tp, reference.type_arguments(), subst, bindings);
            let defaults = self.omitted_argument_defaults(
                tp,
                symbol,
                reference.type_arguments(),
                subst,
                bindings,
            );
            verdict = combine_safety(verdict, defaults);
            if !project {
                return verdict;
            }
            let members = self.declaration_members(
                tp,
                Some(symbol),
                reference.type_arguments(),
                subst,
                bindings,
                true,
            );
            return combine_safety(verdict, members);
        }
        ApiStabilitySafety::Unknown
    }

    // go: internal/typeparser/api_stability_safety.go:639 apiStabilitySafetyScan.referenceToSymbol
    // referenceToSymbol resolves an import alias reference to its target symbol.
    pub(crate) fn reference_to_symbol(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: P<Symbol>,
        arguments: Option<P<NodeList>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        let flags = symbol.flags();
        if flags.intersects(SymbolFlags::TypeAlias) {
            return self.alias_application(tp, symbol, arguments, subst, bindings, project);
        }
        if flags.intersects(SymbolFlags::TypeParameter) {
            return self.type_parameter_reference(tp, symbol, subst, bindings, project);
        }
        if flags.intersects(SymbolFlags::Class | SymbolFlags::Interface | SymbolFlags::Enum) {
            let mut verdict = self.type_arguments(tp, arguments, subst, bindings);
            let defaults = self.omitted_argument_defaults(tp, symbol, arguments, subst, bindings);
            verdict = combine_safety(verdict, defaults);
            if !project {
                return verdict;
            }
            let members =
                self.declaration_members(tp, Some(symbol), arguments, subst, bindings, true);
            return combine_safety(verdict, members);
        }
        ApiStabilitySafety::Unknown
    }

    // go: internal/typeparser/api_stability_safety.go:662 apiStabilitySafetyScan.omittedArgumentDefaults
    // omittedArgumentDefaults scans the declared default annotations the checker
    // evaluates when a generic application omits arguments. Later defaults are
    // scanned with the earlier parameters bound, so a default that references an
    // earlier parameter still follows its concrete argument. A required parameter
    // the application omits is an error the guard refuses to evaluate.
    pub(crate) fn omitted_argument_defaults(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: P<Symbol>,
        arguments: Option<P<NodeList>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
    ) -> ApiStabilitySafety {
        let defaults = safety_type_parameter_defaults(Some(symbol));
        let params = local_type_parameters(tp.checker, symbol);
        if defaults.is_empty() || params.is_empty() {
            return ApiStabilitySafety::Safe;
        }
        let argument_nodes: &'static [P<Node>] = arguments.map_or(&[], |list| list.nodes());
        let provided = argument_nodes.len();
        if provided >= params.len() {
            return ApiStabilitySafety::Safe;
        }
        let mut args = vec![ApiStabilitySafetyArg::default(); params.len()];
        for index in 0..provided.min(params.len()) {
            args[index] = ApiStabilitySafetyArg {
                node: Some(argument_nodes[index]),
                subst: subst.clone(),
                bindings: bindings.cloned(),
                ..Default::default()
            };
        }
        let params_len = params.len();
        let frame = Rc::new(ApiStabilitySafetyBinding {
            parent: bindings.cloned(),
            params,
            args: RefCell::new(args),
        });
        let mut verdict = ApiStabilitySafety::Safe;
        for index in provided..params_len {
            let Some(default_node) = defaults.get(index).copied().flatten() else {
                return ApiStabilitySafety::Unknown;
            };
            frame.args.borrow_mut()[index] =
                ApiStabilitySafetyArg { node: Some(default_node), ..Default::default() };
            let default_verdict =
                self.type_node(tp, Some(default_node), subst, Some(&frame), false);
            verdict = combine_safety(verdict, default_verdict);
        }
        verdict
    }

    // go: internal/typeparser/api_stability_safety.go:698 apiStabilitySafetyScan.typeParameterReference
    // typeParameterReference follows a type parameter reference through the active
    // substitution and binding frames. An unbound parameter keeps the annotation
    // deferred, so the guarded read evaluates nothing for that reference and it is
    // safe. A relation-operand scan refuses an unbound parameter instead: the
    // relation may instantiate it against the other operand.
    pub(crate) fn type_parameter_reference(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: P<Symbol>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        self.a.note_materializing_symbol_read(
            ApiStabilityMaterializationReadKind::DeclaredType,
            Some(symbol),
        );
        let parameter = tp.checker.get_declared_type_of_symbol_exported(symbol);
        if let Some(argument) = safety_binding_lookup(bindings, Some(parameter)) {
            if argument.empty() {
                return ApiStabilitySafety::Unknown;
            }
            if argument.t.is_some() {
                return self.type_value(
                    tp,
                    argument.t,
                    &argument.subst,
                    argument.bindings.as_ref(),
                    project,
                );
            }
            return self.type_node(
                tp,
                argument.node,
                &argument.subst,
                argument.bindings.as_ref(),
                project,
            );
        }
        if let Some((mapped, parent)) = self.a.substitute(tp, subst, Some(parameter)) {
            let Some(mapped) = mapped else {
                // The parameter was replaced by an argument the checker has not
                // represented; the applied value is unknown.
                return ApiStabilitySafety::Unknown;
            };
            if mapped == parameter {
                if self.relation_operands {
                    return ApiStabilitySafety::Unknown;
                }
                return ApiStabilitySafety::Safe;
            }
            return self.type_value(tp, Some(mapped), &parent, bindings, project);
        }
        if self.relation_operands {
            return ApiStabilitySafety::Unknown;
        }
        ApiStabilitySafety::Safe
    }
}

// go: internal/typeparser/api_stability_safety.go:736 safetyBindingLookup
// safetyBindingLookup finds the binding of a declared type parameter in a
// binding chain.
pub(crate) fn safety_binding_lookup(
    bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
    parameter: Option<P<Type>>,
) -> Option<ApiStabilitySafetyArg> {
    let parameter = parameter?;
    let symbol = parameter.symbol();
    let mut frame = bindings.map(|binding| &**binding);
    while let Some(current) = frame {
        let args = current.args.borrow();
        for (index, &candidate) in current.params.iter().enumerate() {
            if index >= args.len() {
                continue;
            }
            if candidate == parameter || (symbol.is_some() && candidate.symbol() == symbol) {
                return Some(args[index].clone());
            }
        }
        drop(args);
        frame = current.parent.as_deref();
    }
    None
}

/// Rust-only: Go's `c.GetLocalTypeParametersOfClassOrInterfaceOrTypeAlias(symbol)`.
pub(crate) fn local_type_parameters(c: &mut Checker, symbol: P<Symbol>) -> Vec<P<Type>> {
    c.get_local_type_parameters_of_class_or_interface_or_type_alias_exported(symbol)
}

impl ApiStabilitySafetyScan<'_> {
    // go: internal/typeparser/api_stability_safety.go:760 apiStabilitySafetyScan.aliasApplication
    // aliasApplication follows one application of a type alias. The alias
    // declaration is scanned with a binding frame for its declared parameters. A
    // repeated application of the same alias with the same concrete arguments on
    // the eager path is an evaluated cycle; a cycle crossed only through lazy
    // positions is never scanned, so reaching it here means the cycle is
    // evaluated.
    pub(crate) fn alias_application(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: P<Symbol>,
        arguments: Option<P<NodeList>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        let Some(declaration) = api_stability_type_alias_declaration(Some(symbol)) else {
            return ApiStabilitySafety::Unknown;
        };
        let params = local_type_parameters(tp.checker, symbol);
        let mut args = vec![ApiStabilitySafetyArg::default(); params.len()];
        let mut verdict = ApiStabilitySafety::Safe;
        if let Some(arguments) = arguments {
            for (index, &argument) in arguments.nodes().iter().enumerate() {
                // Every explicit argument is evaluated eagerly when the checker
                // creates the application, even when the alias right-hand side
                // ignores it. Scan each provided argument in the environment it was
                // written in.
                let argument_verdict = self.type_node(tp, Some(argument), subst, bindings, false);
                verdict = combine_safety(verdict, argument_verdict);
                if index >= params.len() {
                    continue;
                }
                args[index] = ApiStabilitySafetyArg {
                    node: Some(argument),
                    subst: subst.clone(),
                    bindings: bindings.cloned(),
                    ..Default::default()
                };
            }
        }

        // Omitted arguments are filled from declared defaults, which the checker
        // evaluates when it creates the application.
        let mut defaults: Vec<P<Node>> = Vec::new();
        if !params.is_empty() {
            let default_nodes = safety_type_parameter_defaults(Some(symbol));
            for (index, arg) in args.iter_mut().enumerate() {
                if !arg.empty() {
                    continue;
                }
                if let Some(default_node) = default_nodes.get(index).copied().flatten() {
                    *arg = ApiStabilitySafetyArg { node: Some(default_node), ..Default::default() };
                    defaults.push(default_node);
                    continue;
                }
                // A required argument the application omits is an error the guard
                // refuses to evaluate.
                return ApiStabilitySafety::Unknown;
            }
        }
        for application in &self.stack {
            if application.symbol == symbol && safety_args_equal(&application.args, &args) {
                return ApiStabilitySafety::Recursive;
            }
        }
        if self.stack.len() >= API_STABILITY_SAFETY_MAX_DEPTH {
            return ApiStabilitySafety::Unknown;
        }
        self.stack.push(ApiStabilitySafetyApplication { symbol, args: args.clone() });
        let frame = Rc::new(ApiStabilitySafetyBinding {
            parent: bindings.cloned(),
            params,
            args: RefCell::new(args),
        });
        for default_node in defaults {
            let default_verdict =
                self.type_node(tp, Some(default_node), subst, Some(&frame), project);
            verdict = combine_safety(verdict, default_verdict);
        }
        let rhs = self.type_node(tp, declaration.type_node(), subst, Some(&frame), project);
        // Rust: Go's deferred stack pop.
        self.stack.pop();
        combine_safety(verdict, rhs)
    }

    // go: internal/typeparser/api_stability_safety.go:824 apiStabilitySafetyScan.typeValue
    // typeValue scans one represented type. The represented graph is the primary
    // carrier: it is followed through its concrete structure, and any type
    // parameter is resolved through the active substitution or binding frames.
    pub(crate) fn type_value(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: Option<P<Type>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        let Some(t) = t else {
            return ApiStabilitySafety::Safe;
        };
        if self.exceeded() {
            return ApiStabilitySafety::Unknown;
        }
        let t = non_distributed_parameter(t);
        let flags = t.flags();
        if flags.intersects(TypeFlags::TypeParameter) {
            if let Some(parameter) = self.mapped_parameter_value(tp, t, subst, bindings) {
                let Some(parameter) = parameter else {
                    return ApiStabilitySafety::Unknown;
                };
                if parameter == t {
                    return ApiStabilitySafety::Safe;
                }
                return self.type_value(tp, Some(parameter), subst, bindings, project);
            }
            return self.constraint_safety(tp, t, subst, bindings, project);
        }
        if flags.intersects(TypeFlags::UnionOrIntersection) {
            let mut verdict = ApiStabilitySafety::Safe;
            for &member in t.types() {
                let member_verdict = self.type_value(tp, Some(member), subst, bindings, project);
                verdict = combine_safety(verdict, member_verdict);
            }
            return verdict;
        }
        if flags.intersects(TypeFlags::Conditional) {
            return self.conditional_value(tp, t, subst, bindings, project);
        }
        if flags.intersects(TypeFlags::IndexedAccess) {
            let indexed = t.as_indexed_access_type();
            let verdict = self.type_value(tp, indexed.object_type(), subst, bindings, true);
            let index_verdict = self.type_value(tp, indexed.index_type(), subst, bindings, project);
            return combine_safety(verdict, index_verdict);
        }
        if flags.intersects(TypeFlags::Index) {
            return self.type_value(tp, t.as_index_type().target(), subst, bindings, true);
        }
        if flags.intersects(TypeFlags::TemplateLiteral) {
            let mut verdict = ApiStabilitySafety::Safe;
            for &part in t.as_template_literal_type().types() {
                let part_verdict = self.type_value(tp, Some(part), subst, bindings, project);
                verdict = combine_safety(verdict, part_verdict);
            }
            return verdict;
        }
        if flags.intersects(TypeFlags::StringMapping) {
            return self.type_value(
                tp,
                t.as_string_mapping_type().target(),
                subst,
                bindings,
                project,
            );
        }
        if flags.intersects(TypeFlags::Substitution) {
            let substitution = t.as_substitution_type();
            let verdict = self.type_value(tp, substitution.base_type(), subst, bindings, project);
            let constraint =
                self.type_value(tp, substitution.subst_constraint(), subst, bindings, project);
            return combine_safety(verdict, constraint);
        }
        if flags.intersects(TypeFlags::Object) {
            return self.object_value(tp, t, subst, bindings, project);
        }
        ApiStabilitySafety::Safe
    }

    // go: internal/typeparser/api_stability_safety.go:895 apiStabilitySafetyScan.mappedParameterValue
    // mappedParameterValue resolves a type parameter through the binding chain or
    // the active substitution frames. The boolean reports whether the parameter is
    // replaced.
    //
    // Rust: Go's `(*Type, bool)`: None is Go's false.
    #[expect(clippy::option_option, reason = "Go's (*Type, bool): the outer None is Go's false")]
    pub(crate) fn mapped_parameter_value(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: P<Type>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
    ) -> Option<Option<P<Type>>> {
        if let Some(argument) = safety_binding_lookup(bindings, Some(t)) {
            return Some(self.materialize_argument(tp, &argument));
        }
        if let Some((mapped, _)) = self.a.substitute(tp, subst, Some(t)) {
            return Some(mapped);
        }
        None
    }

    // go: internal/typeparser/api_stability_safety.go:911 apiStabilitySafetyScan.materializeArgument
    // materializeArgument returns the represented type of a captured application
    // argument without performing the guarded resolution: a represented argument is
    // resolved through its own environment, a primitive or literal argument is read
    // from the checker intrinsics, and a type parameter argument is followed
    // through its outer binding. Anything else stays nil.
    pub(crate) fn materialize_argument(
        &mut self,
        tp: &mut TypeParser<'_>,
        argument: &ApiStabilitySafetyArg,
    ) -> Option<P<Type>> {
        if let Some(argument_type) = argument.t {
            if !argument_type.flags().intersects(TypeFlags::TypeParameter) {
                return Some(argument_type);
            }
            if self.materializing.contains(&argument_type) {
                return None;
            }
            self.materializing.insert(argument_type);
            let mapped = self.mapped_parameter_value(
                tp,
                argument_type,
                &argument.subst,
                argument.bindings.as_ref(),
            );
            self.materializing.remove(&argument_type);
            if let Some(mapped) = mapped {
                return mapped;
            }
            return Some(argument_type);
        }
        let node = argument.node?;
        if let Some(represented) = get_resolved_type_from_type_node(tp.checker, node) {
            if let Some(mapped) = self.mapped_parameter_value(
                tp,
                represented,
                &argument.subst,
                argument.bindings.as_ref(),
            ) {
                return mapped;
            }
            return Some(represented);
        }
        if let Some(intrinsic) = api_stability_intrinsic_type_of_node(tp.checker, Some(node)) {
            return Some(intrinsic);
        }
        if node.kind() == Kind::TypeReference {
            let reference = node.as_type_reference_node();
            let symbol = self.a.symbol_at_type_name_node(tp, Some(reference.type_name()));
            if let Some(symbol) = symbol
                && symbol.flags().intersects(SymbolFlags::TypeParameter)
            {
                self.a.note_materializing_symbol_read(
                    ApiStabilityMaterializationReadKind::DeclaredType,
                    Some(symbol),
                );
                let parameter = tp.checker.get_declared_type_of_symbol_exported(symbol);
                if self.materializing.contains(&parameter) {
                    return None;
                }
                self.materializing.insert(parameter);
                let mapped = self.mapped_parameter_value(
                    tp,
                    parameter,
                    &argument.subst,
                    argument.bindings.as_ref(),
                );
                self.materializing.remove(&parameter);
                if let Some(mapped) = mapped {
                    return mapped;
                }
            }
        }
        // A concrete argument annotation (a reference to a declaration, an
        // application, a literal) is only resolved when the guard establishes that
        // its own resolution is bounded. The nested verification shares this
        // analysis's work counter and never recurses into the same argument.
        if self.materializing_nodes.contains(&node) {
            return None;
        }
        self.materializing_nodes.insert(node);
        let relation_operands = self.relation_operands;
        let mut nested = self.a.new_safety_scan();
        nested.relation_operands = relation_operands;
        let safe =
            nested.type_node(tp, Some(node), &argument.subst, argument.bindings.as_ref(), false)
                == ApiStabilitySafety::Safe;
        self.materializing_nodes.remove(&node);
        if safe {
            self.a.note_materializing_node_read(Some(node));
            return Some(tp.checker.get_type_from_type_node_exported(node));
        }
        None
    }
}

// go: internal/typeparser/api_stability_safety.go:989 apiStabilityIntrinsicTypeOfNode
// apiStabilityIntrinsicTypeOfNode returns the checker intrinsic for a primitive
// keyword annotation. The mapping is a direct field read; no annotation is
// resolved.
pub(crate) fn api_stability_intrinsic_type_of_node(
    c: &Checker,
    node: Option<P<Node>>,
) -> Option<P<Type>> {
    let node = node?;
    match node.kind() {
        Kind::AnyKeyword => Some(c.get_any_type()),
        Kind::UnknownKeyword => Some(c.get_unknown_type()),
        Kind::NeverKeyword => Some(c.get_never_type()),
        Kind::VoidKeyword => Some(c.get_void_type()),
        Kind::UndefinedKeyword => Some(c.get_undefined_type()),
        Kind::NullKeyword => Some(c.get_null_type()),
        Kind::StringKeyword => Some(c.get_string_type()),
        Kind::NumberKeyword => Some(c.get_number_type()),
        Kind::BigIntKeyword => Some(c.get_big_int_type()),
        Kind::BooleanKeyword => Some(c.get_boolean_type()),
        Kind::SymbolKeyword => Some(c.get_es_symbol_type()),
        Kind::ObjectKeyword => Some(get_non_primitive_type(c)),
        _ => None,
    }
}

impl ApiStabilitySafetyScan<'_> {
    // go: internal/typeparser/api_stability_safety.go:1026 apiStabilitySafetyScan.constraintSafety
    // constraintSafety follows the constraint of an unbound type parameter when the
    // guarded read may resolve it (an apparent-type request on a type parameter). A
    // relation-operand scan refuses an unbound parameter instead: the relation may
    // instantiate it against the other operand.
    pub(crate) fn constraint_safety(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: P<Type>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        if self.relation_operands {
            return ApiStabilitySafety::Unknown;
        }
        let Some(annotation) = type_parameter_annotation_node(Some(t), true) else {
            return ApiStabilitySafety::Safe;
        };
        self.type_node(tp, Some(annotation), subst, bindings, project)
    }

    // go: internal/typeparser/api_stability_safety.go:1038 apiStabilitySafetyScan.objectValue
    // objectValue scans a represented object type.
    pub(crate) fn object_value(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: P<Type>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        let flags = t.object_flags();
        if flags.intersects(ObjectFlags::ReverseMapped | ObjectFlags::EvolvingArray) {
            return ApiStabilitySafety::Safe;
        }
        if flags.intersects(ObjectFlags::Mapped) {
            return self.mapped_value(tp, t, subst, bindings, project);
        }
        if flags.intersects(ObjectFlags::Reference)
            && let Some(target) = t.target()
            && target != t
        {
            let arguments = get_resolved_type_arguments(Some(t));
            let mut verdict = ApiStabilitySafety::Safe;
            for &argument in arguments {
                let argument_verdict =
                    self.type_value(tp, Some(argument), subst, bindings, project);
                verdict = combine_safety(verdict, argument_verdict);
            }
            if !project {
                return verdict;
            }
            if self.relation_operands {
                // Bind the reference's own represented arguments so a member
                // annotation that mentions the target's type parameters resolves to
                // the concrete argument the relation will compare.
                let members = self.declaration_members_of_represented_arguments(
                    tp,
                    Some(target),
                    arguments,
                    subst,
                    bindings,
                    true,
                );
                return combine_safety(verdict, members);
            }
            let members =
                self.declaration_members(tp, target.symbol(), None, subst, bindings, true);
            return combine_safety(verdict, members);
        }
        let Some(symbol) = t.symbol() else {
            return ApiStabilitySafety::Safe;
        };
        self.declaration_structure(tp, Some(symbol), subst, bindings, project)
    }

    // go: internal/typeparser/api_stability_safety.go:1073 apiStabilitySafetyScan.mappedValue
    // mappedValue scans a represented mapped type through its represented mapped
    // components.
    pub(crate) fn mapped_value(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: P<Type>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        let mut verdict =
            self.type_value(tp, get_mapped_type_constraint_type(t), subst, bindings, project);
        let name = self.type_value(tp, get_mapped_type_name_type(t), subst, bindings, project);
        verdict = combine_safety(verdict, name);
        let template =
            self.type_value(tp, get_mapped_type_template_type(t), subst, bindings, project);
        combine_safety(verdict, template)
    }

    // go: internal/typeparser/api_stability_safety.go:1089 apiStabilitySafetyScan.conditionalValue
    // conditionalValue scans a represented conditional type. A deferred
    // conditional whose operands still mention an unbound parameter is not
    // evaluated by the guarded read. A concrete conditional must be resolved: the
    // guard selects the branch with the compiler's own relation instead of
    // guessing, and refuses to evaluate a conditional whose outcome the relation
    // cannot establish without inference.
    pub(crate) fn conditional_value(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: P<Type>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        let conditional = t.as_conditional_type();
        let check = conditional.check_type();
        let extends = conditional.extends_type();
        let check_safe = self.type_value(tp, check, subst, bindings, project);
        let extends_safe = self.type_value(tp, extends, subst, bindings, project);
        if check_safe != ApiStabilitySafety::Safe || extends_safe != ApiStabilitySafety::Safe {
            return combine_safety(check_safe, extends_safe);
        }
        if self.contains_unbound_parameter(tp, check, subst, bindings, &mut FxHashSet::default())
            || self.contains_unbound_parameter(
                tp,
                extends,
                subst,
                bindings,
                &mut FxHashSet::default(),
            )
        {
            return ApiStabilitySafety::Safe;
        }
        let selection = self.select_conditional_branch(tp, check, extends, subst, bindings);
        if selection == BranchSelection::Unknown {
            return ApiStabilitySafety::Unknown;
        }
        let mut verdict = ApiStabilitySafety::Safe;
        if selection.contains(BranchSelection::True) {
            let branch = self.conditional_branch_value(tp, t, true, subst, bindings, project);
            verdict = combine_safety(verdict, branch);
        }
        if selection.contains(BranchSelection::False) {
            let branch = self.conditional_branch_value(tp, t, false, subst, bindings, project);
            verdict = combine_safety(verdict, branch);
        }
        verdict
    }

    // go: internal/typeparser/api_stability_safety.go:1121 apiStabilitySafetyScan.conditionalBranchValue
    // conditionalBranchValue scans one represented branch of a conditional. The
    // materialized branch is preferred; otherwise the declared branch annotation is
    // scanned with the conditional's own mapper bindings.
    pub(crate) fn conditional_branch_value(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: P<Type>,
        true_branch: bool,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        if let Some(represented) = get_resolved_conditional_type_branch(Some(t), true_branch) {
            return self.type_value(tp, Some(represented), subst, bindings, project);
        }
        let Some(node) = get_conditional_type_branch_node(Some(t), true_branch) else {
            return ApiStabilitySafety::Unknown;
        };
        self.type_node(tp, Some(node), subst, bindings, project)
    }

    // go: internal/typeparser/api_stability_safety.go:1138 apiStabilitySafetyScan.conditionalNode
    // conditionalNode scans a raw conditional annotation. Operands that mention an
    // unbound parameter keep the conditional deferred and safe: constructing a
    // deferred conditional never evaluates its branches, even when an infer binder
    // appears. A concrete conditional is resolved through the compiler relation; an
    // infer binder in its extends type has an inferred outcome the relation cannot
    // establish without instantiating the application, so the guard refuses it.
    pub(crate) fn conditional_node(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: P<Node>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        let conditional = node.as_conditional_type_node();
        let check_safe =
            self.type_node(tp, Some(conditional.check_type()), subst, bindings, project);
        let extends_safe =
            self.type_node(tp, Some(conditional.extends_type()), subst, bindings, project);
        if check_safe != ApiStabilitySafety::Safe || extends_safe != ApiStabilitySafety::Safe {
            return combine_safety(check_safe, extends_safe);
        }
        if self.type_node_mentions_unbound_parameter(
            tp,
            Some(conditional.check_type()),
            subst,
            bindings,
            &mut FxHashSet::default(),
        ) || self.type_node_mentions_unbound_parameter(
            tp,
            Some(conditional.extends_type()),
            subst,
            bindings,
            &mut FxHashSet::default(),
        ) {
            return ApiStabilitySafety::Safe;
        }
        if self
            .type_node_mentions_infer(Some(conditional.extends_type()), &mut FxHashSet::default())
        {
            return ApiStabilitySafety::Unknown;
        }
        let check = self.operand_type(tp, Some(conditional.check_type()), subst, bindings);
        let extends = self.operand_type(tp, Some(conditional.extends_type()), subst, bindings);
        let selection = self.select_conditional_branch(tp, check, extends, subst, bindings);
        if selection == BranchSelection::Unknown {
            return ApiStabilitySafety::Unknown;
        }
        let mut verdict = ApiStabilitySafety::Safe;
        if selection.contains(BranchSelection::True) {
            let branch =
                self.type_node(tp, Some(conditional.true_type()), subst, bindings, project);
            verdict = combine_safety(verdict, branch);
        }
        if selection.contains(BranchSelection::False) {
            let branch =
                self.type_node(tp, Some(conditional.false_type()), subst, bindings, project);
            verdict = combine_safety(verdict, branch);
        }
        verdict
    }
}

// go: internal/typeparser/api_stability_safety.go:1173 branchSelection
// branchSelection is a bit set of conditional branches the compiler relation
// evaluates.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub(crate) struct BranchSelection(pub(crate) i64);

// go: internal/typeparser/api_stability_safety.go:1175 branchUnknown
impl BranchSelection {
    pub(crate) const Unknown: BranchSelection = BranchSelection(1 << 0);
    pub(crate) const True: BranchSelection = BranchSelection(1 << 1);
    pub(crate) const False: BranchSelection = BranchSelection(1 << 2);

    /// Rust-only: Go's `selection&branch != 0`.
    pub(crate) fn contains(self, branch: BranchSelection) -> bool {
        self.0 & branch.0 != 0
    }
}

impl ApiStabilitySafetyScan<'_> {
    // go: internal/typeparser/api_stability_safety.go:1189 apiStabilitySafetyScan.selectConditionalBranch
    // selectConditionalBranch asks the compiler relation which branch a concrete
    // conditional evaluates. The check type distributes over unions, so a mixed
    // distribution evaluates both branches. The relation is only asked when both
    // operands are fully represented and free of type parameters the active
    // environment replaces, and when a relation-operand pre-flight establishes that
    // resolving their full projected surfaces is bounded. The guard never
    // instantiates a branch to answer the question and never evaluates an
    // unresolved compound substitution.
    pub(crate) fn select_conditional_branch(
        &mut self,
        tp: &mut TypeParser<'_>,
        check: Option<P<Type>>,
        extends: Option<P<Type>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
    ) -> BranchSelection {
        let (Some(check), Some(extends)) = (check, extends) else {
            return BranchSelection::Unknown;
        };
        if self.exceeded() {
            return BranchSelection::Unknown;
        }
        if tp.checker.get_any_type() == check {
            // `any extends X ? A : B` evaluates to `A | B`.
            return BranchSelection(BranchSelection::True.0 | BranchSelection::False.0);
        }
        if check.flags().intersects(TypeFlags::Never) {
            // `never` is assignable to every target, and a distributive check over
            // `never` evaluates no branch at all. Scanning only the true branch is
            // sound for both outcomes and never authorizes the false branch.
            return BranchSelection::True;
        }
        if check == extends {
            // Identical represented types are assignable without evaluating any
            // member surface.
            return BranchSelection::True;
        }
        if extends == tp.checker.get_any_type() || extends == tp.checker.get_unknown_type() {
            // Every type is assignable to `any` and to `unknown`.
            return BranchSelection::True;
        }
        if self.contains_replaced_parameter(
            tp,
            Some(check),
            subst,
            bindings,
            &mut FxHashSet::default(),
        ) || self.contains_replaced_parameter(
            tp,
            Some(extends),
            subst,
            bindings,
            &mut FxHashSet::default(),
        ) {
            // An operand still mentions a type parameter the active environment
            // replaces. The checker would instantiate the compound operand before
            // relating it; the guard never authorizes evaluating an unresolved
            // compound substitution.
            return BranchSelection::Unknown;
        }
        // The relation compares the full projected member surfaces of both operands:
        // property types, call and construct signature returns and index infos are
        // all resolved while relating. Unless every one of those components is
        // itself established as bounded by a relation-operand scan, the relation
        // must not be asked, because it would expand them eagerly.
        {
            let mut operands = self.a.new_safety_scan();
            operands.relation_operands = true;
            if operands.type_value(tp, Some(check), subst, bindings, true)
                != ApiStabilitySafety::Safe
                || operands.type_value(tp, Some(extends), subst, bindings, true)
                    != ApiStabilitySafety::Safe
            {
                return BranchSelection::Unknown;
            }
        }
        let mut check_members = vec![check];
        if check.flags().intersects(TypeFlags::Union) {
            check_members = check.types().to_vec();
        }
        let mut selection = BranchSelection(0);
        for member in check_members {
            if tp.checker.is_type_assignable_to(member, extends) {
                selection.0 |= BranchSelection::True.0;
            } else {
                selection.0 |= BranchSelection::False.0;
            }
        }
        selection
    }

    // go: internal/typeparser/api_stability_safety.go:1255 apiStabilitySafetyScan.operandType
    // operandType returns the represented type of a concrete conditional operand,
    // following only a top-level parameter reference through the active
    // environment. A compound operand that still mentions a substituted parameter
    // keeps that parameter and is refused by the relation-operand pre-flight; the
    // guard never instantiates a compound substitution itself.
    pub(crate) fn operand_type(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Option<P<Node>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
    ) -> Option<P<Type>> {
        let node = node?;
        let resolved = get_resolved_type_from_type_node(tp.checker, node);
        let represented = if let Some(represented) = resolved {
            represented
        } else {
            self.a.note_materializing_node_read(Some(node));
            tp.checker.get_type_from_type_node_exported(node)
        };
        if let Some(mapped) = self.mapped_parameter_value(tp, represented, subst, bindings) {
            return mapped;
        }
        Some(represented)
    }

    // go: internal/typeparser/api_stability_safety.go:1275 apiStabilitySafetyScan.containsUnboundParameter
    // containsUnboundParameter reports whether a represented type still mentions a
    // type parameter that no active frame replaces.
    pub(crate) fn contains_unbound_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: Option<P<Type>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        active: &mut FxHashSet<P<Type>>,
    ) -> bool {
        let Some(t) = t else {
            return false;
        };
        if active.contains(&t) {
            return false;
        }
        active.insert(t);
        let result = self.contains_unbound_parameter_inner(tp, t, subst, bindings, active);
        // Rust: Go's `defer delete(active, t)` (the key is the type before normalization).
        active.remove(&t);
        result
    }

    // go: internal/typeparser/api_stability_safety.go:1275 apiStabilitySafetyScan.containsUnboundParameter
    // Rust: the body after the cycle guard, split out so the deferred release runs after it.
    fn contains_unbound_parameter_inner(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: P<Type>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        active: &mut FxHashSet<P<Type>>,
    ) -> bool {
        let t = non_distributed_parameter(t);
        let flags = t.flags();
        if flags.intersects(TypeFlags::TypeParameter) {
            if let Some(parameter) = self.mapped_parameter_value(tp, t, subst, bindings) {
                let Some(parameter) = parameter else {
                    return true;
                };
                if parameter == t {
                    return false;
                }
                return self.contains_unbound_parameter(
                    tp,
                    Some(parameter),
                    subst,
                    bindings,
                    active,
                );
            }
            return true;
        }
        if flags.intersects(TypeFlags::UnionOrIntersection) {
            for &member in t.types() {
                if self.contains_unbound_parameter(tp, Some(member), subst, bindings, active) {
                    return true;
                }
            }
        } else if flags.intersects(TypeFlags::Object)
            && t.object_flags().intersects(ObjectFlags::Reference)
        {
            for &argument in get_resolved_type_arguments(Some(t)) {
                if self.contains_unbound_parameter(tp, Some(argument), subst, bindings, active) {
                    return true;
                }
            }
        } else if flags.intersects(TypeFlags::Conditional) {
            let conditional = t.as_conditional_type();
            return self.contains_unbound_parameter(
                tp,
                conditional.check_type(),
                subst,
                bindings,
                active,
            ) || self.contains_unbound_parameter(
                tp,
                conditional.extends_type(),
                subst,
                bindings,
                active,
            );
        } else if flags.intersects(TypeFlags::IndexedAccess) {
            let indexed = t.as_indexed_access_type();
            return self.contains_unbound_parameter(
                tp,
                indexed.object_type(),
                subst,
                bindings,
                active,
            ) || self.contains_unbound_parameter(
                tp,
                indexed.index_type(),
                subst,
                bindings,
                active,
            );
        } else if flags.intersects(TypeFlags::Index) {
            return self.contains_unbound_parameter(
                tp,
                t.as_index_type().target(),
                subst,
                bindings,
                active,
            );
        } else if flags.intersects(TypeFlags::TemplateLiteral) {
            for &part in t.as_template_literal_type().types() {
                if self.contains_unbound_parameter(tp, Some(part), subst, bindings, active) {
                    return true;
                }
            }
        }
        false
    }

    // go: internal/typeparser/api_stability_safety.go:1347 apiStabilitySafetyScan.containsReplacedParameter
    // containsReplacedParameter reports whether a represented type still mentions a
    // type parameter the active environment replaces. The compiler would
    // instantiate such a compound operand before evaluating a relation over it, so
    // the guard treats it as unknown instead of guessing a substitution. Reference
    // arguments are followed directly; an anonymous compound the checker has not
    // instantiated is inspected through its raw declarations, because its
    // unsubstituted representation does not show the parameter its resolved members
    // would carry.
    pub(crate) fn contains_replaced_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: Option<P<Type>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        active: &mut FxHashSet<P<Type>>,
    ) -> bool {
        let Some(t) = t else {
            return false;
        };
        if active.contains(&t) {
            return false;
        }
        active.insert(t);
        let result = self.contains_replaced_parameter_inner(tp, t, subst, bindings, active);
        // Rust: Go's `defer delete(active, t)` (the key is the type before normalization).
        active.remove(&t);
        result
    }

    // go: internal/typeparser/api_stability_safety.go:1347 apiStabilitySafetyScan.containsReplacedParameter
    // Rust: the body after the cycle guard, split out so the deferred release runs after it.
    fn contains_replaced_parameter_inner(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: P<Type>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        active: &mut FxHashSet<P<Type>>,
    ) -> bool {
        let t = non_distributed_parameter(t);
        let flags = t.flags();
        if flags.intersects(TypeFlags::TypeParameter) {
            return self.mapped_parameter_value(tp, t, subst, bindings).is_some();
        }
        if flags.intersects(TypeFlags::UnionOrIntersection) {
            for &member in t.types() {
                if self.contains_replaced_parameter(tp, Some(member), subst, bindings, active) {
                    return true;
                }
            }
        } else if flags.intersects(TypeFlags::Object) {
            if t.object_flags().intersects(ObjectFlags::Reference) {
                for &argument in get_resolved_type_arguments(Some(t)) {
                    if self.contains_replaced_parameter(tp, Some(argument), subst, bindings, active)
                    {
                        return true;
                    }
                }
                return false;
            }
            if t.object_flags().intersects(ObjectFlags::Instantiated) {
                // The represented instantiation substitutes its members through
                // its own mapper; the relation-operand scan refuses any raw member
                // parameter it cannot bind, so no additional raw detection applies.
                return false;
            }
            // An anonymous compound the checker has not instantiated still
            // contains its raw member annotations. A replaced parameter inside one
            // of them is instantiated before the relation compares the operand,
            // which the represented structure does not show, so the operand is
            // refused.
            return self.raw_declarations_mention_replaced_parameter(
                tp,
                t.symbol(),
                subst,
                bindings,
            );
        } else if flags.intersects(TypeFlags::Conditional) {
            let conditional = t.as_conditional_type();
            return self.contains_replaced_parameter(
                tp,
                conditional.check_type(),
                subst,
                bindings,
                active,
            ) || self.contains_replaced_parameter(
                tp,
                conditional.extends_type(),
                subst,
                bindings,
                active,
            );
        } else if flags.intersects(TypeFlags::IndexedAccess) {
            let indexed = t.as_indexed_access_type();
            return self.contains_replaced_parameter(
                tp,
                indexed.object_type(),
                subst,
                bindings,
                active,
            ) || self.contains_replaced_parameter(
                tp,
                indexed.index_type(),
                subst,
                bindings,
                active,
            );
        } else if flags.intersects(TypeFlags::Index) {
            return self.contains_replaced_parameter(
                tp,
                t.as_index_type().target(),
                subst,
                bindings,
                active,
            );
        } else if flags.intersects(TypeFlags::TemplateLiteral) {
            for &part in t.as_template_literal_type().types() {
                if self.contains_replaced_parameter(tp, Some(part), subst, bindings, active) {
                    return true;
                }
            }
        } else if flags.intersects(TypeFlags::Substitution) {
            let substitution = t.as_substitution_type();
            return self.contains_replaced_parameter(
                tp,
                substitution.base_type(),
                subst,
                bindings,
                active,
            ) || self.contains_replaced_parameter(
                tp,
                substitution.subst_constraint(),
                subst,
                bindings,
                active,
            );
        }
        false
    }

    // go: internal/typeparser/api_stability_safety.go:1429 apiStabilitySafetyScan.typeNodeMentionsUnboundParameter
    // typeNodeMentionsUnboundParameter reports whether a raw annotation mentions a
    // type parameter no active frame replaces. An infer binder is bound by its own
    // conditional, and a callable's own type parameters are bound by the callable,
    // so neither makes the surrounding conditional deferred. A conditional whose
    // operands still mention an unbound parameter is deferred and never evaluated
    // by an ordinary read.
    pub(crate) fn type_node_mentions_unbound_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Option<P<Node>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        visiting: &mut FxHashSet<P<Node>>,
    ) -> bool {
        self.type_node_mentions_unbound_parameter_scoped(
            tp,
            node,
            subst,
            bindings,
            visiting,
            &FxHashSet::default(),
        )
    }

    // go: internal/typeparser/api_stability_safety.go:1436 apiStabilitySafetyScan.typeNodeMentionsUnboundParameterScoped
    // typeNodeMentionsUnboundParameterScoped carries the type parameters the
    // enclosing callables bind so a reference to one of them is not mistaken for an
    // unbound parameter of the surrounding conditional.
    pub(crate) fn type_node_mentions_unbound_parameter_scoped(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Option<P<Node>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        visiting: &mut FxHashSet<P<Node>>,
        scope: &FxHashSet<P<Type>>,
    ) -> bool {
        let Some(node) = node else {
            return false;
        };
        if visiting.contains(&node) {
            return false;
        }
        visiting.insert(node);
        let result = self.type_node_mentions_unbound_parameter_scoped_inner(
            tp, node, subst, bindings, visiting, scope,
        );
        // Rust: Go's `defer delete(visiting, node)`.
        visiting.remove(&node);
        result
    }

    // go: internal/typeparser/api_stability_safety.go:1436 apiStabilitySafetyScan.typeNodeMentionsUnboundParameterScoped
    // Rust: the body after the cycle guard, split out so the deferred release runs after it.
    fn type_node_mentions_unbound_parameter_scoped_inner(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: P<Node>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        visiting: &mut FxHashSet<P<Node>>,
        scope: &FxHashSet<P<Type>>,
    ) -> bool {
        match node.kind() {
            Kind::InferType => false,
            Kind::TypeReference => {
                let reference = node.as_type_reference_node();
                let symbol = self.a.symbol_at_type_name_node(tp, Some(reference.type_name()));
                if let Some(symbol) = symbol
                    && symbol.flags().intersects(SymbolFlags::TypeParameter)
                {
                    self.a.note_materializing_symbol_read(
                        ApiStabilityMaterializationReadKind::DeclaredType,
                        Some(symbol),
                    );
                    let parameter = tp.checker.get_declared_type_of_symbol_exported(symbol);
                    if !scope.contains(&parameter)
                        && self.mapped_parameter_value(tp, parameter, subst, bindings).is_none()
                    {
                        return true;
                    }
                }
                if let Some(arguments) = reference.type_arguments() {
                    for &argument in arguments.nodes() {
                        if self.type_node_mentions_unbound_parameter_scoped(
                            tp,
                            Some(argument),
                            subst,
                            bindings,
                            visiting,
                            scope,
                        ) {
                            return true;
                        }
                    }
                }
                false
            }
            Kind::ParenthesizedType => self.type_node_mentions_unbound_parameter_scoped(
                tp,
                Some(node.as_parenthesized_type_node().type_()),
                subst,
                bindings,
                visiting,
                scope,
            ),
            Kind::ArrayType => self.type_node_mentions_unbound_parameter_scoped(
                tp,
                Some(node.as_array_type_node().element_type()),
                subst,
                bindings,
                visiting,
                scope,
            ),
            Kind::OptionalType => self.type_node_mentions_unbound_parameter_scoped(
                tp,
                Some(node.as_optional_type_node().type_()),
                subst,
                bindings,
                visiting,
                scope,
            ),
            Kind::RestType => self.type_node_mentions_unbound_parameter_scoped(
                tp,
                Some(node.as_rest_type_node().type_()),
                subst,
                bindings,
                visiting,
                scope,
            ),
            Kind::TypeOperator => self.type_node_mentions_unbound_parameter_scoped(
                tp,
                Some(node.as_type_operator_node().type_()),
                subst,
                bindings,
                visiting,
                scope,
            ),
            Kind::NamedTupleMember => self.type_node_mentions_unbound_parameter_scoped(
                tp,
                Some(node.as_named_tuple_member().type_()),
                subst,
                bindings,
                visiting,
                scope,
            ),
            Kind::TupleType => {
                for &element in node.as_tuple_type_node().elements().nodes() {
                    if self.type_node_mentions_unbound_parameter_scoped(
                        tp,
                        Some(element),
                        subst,
                        bindings,
                        visiting,
                        scope,
                    ) {
                        return true;
                    }
                }
                false
            }
            Kind::UnionType => {
                for &member in node.as_union_type_node().types().nodes() {
                    if self.type_node_mentions_unbound_parameter_scoped(
                        tp,
                        Some(member),
                        subst,
                        bindings,
                        visiting,
                        scope,
                    ) {
                        return true;
                    }
                }
                false
            }
            Kind::IntersectionType => {
                for &member in node.as_intersection_type_node().types().nodes() {
                    if self.type_node_mentions_unbound_parameter_scoped(
                        tp,
                        Some(member),
                        subst,
                        bindings,
                        visiting,
                        scope,
                    ) {
                        return true;
                    }
                }
                false
            }
            Kind::TemplateLiteralType => {
                for &span in node.as_template_literal_type_node().template_spans().nodes() {
                    if span.kind() != Kind::TemplateLiteralTypeSpan {
                        continue;
                    }
                    if self.type_node_mentions_unbound_parameter_scoped(
                        tp,
                        Some(span.as_template_literal_type_span().type_()),
                        subst,
                        bindings,
                        visiting,
                        scope,
                    ) {
                        return true;
                    }
                }
                false
            }
            Kind::IndexedAccessType => {
                let indexed = node.as_indexed_access_type_node();
                self.type_node_mentions_unbound_parameter_scoped(
                    tp,
                    Some(indexed.object_type()),
                    subst,
                    bindings,
                    visiting,
                    scope,
                ) || self.type_node_mentions_unbound_parameter_scoped(
                    tp,
                    Some(indexed.index_type()),
                    subst,
                    bindings,
                    visiting,
                    scope,
                )
            }
            Kind::ConditionalType => {
                let conditional = node.as_conditional_type_node();
                self.type_node_mentions_unbound_parameter_scoped(
                    tp,
                    Some(conditional.check_type()),
                    subst,
                    bindings,
                    visiting,
                    scope,
                ) || self.type_node_mentions_unbound_parameter_scoped(
                    tp,
                    Some(conditional.extends_type()),
                    subst,
                    bindings,
                    visiting,
                    scope,
                )
            }
            Kind::TypeLiteral => {
                for &member in node.members() {
                    if let Some(function_like) = member.function_like_data() {
                        if self.function_like_mentions_unbound_parameter(
                            tp,
                            Some(function_like),
                            subst,
                            bindings,
                            visiting,
                            scope,
                        ) {
                            return true;
                        }
                        continue;
                    }
                    if self.type_node_mentions_unbound_parameter_scoped(
                        tp,
                        Some(member),
                        subst,
                        bindings,
                        visiting,
                        scope,
                    ) {
                        return true;
                    }
                }
                false
            }
            Kind::PropertySignature => self.type_node_mentions_unbound_parameter_scoped(
                tp,
                node.as_property_signature_declaration().type_(),
                subst,
                bindings,
                visiting,
                scope,
            ),
            Kind::PropertyDeclaration => self.type_node_mentions_unbound_parameter_scoped(
                tp,
                node.as_property_declaration().type_(),
                subst,
                bindings,
                visiting,
                scope,
            ),
            Kind::IndexSignature => {
                let declaration = node.as_index_signature_declaration();
                if let Some(parameters) = declaration.parameters()
                    && parameters.nodes().len() == 1
                    && parameters.nodes()[0].kind() == Kind::Parameter
                    && self.type_node_mentions_unbound_parameter_scoped(
                        tp,
                        parameters.nodes()[0].as_parameter_declaration().type_(),
                        subst,
                        bindings,
                        visiting,
                        scope,
                    )
                {
                    return true;
                }
                self.type_node_mentions_unbound_parameter_scoped(
                    tp,
                    declaration.type_(),
                    subst,
                    bindings,
                    visiting,
                    scope,
                )
            }
            Kind::MappedType => {
                let mapped = node.as_mapped_type_node();
                if self.type_node_mentions_unbound_parameter_scoped(
                    tp,
                    mapped.name_type(),
                    subst,
                    bindings,
                    visiting,
                    scope,
                ) || self.type_node_mentions_unbound_parameter_scoped(
                    tp,
                    mapped.type_(),
                    subst,
                    bindings,
                    visiting,
                    scope,
                ) {
                    return true;
                }
                self.type_node_mentions_unbound_parameter_scoped(
                    tp,
                    mapped.type_parameter().as_type_parameter_declaration().constraint(),
                    subst,
                    bindings,
                    visiting,
                    scope,
                )
            }
            Kind::FunctionType | Kind::ConstructorType => {
                if let Some(function_like) = node.function_like_data() {
                    return self.function_like_mentions_unbound_parameter(
                        tp,
                        Some(function_like),
                        subst,
                        bindings,
                        visiting,
                        scope,
                    );
                }
                false
            }
            _ => false,
        }
    }

    // go: internal/typeparser/api_stability_safety.go:1576 apiStabilitySafetyScan.functionLikeMentionsUnboundParameter
    // functionLikeMentionsUnboundParameter walks the constraints, parameter and
    // return annotations of a function-like declaration looking for an unbound
    // parameter. The declaration's own type parameters are bound by the callable
    // itself and are excluded through the scope, so a generic operand is not
    // mistaken for a deferred one.
    pub(crate) fn function_like_mentions_unbound_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        function_like: Option<&FunctionLikeBase>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        visiting: &mut FxHashSet<P<Node>>,
        scope: &FxHashSet<P<Type>>,
    ) -> bool {
        let Some(function_like) = function_like else {
            return false;
        };
        let mut own_scope = None;
        if let Some(type_parameters) = function_like.type_parameters() {
            let mut own: FxHashSet<P<Type>> = scope.clone();
            for &type_parameter in type_parameters.nodes() {
                if type_parameter.kind() != Kind::TypeParameter {
                    continue;
                }
                if let Some(name) = ets_shim::ast::get_name_of_declaration(type_parameter)
                    && let Some(symbol) = self.a.symbol_at_type_name_node(tp, Some(name))
                {
                    // The reference symbol of a type parameter can be a
                    // distinct instance; the declared type is the shared
                    // identity, so it is the scope key.
                    self.a.note_materializing_symbol_read(
                        ApiStabilityMaterializationReadKind::DeclaredType,
                        Some(symbol),
                    );
                    own.insert(tp.checker.get_declared_type_of_symbol_exported(symbol));
                }
            }
            for &type_parameter in type_parameters.nodes() {
                if type_parameter.kind() != Kind::TypeParameter {
                    continue;
                }
                let declaration = type_parameter.as_type_parameter_declaration();
                // A constraint is evaluated in the callable's own scope, so an
                // earlier own type parameter referenced by a later constraint is
                // bound as well.
                if self.type_node_mentions_unbound_parameter_scoped(
                    tp,
                    declaration.constraint(),
                    subst,
                    bindings,
                    visiting,
                    &own,
                ) {
                    return true;
                }
            }
            own_scope = Some(own);
        }
        let own = own_scope.as_ref().unwrap_or(scope);
        for &parameter in function_like_parameter_nodes(Some(function_like)) {
            if parameter.kind() == Kind::Parameter
                && self.type_node_mentions_unbound_parameter_scoped(
                    tp,
                    parameter.as_parameter_declaration().type_(),
                    subst,
                    bindings,
                    visiting,
                    own,
                )
            {
                return true;
            }
        }
        function_like.type_().is_some()
            && self.type_node_mentions_unbound_parameter_scoped(
                tp,
                function_like.type_(),
                subst,
                bindings,
                visiting,
                own,
            )
    }

    // go: internal/typeparser/api_stability_safety.go:1629 apiStabilitySafetyScan.typeNodeMentionsInfer
    // typeNodeMentionsInfer reports whether an annotation mentions an infer binder.
    pub(crate) fn type_node_mentions_infer(
        &mut self,
        node: Option<P<Node>>,
        visiting: &mut FxHashSet<P<Node>>,
    ) -> bool {
        let Some(node) = node else {
            return false;
        };
        if visiting.contains(&node) {
            return false;
        }
        visiting.insert(node);
        let result = self.type_node_mentions_infer_inner(node, visiting);
        // Rust: Go's `defer delete(visiting, node)`.
        visiting.remove(&node);
        result
    }

    // go: internal/typeparser/api_stability_safety.go:1629 apiStabilitySafetyScan.typeNodeMentionsInfer
    // Rust: the body after the cycle guard, split out so the deferred release runs after it.
    fn type_node_mentions_infer_inner(
        &mut self,
        node: P<Node>,
        visiting: &mut FxHashSet<P<Node>>,
    ) -> bool {
        match node.kind() {
            Kind::InferType => true,
            Kind::ParenthesizedType => self.type_node_mentions_infer(
                Some(node.as_parenthesized_type_node().type_()),
                visiting,
            ),
            Kind::ArrayType => self
                .type_node_mentions_infer(Some(node.as_array_type_node().element_type()), visiting),
            Kind::OptionalType => {
                self.type_node_mentions_infer(Some(node.as_optional_type_node().type_()), visiting)
            }
            Kind::RestType => {
                self.type_node_mentions_infer(Some(node.as_rest_type_node().type_()), visiting)
            }
            Kind::TypeOperator => {
                self.type_node_mentions_infer(Some(node.as_type_operator_node().type_()), visiting)
            }
            Kind::NamedTupleMember => {
                self.type_node_mentions_infer(Some(node.as_named_tuple_member().type_()), visiting)
            }
            Kind::TupleType => {
                for &element in node.as_tuple_type_node().elements().nodes() {
                    if self.type_node_mentions_infer(Some(element), visiting) {
                        return true;
                    }
                }
                false
            }
            Kind::UnionType => {
                for &member in node.as_union_type_node().types().nodes() {
                    if self.type_node_mentions_infer(Some(member), visiting) {
                        return true;
                    }
                }
                false
            }
            Kind::IntersectionType => {
                for &member in node.as_intersection_type_node().types().nodes() {
                    if self.type_node_mentions_infer(Some(member), visiting) {
                        return true;
                    }
                }
                false
            }
            Kind::TemplateLiteralType => {
                for &span in node.as_template_literal_type_node().template_spans().nodes() {
                    if span.kind() == Kind::TemplateLiteralTypeSpan
                        && self.type_node_mentions_infer(
                            Some(span.as_template_literal_type_span().type_()),
                            visiting,
                        )
                    {
                        return true;
                    }
                }
                false
            }
            Kind::IndexedAccessType => {
                let indexed = node.as_indexed_access_type_node();
                self.type_node_mentions_infer(Some(indexed.object_type()), visiting)
                    || self.type_node_mentions_infer(Some(indexed.index_type()), visiting)
            }
            Kind::ConditionalType => {
                let conditional = node.as_conditional_type_node();
                self.type_node_mentions_infer(Some(conditional.check_type()), visiting)
                    || self.type_node_mentions_infer(Some(conditional.extends_type()), visiting)
                    || self.type_node_mentions_infer(Some(conditional.true_type()), visiting)
                    || self.type_node_mentions_infer(Some(conditional.false_type()), visiting)
            }
            Kind::TypeReference => {
                if let Some(arguments) = node.as_type_reference_node().type_arguments() {
                    for &argument in arguments.nodes() {
                        if self.type_node_mentions_infer(Some(argument), visiting) {
                            return true;
                        }
                    }
                }
                false
            }
            Kind::FunctionType | Kind::ConstructorType => {
                if let Some(function_like) = node.function_like_data() {
                    for &parameter in function_like_parameter_nodes(Some(function_like)) {
                        if parameter.kind() == Kind::Parameter
                            && self.type_node_mentions_infer(
                                parameter.as_parameter_declaration().type_(),
                                visiting,
                            )
                        {
                            return true;
                        }
                    }
                    if function_like.type_().is_some()
                        && self.type_node_mentions_infer(function_like.type_(), visiting)
                    {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    // go: internal/typeparser/api_stability_safety.go:1725 apiStabilitySafetyScan.rawDeclarationsMentionReplacedParameter
    // rawDeclarationsMentionReplacedParameter reports whether the raw declarations
    // of a symbol contain an annotation that mentions a type parameter the active
    // environment replaces. It is a non-evaluating declaration-structure walk used
    // to refuse a relation operand whose represented compound does not show the
    // substitution its resolved members would carry. A compound without a
    // declaration cannot be established as safe and is refused.
    pub(crate) fn raw_declarations_mention_replaced_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: Option<P<Symbol>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
    ) -> bool {
        let Some(symbol) = symbol else {
            return true;
        };
        if symbol.declarations().is_empty() {
            return true;
        }
        let mut visiting = FxHashSet::default();
        for &declaration in symbol.declarations() {
            if self.declaration_mentions_replaced_parameter(
                tp,
                Some(declaration),
                subst,
                bindings,
                &mut visiting,
            ) {
                return true;
            }
        }
        false
    }

    // go: internal/typeparser/api_stability_safety.go:1745 apiStabilitySafetyScan.declarationMentionsReplacedParameter
    // declarationMentionsReplacedParameter walks the public annotations of one raw
    // declaration: function-like parameters and returns, type literal and heritage
    // member annotations, index signatures and mapped components. It resolves
    // nothing.
    pub(crate) fn declaration_mentions_replaced_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        declaration: Option<P<Node>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        visiting: &mut FxHashSet<P<Node>>,
    ) -> bool {
        let Some(declaration) = declaration else {
            return false;
        };
        match declaration.kind() {
            Kind::InterfaceDeclaration
            | Kind::ClassDeclaration
            | Kind::ClassExpression
            | Kind::EnumDeclaration => {
                for &member in declaration.members() {
                    if self.type_node_mentions_replaced_parameter(
                        tp,
                        Some(member),
                        subst,
                        bindings,
                        visiting,
                    ) {
                        return true;
                    }
                }
                false
            }
            Kind::TypeAliasDeclaration | Kind::JSTypeAliasDeclaration => self
                .type_node_mentions_replaced_parameter(
                    tp,
                    declaration.type_node(),
                    subst,
                    bindings,
                    visiting,
                ),
            Kind::VariableDeclaration
            | Kind::PropertyDeclaration
            | Kind::PropertySignature
            | Kind::Parameter => self.type_node_mentions_replaced_parameter(
                tp,
                declaration_annotation_node_of(Some(declaration)),
                subst,
                bindings,
                visiting,
            ),
            _ => self.type_node_mentions_replaced_parameter(
                tp,
                Some(declaration),
                subst,
                bindings,
                visiting,
            ),
        }
    }

    // go: internal/typeparser/api_stability_safety.go:1768 apiStabilitySafetyScan.functionLikeMentionsReplacedParameter
    // functionLikeMentionsReplacedParameter walks the parameter, type parameter
    // constraint and return annotations of a function-like declaration.
    pub(crate) fn function_like_mentions_replaced_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        function_like: Option<&FunctionLikeBase>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        visiting: &mut FxHashSet<P<Node>>,
    ) -> bool {
        let Some(function_like) = function_like else {
            return false;
        };
        for &parameter in function_like_parameter_nodes(Some(function_like)) {
            if parameter.kind() != Kind::Parameter {
                continue;
            }
            if self.type_node_mentions_replaced_parameter(
                tp,
                parameter.as_parameter_declaration().type_(),
                subst,
                bindings,
                visiting,
            ) {
                return true;
            }
        }
        if let Some(type_parameters) = function_like.type_parameters() {
            for &type_parameter in type_parameters.nodes() {
                if type_parameter.kind() != Kind::TypeParameter {
                    continue;
                }
                if self.type_node_mentions_replaced_parameter(
                    tp,
                    type_parameter.as_type_parameter_declaration().constraint(),
                    subst,
                    bindings,
                    visiting,
                ) {
                    return true;
                }
            }
        }
        self.type_node_mentions_replaced_parameter(
            tp,
            function_like.type_(),
            subst,
            bindings,
            visiting,
        )
    }

    // go: internal/typeparser/api_stability_safety.go:1802 apiStabilitySafetyScan.typeNodeMentionsReplacedParameter
    // typeNodeMentionsReplacedParameter reports whether a raw annotation mentions a
    // type parameter the active environment replaces anywhere inside its structure.
    // It is a non-evaluating syntax gate: a conditional operand that still contains
    // such a parameter is instantiated by the checker before a relation compares
    // it, so the guard refuses to ask the relation instead of guessing the
    // substituted value. An `infer` binder is scoped by its own conditional and is
    // not an environment parameter.
    pub(crate) fn type_node_mentions_replaced_parameter(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: Option<P<Node>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        visiting: &mut FxHashSet<P<Node>>,
    ) -> bool {
        let Some(node) = node else {
            return false;
        };
        if visiting.contains(&node) {
            return false;
        }
        if let Some(function_like) = node.function_like_data() {
            return self.function_like_mentions_replaced_parameter(
                tp,
                Some(function_like),
                subst,
                bindings,
                visiting,
            );
        }
        visiting.insert(node);
        let result =
            self.type_node_mentions_replaced_parameter_inner(tp, node, subst, bindings, visiting);
        // Rust: Go's `defer delete(visiting, node)`.
        visiting.remove(&node);
        result
    }

    // go: internal/typeparser/api_stability_safety.go:1802 apiStabilitySafetyScan.typeNodeMentionsReplacedParameter
    // Rust: the body after the cycle guard, split out so the deferred release runs after it.
    fn type_node_mentions_replaced_parameter_inner(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: P<Node>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        visiting: &mut FxHashSet<P<Node>>,
    ) -> bool {
        match node.kind() {
            Kind::InferType => false,
            Kind::TypeReference => {
                let reference = node.as_type_reference_node();
                if let Some(symbol) =
                    self.a.symbol_at_type_name_node(tp, Some(reference.type_name()))
                    && symbol.flags().intersects(SymbolFlags::TypeParameter)
                {
                    self.a.note_materializing_symbol_read(
                        ApiStabilityMaterializationReadKind::DeclaredType,
                        Some(symbol),
                    );
                    let parameter = tp.checker.get_declared_type_of_symbol_exported(symbol);
                    if self.mapped_parameter_value(tp, parameter, subst, bindings).is_some() {
                        return true;
                    }
                }
                if let Some(arguments) = reference.type_arguments() {
                    for &argument in arguments.nodes() {
                        if self.type_node_mentions_replaced_parameter(
                            tp,
                            Some(argument),
                            subst,
                            bindings,
                            visiting,
                        ) {
                            return true;
                        }
                    }
                }
                false
            }
            Kind::ParenthesizedType => self.type_node_mentions_replaced_parameter(
                tp,
                Some(node.as_parenthesized_type_node().type_()),
                subst,
                bindings,
                visiting,
            ),
            Kind::ArrayType => self.type_node_mentions_replaced_parameter(
                tp,
                Some(node.as_array_type_node().element_type()),
                subst,
                bindings,
                visiting,
            ),
            Kind::OptionalType => self.type_node_mentions_replaced_parameter(
                tp,
                Some(node.as_optional_type_node().type_()),
                subst,
                bindings,
                visiting,
            ),
            Kind::RestType => self.type_node_mentions_replaced_parameter(
                tp,
                Some(node.as_rest_type_node().type_()),
                subst,
                bindings,
                visiting,
            ),
            Kind::TypeOperator => self.type_node_mentions_replaced_parameter(
                tp,
                Some(node.as_type_operator_node().type_()),
                subst,
                bindings,
                visiting,
            ),
            Kind::NamedTupleMember => self.type_node_mentions_replaced_parameter(
                tp,
                Some(node.as_named_tuple_member().type_()),
                subst,
                bindings,
                visiting,
            ),
            Kind::TupleType => {
                for &element in node.as_tuple_type_node().elements().nodes() {
                    if self.type_node_mentions_replaced_parameter(
                        tp,
                        Some(element),
                        subst,
                        bindings,
                        visiting,
                    ) {
                        return true;
                    }
                }
                false
            }
            Kind::UnionType => {
                for &member in node.as_union_type_node().types().nodes() {
                    if self.type_node_mentions_replaced_parameter(
                        tp,
                        Some(member),
                        subst,
                        bindings,
                        visiting,
                    ) {
                        return true;
                    }
                }
                false
            }
            Kind::IntersectionType => {
                for &member in node.as_intersection_type_node().types().nodes() {
                    if self.type_node_mentions_replaced_parameter(
                        tp,
                        Some(member),
                        subst,
                        bindings,
                        visiting,
                    ) {
                        return true;
                    }
                }
                false
            }
            Kind::TemplateLiteralType => {
                for &span in node.as_template_literal_type_node().template_spans().nodes() {
                    if span.kind() != Kind::TemplateLiteralTypeSpan {
                        continue;
                    }
                    if self.type_node_mentions_replaced_parameter(
                        tp,
                        Some(span.as_template_literal_type_span().type_()),
                        subst,
                        bindings,
                        visiting,
                    ) {
                        return true;
                    }
                }
                false
            }
            Kind::IndexedAccessType => {
                let indexed = node.as_indexed_access_type_node();
                self.type_node_mentions_replaced_parameter(
                    tp,
                    Some(indexed.object_type()),
                    subst,
                    bindings,
                    visiting,
                ) || self.type_node_mentions_replaced_parameter(
                    tp,
                    Some(indexed.index_type()),
                    subst,
                    bindings,
                    visiting,
                )
            }
            Kind::ConditionalType => {
                let conditional = node.as_conditional_type_node();
                self.type_node_mentions_replaced_parameter(
                    tp,
                    Some(conditional.check_type()),
                    subst,
                    bindings,
                    visiting,
                ) || self.type_node_mentions_replaced_parameter(
                    tp,
                    Some(conditional.extends_type()),
                    subst,
                    bindings,
                    visiting,
                ) || self.type_node_mentions_replaced_parameter(
                    tp,
                    Some(conditional.true_type()),
                    subst,
                    bindings,
                    visiting,
                ) || self.type_node_mentions_replaced_parameter(
                    tp,
                    Some(conditional.false_type()),
                    subst,
                    bindings,
                    visiting,
                )
            }
            Kind::TypeLiteral => {
                for &member in node.members() {
                    if self.type_node_mentions_replaced_parameter(
                        tp,
                        Some(member),
                        subst,
                        bindings,
                        visiting,
                    ) {
                        return true;
                    }
                }
                false
            }
            Kind::MappedType => {
                let mapped = node.as_mapped_type_node();
                if self.type_node_mentions_replaced_parameter(
                    tp,
                    mapped.name_type(),
                    subst,
                    bindings,
                    visiting,
                ) || self.type_node_mentions_replaced_parameter(
                    tp,
                    mapped.type_(),
                    subst,
                    bindings,
                    visiting,
                ) {
                    return true;
                }
                self.type_node_mentions_replaced_parameter(
                    tp,
                    mapped.type_parameter().as_type_parameter_declaration().constraint(),
                    subst,
                    bindings,
                    visiting,
                )
            }
            Kind::PropertySignature => self.type_node_mentions_replaced_parameter(
                tp,
                node.as_property_signature_declaration().type_(),
                subst,
                bindings,
                visiting,
            ),
            Kind::PropertyDeclaration => self.type_node_mentions_replaced_parameter(
                tp,
                node.as_property_declaration().type_(),
                subst,
                bindings,
                visiting,
            ),
            Kind::Parameter => self.type_node_mentions_replaced_parameter(
                tp,
                node.as_parameter_declaration().type_(),
                subst,
                bindings,
                visiting,
            ),
            Kind::IndexSignature => {
                let declaration = node.as_index_signature_declaration();
                if let Some(parameters) = declaration.parameters()
                    && parameters.nodes().len() == 1
                    && parameters.nodes()[0].kind() == Kind::Parameter
                    && self.type_node_mentions_replaced_parameter(
                        tp,
                        parameters.nodes()[0].as_parameter_declaration().type_(),
                        subst,
                        bindings,
                        visiting,
                    )
                {
                    return true;
                }
                self.type_node_mentions_replaced_parameter(
                    tp,
                    declaration.type_(),
                    subst,
                    bindings,
                    visiting,
                )
            }
            _ => false,
        }
    }

    // go: internal/typeparser/api_stability_safety.go:1932 apiStabilitySafetyScan.typeLiteral
    // typeLiteral scans a type literal annotation. Index signatures are evaluated
    // eagerly by member resolution; property and method annotations are evaluated
    // only when the literal is projected into.
    pub(crate) fn type_literal(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: P<Node>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        let mut verdict = ApiStabilitySafety::Safe;
        for &member in node.members() {
            let member_verdict = self.member_annotation(tp, member, subst, bindings, project);
            verdict = combine_safety(verdict, member_verdict);
        }
        verdict
    }

    // go: internal/typeparser/api_stability_safety.go:1945 apiStabilitySafetyScan.mappedTypeNode
    // mappedTypeNode scans a mapped type annotation through its eagerly evaluated
    // constraint, name and template annotations.
    pub(crate) fn mapped_type_node(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: P<Node>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        let mapped = node.as_mapped_type_node();
        let mut verdict = self.type_node(tp, mapped.name_type(), subst, bindings, project);
        let template = self.type_node(tp, mapped.type_(), subst, bindings, project);
        verdict = combine_safety(verdict, template);
        {
            let type_parameter = mapped.type_parameter();
            let constraint = self.type_node(
                tp,
                type_parameter.as_type_parameter_declaration().constraint(),
                subst,
                bindings,
                project,
            );
            verdict = combine_safety(verdict, constraint);
        }
        verdict
    }

    // go: internal/typeparser/api_stability_safety.go:1964 apiStabilitySafetyScan.memberAnnotation
    // memberAnnotation scans one class, interface or type literal member. Index
    // signatures and declared call and construct signatures are always evaluated by
    // member resolution; a named property or method is evaluated only when the
    // member surface is projected into.
    pub(crate) fn member_annotation(
        &mut self,
        tp: &mut TypeParser<'_>,
        member: P<Node>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        match member.kind() {
            Kind::IndexSignature => {
                let declaration = member.as_index_signature_declaration();
                let mut verdict = ApiStabilitySafety::Safe;
                if let Some(parameters) = declaration.parameters()
                    && parameters.nodes().len() == 1
                    && parameters.nodes()[0].kind() == Kind::Parameter
                {
                    verdict = self.type_node(
                        tp,
                        parameters.nodes()[0].as_parameter_declaration().type_(),
                        subst,
                        bindings,
                        true,
                    );
                }
                let value = self.type_node(tp, declaration.type_(), subst, bindings, true);
                combine_safety(verdict, value)
            }
            Kind::CallSignature | Kind::ConstructSignature | Kind::Constructor => {
                if self.member_has_late_bound_name(member) {
                    return ApiStabilitySafety::Unknown;
                }
                self.function_like_resolution(tp, member, subst, bindings)
            }
            Kind::PropertySignature | Kind::PropertyDeclaration => {
                if !project {
                    return ApiStabilitySafety::Safe;
                }
                if self.member_has_late_bound_name(member) {
                    return ApiStabilitySafety::Unknown;
                }
                let annotation = if member.kind() == Kind::PropertySignature {
                    member.as_property_signature_declaration().type_()
                } else {
                    member.as_property_declaration().type_()
                };
                if annotation.is_none() && self.relation_operands {
                    // The relation compares the member's resolved type. A property
                    // without an annotation is only resolvable by inference (an
                    // initializer or an implicit `any`), which the guard refuses to
                    // force unless the checker already materialized the component.
                    if let Some(symbol) = self.member_symbol_of_declaration(tp, Some(member))
                        && let Some(materialized) =
                            get_resolved_type_of_symbol_if_materialized(tp.checker, Some(symbol))
                    {
                        return self.type_value(tp, Some(materialized), subst, bindings, true);
                    }
                    return ApiStabilitySafety::Unknown;
                }
                self.type_node(tp, annotation, subst, bindings, true)
            }
            Kind::MethodSignature | Kind::MethodDeclaration | Kind::FunctionDeclaration => {
                if self.member_has_late_bound_name(member) {
                    return ApiStabilitySafety::Unknown;
                }
                if !project {
                    return ApiStabilitySafety::Safe;
                }
                self.function_like_resolution(tp, member, subst, bindings)
            }
            Kind::GetAccessor | Kind::SetAccessor => {
                if self.member_has_late_bound_name(member) {
                    return ApiStabilitySafety::Unknown;
                }
                if !project {
                    return ApiStabilitySafety::Safe;
                }
                self.function_like_resolution(tp, member, subst, bindings)
            }
            _ => {
                if self.member_has_late_bound_name(member) {
                    return ApiStabilitySafety::Unknown;
                }
                ApiStabilitySafety::Safe
            }
        }
    }

    // go: internal/typeparser/api_stability_safety.go:2035 apiStabilitySafetyScan.functionLikeResolution
    // functionLikeResolution scans the parameter and type parameter constraint
    // annotations a function-like declaration resolves eagerly. The return
    // annotation is lazy and gated by its own read; a relation-operand scan reads
    // it too, and projects parameter and return member surfaces, because the
    // relation compares them structurally and would instantiate them. A relation
    // operand whose inferred return the relation would have to infer from a body is
    // refused.
    pub(crate) fn function_like_resolution(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: P<Node>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
    ) -> ApiStabilitySafety {
        let Some(function_like) = node.function_like_data() else {
            return ApiStabilitySafety::Safe;
        };
        // The relation resolves the full projected member surfaces of parameters
        // and returns while it compares them, so a relation-operand scan projects
        // into both positions exactly as the relation would.
        let project = self.relation_operands;
        let mut verdict = ApiStabilitySafety::Safe;
        for &parameter in function_like_parameter_nodes(Some(function_like)) {
            if parameter.kind() != Kind::Parameter {
                continue;
            }
            let parameter_verdict = self.type_node(
                tp,
                parameter.as_parameter_declaration().type_(),
                subst,
                bindings,
                project,
            );
            verdict = combine_safety(verdict, parameter_verdict);
        }
        if let Some(type_parameters) = function_like.type_parameters() {
            for &type_parameter in type_parameters.nodes() {
                if type_parameter.kind() != Kind::TypeParameter {
                    continue;
                }
                let declaration = type_parameter.as_type_parameter_declaration();
                if self.relation_operands {
                    // The relation instantiates a generic signature in the context
                    // of the other operand and resolves the declaration's
                    // constraints and defaults while inferring. A constraint or
                    // default whose raw structure mentions a replaced parameter, or
                    // whose projected surface would expand a recursive alias, is
                    // refused rather than evaluated.
                    let mut visiting = FxHashSet::default();
                    if self.type_node_mentions_replaced_parameter(
                        tp,
                        declaration.constraint(),
                        subst,
                        bindings,
                        &mut visiting,
                    ) || self.type_node_mentions_replaced_parameter(
                        tp,
                        declaration.default_type(),
                        subst,
                        bindings,
                        &mut visiting,
                    ) {
                        return ApiStabilitySafety::Unknown;
                    }
                }
                let constraint =
                    self.type_node(tp, declaration.constraint(), subst, bindings, project);
                verdict = combine_safety(verdict, constraint);
                if self.relation_operands {
                    let default_verdict =
                        self.type_node(tp, declaration.default_type(), subst, bindings, project);
                    verdict = combine_safety(verdict, default_verdict);
                }
            }
        }
        if self.relation_operands {
            if function_like.type_().is_none() && !ets_shim::ast::node_is_missing(node.body()) {
                // The relation would infer the return type from the body, which the
                // guard refuses to force.
                return ApiStabilitySafety::Unknown;
            }
            let return_verdict =
                self.type_node(tp, function_like.type_(), subst, bindings, project);
            verdict = combine_safety(verdict, return_verdict);
        }
        verdict
    }

    // go: internal/typeparser/api_stability_safety.go:2093 apiStabilitySafetyScan.memberSymbolOfDeclaration
    // memberSymbolOfDeclaration returns the checker symbol a member declaration
    // names, or nil when the declaration does not name one. It only looks up the
    // declared name; no member type is resolved.
    pub(crate) fn member_symbol_of_declaration(
        &mut self,
        tp: &mut TypeParser<'_>,
        member: Option<P<Node>>,
    ) -> Option<P<Symbol>> {
        let name = ets_shim::ast::get_name_of_declaration(member?)?;
        if name.kind() == Kind::ComputedPropertyName {
            return None;
        }
        self.a.symbol_at_type_name_node(tp, Some(name))
    }

    // go: internal/typeparser/api_stability_safety.go:2108 apiStabilitySafetyScan.memberHasLateBoundName
    // memberHasLateBoundName reports whether a member's declared name must be
    // evaluated to resolve the member table. A computed name whose expression is an
    // entity name is late-bindable and is refused rather than checked, except for
    // the well-known `Symbol.*` names whose evaluation is a pure library lookup.
    #[expect(clippy::unused_self, reason = "a method of Go's apiStabilitySafetyScan")]
    pub(crate) fn member_has_late_bound_name(&self, member: P<Node>) -> bool {
        let Some(name) = ets_shim::ast::get_name_of_declaration(member) else {
            return false;
        };
        if name.kind() != Kind::ComputedPropertyName {
            return false;
        }
        let Some(expression) = name.expression() else {
            return false;
        };
        if !ets_shim::ast::is_entity_name_expression(expression) {
            return false;
        }
        !api_stability_well_known_symbol_name(Some(expression))
    }
}

// go: internal/typeparser/api_stability_safety.go:2123 apiStabilityWellKnownSymbolName
// apiStabilityWellKnownSymbolName reports whether an entity-name expression is
// a `Symbol.<well-known>` reference that resolves to a library unique symbol
// without checking program code.
pub(crate) fn api_stability_well_known_symbol_name(expression: Option<P<Node>>) -> bool {
    let Some(expression) = expression else {
        return false;
    };
    if expression.kind() != Kind::PropertyAccessExpression {
        return false;
    }
    let access = expression.as_property_access_expression();
    let access_expression = access.expression();
    let name = access.name();
    if access_expression.kind() != Kind::Identifier || access_expression.text() != "Symbol" {
        return false;
    }
    matches!(
        name.text(),
        "iterator"
            | "asyncIterator"
            | "hasInstance"
            | "isConcatSpreadable"
            | "match"
            | "matchAll"
            | "replace"
            | "search"
            | "species"
            | "split"
            | "toPrimitive"
            | "toStringTag"
            | "unscopables"
    )
}

// go: internal/typeparser/api_stability_safety.go:2146 apiStabilityWellKnownSymbolDisplayName
// apiStabilityWellKnownSymbolDisplayName renders a well-known `Symbol.<name>`
// computed-name expression for diagnostics, or "" when the expression is not a
// well-known symbol reference. It reads the expression's identifiers only and
// never evaluates it.
pub(crate) fn api_stability_well_known_symbol_display_name(expression: Option<P<Node>>) -> String {
    if !api_stability_well_known_symbol_name(expression) {
        return String::new();
    }
    let Some(expression) = expression else {
        return String::new();
    };
    let access = expression.as_property_access_expression();
    format!("[Symbol.{}]", access.name().text())
}

impl ApiStabilityAnalysis {
    // go: internal/typeparser/api_stability_safety.go:2163 apiStabilityAnalysis.symbolHasLateBoundMembers
    // symbolHasLateBoundMembers reports whether any declaration member of a symbol
    // still carries the binder's unresolved computed-name placeholder. Such a
    // member is absent from the early member table and only appears once the
    // checker late-binds its name, so a surface read from the early table is
    // incomplete. A computed member the binder could name with a literal (or a
    // literal type) is already in the early table and does not set the flag.
    #[expect(clippy::unused_self, reason = "a method of Go's apiStabilityAnalysis")]
    pub(crate) fn symbol_has_late_bound_members(&self, symbol: Option<P<Symbol>>) -> bool {
        let Some(symbol) = symbol else {
            return false;
        };
        for &declaration in symbol.declarations() {
            match declaration.kind() {
                Kind::InterfaceDeclaration
                | Kind::ClassDeclaration
                | Kind::ClassExpression
                | Kind::TypeLiteral
                | Kind::EnumDeclaration => {}
                _ => continue,
            }
            for &member in declaration.members() {
                if let Some(member_symbol) = member.symbol()
                    && member_symbol.name() == ets_shim::ast::InternalSymbolNameComputed
                {
                    return true;
                }
            }
        }
        false
    }
}

impl ApiStabilitySafetyScan<'_> {
    // go: internal/typeparser/api_stability_safety.go:2193 apiStabilitySafetyScan.declarationMembers
    // declarationMembers scans the member resolution of a declaration symbol: index
    // signature annotations are evaluated eagerly, heritage is resolved
    // recursively, and a projected read evaluates the member annotations too.
    pub(crate) fn declaration_members(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: Option<P<Symbol>>,
        arguments: Option<P<NodeList>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        let Some(symbol) = symbol else {
            return ApiStabilitySafety::Unknown;
        };
        let params = local_type_parameters(tp.checker, symbol);
        let mut frame = bindings.cloned();
        if !params.is_empty() {
            let mut args = vec![ApiStabilitySafetyArg::default(); params.len()];
            if let Some(arguments) = arguments {
                for (index, &argument) in arguments.nodes().iter().enumerate() {
                    if index >= params.len() {
                        continue;
                    }
                    args[index] = ApiStabilitySafetyArg {
                        node: Some(argument),
                        subst: subst.clone(),
                        bindings: bindings.cloned(),
                        ..Default::default()
                    };
                }
            }
            frame = Some(Rc::new(ApiStabilitySafetyBinding {
                parent: bindings.cloned(),
                params,
                args: RefCell::new(args),
            }));
        }
        self.declaration_structure(tp, Some(symbol), subst, frame.as_ref(), project)
    }

    // go: internal/typeparser/api_stability_safety.go:2218 apiStabilitySafetyScan.declarationMembersOfRepresentedArguments
    // declarationMembersOfRepresentedArguments scans a reference target's
    // declaration surface with the reference's represented arguments bound as
    // concrete values. A nil argument stays an empty binding, so a parameter the
    // checker has not represented cannot fall back to its constraint.
    pub(crate) fn declaration_members_of_represented_arguments(
        &mut self,
        tp: &mut TypeParser<'_>,
        target: Option<P<Type>>,
        arguments: &[P<Type>],
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        let Some(target) = target else {
            return ApiStabilitySafety::Unknown;
        };
        let Some(symbol) = target.symbol() else {
            return ApiStabilitySafety::Unknown;
        };
        let params = local_type_parameters(tp.checker, symbol);
        let mut frame = bindings.cloned();
        if !params.is_empty() {
            let mut args = vec![ApiStabilitySafetyArg::default(); params.len()];
            for index in 0..params.len() {
                if index < arguments.len() {
                    args[index] =
                        ApiStabilitySafetyArg { t: Some(arguments[index]), ..Default::default() };
                }
            }
            frame = Some(Rc::new(ApiStabilitySafetyBinding {
                parent: bindings.cloned(),
                params,
                args: RefCell::new(args),
            }));
        }
        self.declaration_structure(tp, Some(symbol), subst, frame.as_ref(), project)
    }

    // go: internal/typeparser/api_stability_safety.go:2242 apiStabilitySafetyScan.declarationStructure
    // declarationStructure scans a declaration symbol's own member and heritage
    // structure.
    pub(crate) fn declaration_structure(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: Option<P<Symbol>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        let Some(symbol) = symbol else {
            return ApiStabilitySafety::Unknown;
        };
        if self.relation_operands && !self.a.should_inspect_symbol(tp, Some(symbol)) {
            // A default-library declaration is fixed and never references a program
            // alias; the relation may compare it freely. Only program symbols
            // reached through its represented arguments can carry a recursive
            // alias.
            return ApiStabilitySafety::Safe;
        }
        if self.active_declarations.contains(&symbol) {
            return ApiStabilitySafety::Unknown;
        }
        if self.exceeded() {
            return ApiStabilitySafety::Unknown;
        }
        self.active_declarations.insert(symbol);

        let mut verdict = ApiStabilitySafety::Safe;
        for &declaration in symbol.declarations() {
            match declaration.kind() {
                Kind::InterfaceDeclaration
                | Kind::ClassDeclaration
                | Kind::ClassExpression
                | Kind::TypeLiteral
                | Kind::EnumDeclaration => {
                    for &member in declaration.members() {
                        let member_verdict =
                            self.member_annotation(tp, member, subst, bindings, project);
                        verdict = combine_safety(verdict, member_verdict);
                    }
                    let heritage = self.heritage(tp, Some(symbol), subst, bindings, project);
                    verdict = combine_safety(verdict, heritage);
                }
                Kind::MappedType => {
                    let mapped = self.mapped_type_node(tp, declaration, subst, bindings, project);
                    verdict = combine_safety(verdict, mapped);
                }
                _ => {
                    if declaration.function_like_data().is_some() && self.relation_operands {
                        let resolution =
                            self.function_like_resolution(tp, declaration, subst, bindings);
                        verdict = combine_safety(verdict, resolution);
                        continue;
                    }
                    let annotation = declaration_annotation_node_of(Some(declaration));
                    let annotation_verdict =
                        self.type_node(tp, annotation, subst, bindings, project);
                    verdict = combine_safety(verdict, annotation_verdict);
                }
            }
        }
        // Rust: Go's `defer delete(s.activeDeclarations, symbol)`.
        self.active_declarations.remove(&symbol);
        verdict
    }

    // go: internal/typeparser/api_stability_safety.go:2296 apiStabilitySafetyScan.heritage
    // heritage scans the heritage clause type arguments a base-type resolution
    // evaluates, and recursively inspects the base declarations' own member
    // structure because member resolution walks the whole inheritance chain. The
    // projection flag is propagated: when a projected read evaluates the derived
    // member surface, inherited member annotations are evaluated with it.
    pub(crate) fn heritage(
        &mut self,
        tp: &mut TypeParser<'_>,
        symbol: Option<P<Symbol>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        let Some(symbol) = symbol else {
            return ApiStabilitySafety::Safe;
        };
        let mut verdict = ApiStabilitySafety::Safe;
        for &declaration in symbol.declarations() {
            let clauses = match declaration.kind() {
                Kind::ClassDeclaration | Kind::ClassExpression => {
                    declaration.class_like_data().and_then(ClassLikeBase::heritage_clauses)
                }
                Kind::InterfaceDeclaration => {
                    declaration.as_interface_declaration().heritage_clauses()
                }
                _ => None,
            };
            let Some(clauses) = clauses else {
                continue;
            };
            for &clause_node in clauses.nodes() {
                let types = clause_node.as_heritage_clause().types();
                for &type_node in types.nodes() {
                    // An interface heritage entry is written as a type reference,
                    // a class heritage entry as an expression with type arguments.
                    let (base_symbol, arguments) = match type_node.kind() {
                        Kind::TypeReference => {
                            let reference = type_node.as_type_reference_node();
                            (
                                self.a.symbol_at_type_name_node(tp, Some(reference.type_name())),
                                reference.type_arguments(),
                            )
                        }
                        Kind::ExpressionWithTypeArguments => {
                            let expression = type_node.as_expression_with_type_arguments();
                            (
                                self.a.symbol_at_type_name_node(tp, Some(expression.expression())),
                                expression.type_arguments(),
                            )
                        }
                        _ => continue,
                    };
                    let Some(base_symbol) = base_symbol else {
                        verdict = combine_safety(verdict, ApiStabilitySafety::Unknown);
                        continue;
                    };
                    if arguments.is_some() {
                        let arguments_verdict = self.type_arguments(tp, arguments, subst, bindings);
                        verdict = combine_safety(verdict, arguments_verdict);
                    }
                    let base_params = local_type_parameters(tp.checker, base_symbol);
                    let mut frame = bindings.cloned();
                    if !base_params.is_empty() {
                        let mut args = vec![ApiStabilitySafetyArg::default(); base_params.len()];
                        if let Some(arguments) = arguments {
                            for (index, &argument) in arguments.nodes().iter().enumerate() {
                                if index >= base_params.len() {
                                    continue;
                                }
                                args[index] = ApiStabilitySafetyArg {
                                    node: Some(argument),
                                    subst: subst.clone(),
                                    bindings: bindings.cloned(),
                                    ..Default::default()
                                };
                            }
                        }
                        frame = Some(Rc::new(ApiStabilitySafetyBinding {
                            parent: bindings.cloned(),
                            params: base_params,
                            args: RefCell::new(args),
                        }));
                    }
                    let structure = self.declaration_structure(
                        tp,
                        Some(base_symbol),
                        subst,
                        frame.as_ref(),
                        project,
                    );
                    verdict = combine_safety(verdict, structure);
                }
            }
        }
        verdict
    }

    // go: internal/typeparser/api_stability_safety.go:2375 apiStabilitySafetyScan.memberTable
    // memberTable scans a represented type that is about to have its member table
    // resolved.
    pub(crate) fn member_table(
        &mut self,
        tp: &mut TypeParser<'_>,
        t: Option<P<Type>>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
    ) -> ApiStabilitySafety {
        let Some(t) = t else {
            return ApiStabilitySafety::Safe;
        };
        if self.exceeded() {
            return ApiStabilitySafety::Unknown;
        }
        let t = non_distributed_parameter(t);
        let flags = t.flags();
        if flags.intersects(TypeFlags::TypeParameter) {
            return self.constraint_safety(tp, t, subst, bindings, true);
        }
        if flags.intersects(TypeFlags::UnionOrIntersection) {
            let mut verdict = ApiStabilitySafety::Safe;
            for &member in t.types() {
                let member_verdict = self.member_table(tp, Some(member), subst, bindings);
                verdict = combine_safety(verdict, member_verdict);
            }
            return verdict;
        }
        if flags.intersects(TypeFlags::Object) {
            if t.object_flags().intersects(ObjectFlags::Mapped) {
                return self.mapped_value(tp, t, subst, bindings, false);
            }
            if t.object_flags().intersects(ObjectFlags::Reference)
                && let Some(target) = t.target()
                && target != t
            {
                let mut verdict = ApiStabilitySafety::Safe;
                for &argument in get_resolved_type_arguments(Some(t)) {
                    let argument_verdict =
                        self.type_value(tp, Some(argument), subst, bindings, false);
                    verdict = combine_safety(verdict, argument_verdict);
                }
                let structure =
                    self.declaration_structure(tp, target.symbol(), subst, bindings, false);
                return combine_safety(verdict, structure);
            }
            if let Some(symbol) = t.symbol() {
                return self.declaration_structure(tp, Some(symbol), subst, bindings, false);
            }
            return ApiStabilitySafety::Safe;
        }
        ApiStabilitySafety::Safe
    }

    // go: internal/typeparser/api_stability_safety.go:2414 apiStabilitySafetyScan.typeQuery
    // typeQuery scans a `typeof` annotation through the value symbol it names.
    pub(crate) fn type_query(
        &mut self,
        tp: &mut TypeParser<'_>,
        node: P<Node>,
        subst: &ApiStabilitySubstitution,
        bindings: Option<&Rc<ApiStabilitySafetyBinding>>,
        project: bool,
    ) -> ApiStabilitySafety {
        let query = node.as_type_query_node();
        let Some(symbol) = self.a.symbol_at_type_name_node(tp, Some(query.expr_name())) else {
            return ApiStabilitySafety::Unknown;
        };
        if let Some(materialized) =
            get_resolved_type_of_symbol_if_materialized(tp.checker, Some(symbol))
        {
            return self.type_value(tp, Some(materialized), subst, bindings, project);
        }
        for &declaration in symbol.declarations() {
            if declaration.function_like_data().is_some() {
                let verdict = self.function_like_resolution(tp, declaration, subst, bindings);
                if verdict != ApiStabilitySafety::Safe {
                    return verdict;
                }
                continue;
            }
            let Some(annotation) = declaration_annotation_node_of(Some(declaration)) else {
                if self.relation_operands {
                    // The relation would resolve the inferred value's type.
                    return ApiStabilitySafety::Unknown;
                }
                continue;
            };
            let verdict = self.type_node(tp, Some(annotation), subst, bindings, project);
            if verdict != ApiStabilitySafety::Safe {
                return verdict;
            }
        }
        ApiStabilitySafety::Safe
    }
}

// go: internal/typeparser/api_stability_safety.go:2454 nonDistributedParameter
// nonDistributedParameter normalizes a distributed conditional's cloned check
// parameter back to its declared binder.
//
// Rust: Go also passes the checker, which the shim accessor does not read.
pub(crate) fn non_distributed_parameter(t: P<Type>) -> P<Type> {
    if !t.flags().intersects(TypeFlags::TypeParameter) {
        return t;
    }
    if let Some(normalized) = get_non_distributed_type_parameter(Some(t))
        && normalized != t
    {
        return normalized;
    }
    t
}

// go: internal/typeparser/api_stability_safety.go:2467 safetyTypeParameterDefaults
// safetyTypeParameterDefaults returns the declared default annotations of a
// symbol's type parameters, aligned with the symbol's local type parameter
// order.
pub(crate) fn safety_type_parameter_defaults(symbol: Option<P<Symbol>>) -> Vec<Option<P<Node>>> {
    let Some(symbol) = symbol else {
        return Vec::new();
    };
    for &declaration in symbol.declarations() {
        let type_parameters = match declaration.kind() {
            Kind::TypeAliasDeclaration | Kind::JSTypeAliasDeclaration => {
                declaration.as_type_alias_declaration().type_parameters()
            }
            Kind::InterfaceDeclaration => declaration.as_interface_declaration().type_parameters(),
            Kind::ClassDeclaration | Kind::ClassExpression => {
                declaration.class_like_data().and_then(ClassLikeBase::type_parameters)
            }
            _ => None,
        };
        let Some(type_parameters) = type_parameters else {
            continue;
        };
        let mut defaults = Vec::with_capacity(type_parameters.nodes().len());
        for &type_parameter in type_parameters.nodes() {
            if type_parameter.kind() != Kind::TypeParameter {
                defaults.push(None);
                continue;
            }
            defaults.push(type_parameter.as_type_parameter_declaration().default_type());
        }
        return defaults;
    }
    Vec::new()
}

// go: internal/typeparser/api_stability_safety.go:2506 functionLikeParameterNodes
// functionLikeParameterNodes returns the declared parameters of a function-like
// node, or nil when it declares none.
pub(crate) fn function_like_parameter_nodes(
    function_like: Option<&FunctionLikeBase>,
) -> &'static [P<Node>] {
    function_like.and_then(FunctionLikeBase::parameters).map_or(&[], |list| list.nodes())
}

// go: internal/typeparser/api_stability_safety.go:2515 declarationAnnotationNodeOf
// declarationAnnotationNodeOf returns the type annotation node a declaration
// resolves, or nil for an inferred declaration.
pub(crate) fn declaration_annotation_node_of(declaration: Option<P<Node>>) -> Option<P<Node>> {
    let declaration = declaration?;
    match declaration.kind() {
        Kind::VariableDeclaration => declaration.as_variable_declaration().type_(),
        Kind::PropertyDeclaration => declaration.as_property_declaration().type_(),
        Kind::PropertySignature => declaration.as_property_signature_declaration().type_(),
        Kind::Parameter => declaration.as_parameter_declaration().type_(),
        Kind::TypeAliasDeclaration | Kind::JSTypeAliasDeclaration => declaration.type_node(),
        Kind::GetAccessor => declaration.function_like_data()?.type_(),
        Kind::SetAccessor => {
            let function_like = declaration.function_like_data()?;
            for &parameter in function_like_parameter_nodes(Some(function_like)) {
                if parameter.kind() == Kind::Parameter {
                    return parameter.as_parameter_declaration().type_();
                }
            }
            None
        }
        _ => None,
    }
}
