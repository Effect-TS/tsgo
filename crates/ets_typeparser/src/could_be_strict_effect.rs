use std::panic::AssertUnwindSafe;

use ets_go::panics::recover;
use ets_shim::ast::{Kind, Node, Symbol};
use ets_shim::checker::{Type, TypeFlags};
use ets_shim::core::P;

use crate::*;

// go: internal/typeparser/could_be_strict_effect.go:11 strictEffectTypeNames
// strictEffectTypeNames are the type symbol names that StrictEffectType can
// match. couldBeNamed takes a set so future prefilters for other wrapper
// types (Stream, Layer, ...) can reuse the same conservative walk.
//
// Rust: Go's `map[string]bool` set is the slice of its keys.
pub(crate) static STRICT_EFFECT_TYPE_NAMES: &[&str] = &["Effect"];

impl TypeParser<'_> {
    // go: internal/typeparser/could_be_strict_effect.go:30 TypeParser.NodeCouldBeStrictEffect
    // NodeCouldBeStrictEffect reports whether node's flow type could possibly be
    // a strict Effect type (a type whose symbol is named "Effect", see
    // StrictEffectType). For reference nodes (identifiers and property accesses)
    // it inspects the referenced symbol's declared type, which is cheap compared
    // to the flow analysis performed by GetTypeAtLocation.
    //
    // The declared-type shortcut is not sound for every narrowing: a type guard
    // can narrow any declared type into an Effect, e.g. Effect.isEffect narrows
    // to Effect<unknown, unknown, unknown>. Callers accept this because they only
    // report on specific A or E channel contents (an Effect in E for
    // effectInFailure, a Promise in A for promiseInEffectSuccess), which an
    // all-unknown Effect never has; only a custom guard narrowing to a concrete
    // Effect type could be missed. Checking every reference costs about 5s of
    // wall time on the Effect monorepo.
    //
    // It returns true ("cannot rule out") for every other node kind and whenever
    // the answer is not conclusively negative.
    pub fn node_could_be_strict_effect(&mut self, node: impl Into<Option<P<Node>>>) -> bool {
        let Some(node) = node.into() else {
            return true;
        };
        // An object literal's type is anonymous, so it is never named "Effect".
        if node.kind() == Kind::ObjectLiteralExpression
            || self.node_cannot_be_effect_library_type(node)
        {
            return false;
        }
        if node.kind() == Kind::CallExpression {
            return self.call_could_return_strict_effect(node);
        }
        if node.kind() != Kind::Identifier && node.kind() != Kind::PropertyAccessExpression {
            return true;
        }
        let Some(sym) = self.reference_symbol_at_node(node) else {
            return true;
        };
        self.symbol_could_be_strict_effect(sym)
    }

    // go: internal/typeparser/could_be_strict_effect.go:59 TypeParser.callCouldReturnStrictEffect
    // callCouldReturnStrictEffect reports whether a call expression's type could
    // possibly be a strict Effect type, based on the return type of its resolved
    // signature. The signature and its return type are cached from the main check
    // phase, so consulting them is cheap compared to re-checking the call via
    // GetTypeAtLocation. A call expression's type is its resolved signature's
    // return type (union-widened with undefined for optional chains, which the
    // conservative union walk handles), so a conclusively non-Effect declared
    // return type rules the node out.
    //
    // Rust: Go's nil-signature check drops (tsrs's GetResolvedSignature never returns nil, see docs/TSRS_API.md).
    pub(crate) fn call_could_return_strict_effect(&mut self, node: P<Node>) -> bool {
        // Go: a deferred recover turns a panic into true.
        recover(AssertUnwindSafe(|| {
            let signature = self.checker.get_resolved_signature_exported(node);
            could_be_named(
                Some(self.checker.get_return_type_of_signature_exported(signature)),
                STRICT_EFFECT_TYPE_NAMES,
                0,
            )
        }))
        .unwrap_or(true)
    }

    // go: internal/typeparser/could_be_strict_effect.go:74 TypeParser.SymbolCouldBeStrictEffect
    // SymbolCouldBeStrictEffect reports whether a reference to sym could possibly
    // have a strict-Effect flow type, based on the symbol's declared type only.
    pub fn symbol_could_be_strict_effect(&mut self, sym: impl Into<Option<P<Symbol>>>) -> bool {
        let Some(sym) = sym.into() else {
            return true;
        };
        let declared = self.get_type_of_symbol_safe(sym);
        self.could_be_strict_effect(declared)
    }

    // go: internal/typeparser/could_be_strict_effect.go:84 TypeParser.getTypeOfSymbolSafe
    // getTypeOfSymbolSafe wraps Checker.GetTypeOfSymbol with a panic guard,
    // returning nil (treated as inconclusive by callers) on any checker panic.
    pub(crate) fn get_type_of_symbol_safe(&mut self, sym: P<Symbol>) -> Option<P<Type>> {
        recover(AssertUnwindSafe(|| self.checker.get_type_of_symbol_exported(sym))).ok()
    }

    // go: internal/typeparser/could_be_strict_effect.go:99 TypeParser.CouldBeStrictEffect
    // CouldBeStrictEffect reports whether flow narrowing starting from declared
    // type t could ever produce a strict Effect type. It is deliberately
    // conservative: it only returns false when t is conclusively non-Effect (a
    // primitive/never type, or a plain object type with a non-nil symbol whose
    // name is not "Effect"). Any/unknown, type variables, unions, intersections
    // and symbol-less types all return true.
    #[expect(clippy::unused_self, reason = "a TypeParser method in Go")]
    pub fn could_be_strict_effect(&self, t: impl Into<Option<P<Type>>>) -> bool {
        could_be_named(t.into(), STRICT_EFFECT_TYPE_NAMES, 0)
    }
}

// go: internal/typeparser/could_be_strict_effect.go:106 couldBeNamed
// couldBeNamed reports whether flow narrowing starting from declared type t
// could ever produce a type whose symbol name is in names. False only on a
// conclusive negative.
pub(crate) fn could_be_named(t: Option<P<Type>>, names: &[&str], depth: i64) -> bool {
    could_be(t, depth, &mut |object| {
        let sym = object.symbol();
        sym.is_none_or(|sym| names.contains(&sym.name()))
    })
}

// go: internal/typeparser/could_be_strict_effect.go:115 couldBe
// couldBe walks declared type t conservatively and asks objectCouldMatch
// about each plain object constituent. False only on a conclusive negative.
pub(crate) fn could_be(
    t: Option<P<Type>>,
    depth: i64,
    object_could_match: &mut dyn FnMut(P<Type>) -> bool,
) -> bool {
    let Some(t) = t else {
        return true;
    };
    let flags = t.flags();
    if flags.intersects(TypeFlags::AnyOrUnknown) {
        return true;
    }
    if flags.intersects(TypeFlags::UnionOrIntersection) {
        if depth > 4 {
            return true;
        }
        for &member in t.types() {
            if could_be(Some(member), depth + 1, object_could_match) {
                return true;
            }
        }
        return false;
    }
    // Type parameters, indexed accesses, conditionals, substitutions, etc.
    // can instantiate to anything.
    if flags.intersects(TypeFlags::Instantiable) {
        return true;
    }
    // Primitives and never can never narrow to an object type.
    if flags.intersects(TypeFlags::Primitive | TypeFlags::Never) {
        return false;
    }
    // Anything that is not a plain object type at this point is unexpected;
    // stay conservative.
    if !flags.intersects(TypeFlags::Object) {
        return true;
    }
    object_could_match(t)
}

impl TypeParser<'_> {
    // go: internal/typeparser/could_be_strict_effect.go:158 TypeParser.NodeCouldBeEffect
    // NodeCouldBeEffect reports whether node's type could possibly be an Effect
    // type as recognized by EffectType. Only calls are inspected beyond the
    // syntactic NodeCannotBeEffectLibraryType check: a call's type is its
    // resolved signature's return type and is never narrowed, while a reference
    // can be narrowed by a type guard such as Effect.isEffect into an Effect
    // whatever its declared type. Only v4 identifies Effects by a single
    // property, so calls stay possible for other versions.
    pub fn node_could_be_effect(&mut self, node: impl Into<Option<P<Node>>>) -> bool {
        let Some(node) = node.into() else {
            return true;
        };
        if self.node_cannot_be_effect_library_type(node) {
            return false;
        }
        if node.kind() != Kind::CallExpression
            || self.detect_effect_version() != EffectMajorVersion::V4
        {
            return true;
        }
        // Go: a deferred recover turns a panic into true.
        recover(AssertUnwindSafe(|| {
            let signature = self.checker.get_resolved_signature_exported(node);
            let return_type = self.checker.get_return_type_of_signature_exported(signature);
            could_be(Some(return_type), 0, &mut |object| {
                self.get_type_of_property_by_name(object, EFFECT_TYPE_ID).is_some()
            })
        }))
        .unwrap_or(true)
    }
}
