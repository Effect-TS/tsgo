package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// strictEffectTypeNames are the type symbol names that StrictEffectType can
// match. couldBeNamed takes a set so future prefilters for other wrapper
// types (Stream, Layer, ...) can reuse the same conservative walk.
var strictEffectTypeNames = map[string]bool{"Effect": true}

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
func (tp *TypeParser) NodeCouldBeStrictEffect(node *ast.Node) bool {
	if tp == nil || tp.checker == nil || node == nil {
		return true
	}
	// An object literal's type is anonymous, so it is never named "Effect".
	if node.Kind == ast.KindObjectLiteralExpression || tp.NodeCannotBeEffectLibraryType(node) {
		return false
	}
	if node.Kind == ast.KindCallExpression {
		return tp.callCouldReturnStrictEffect(node)
	}
	if node.Kind != ast.KindIdentifier && node.Kind != ast.KindPropertyAccessExpression {
		return true
	}
	sym := tp.ReferenceSymbolAtNode(node)
	if sym == nil {
		return true
	}
	return tp.SymbolCouldBeStrictEffect(sym)
}

// callCouldReturnStrictEffect reports whether a call expression's type could
// possibly be a strict Effect type, based on the return type of its resolved
// signature. The signature and its return type are cached from the main check
// phase, so consulting them is cheap compared to re-checking the call via
// GetTypeAtLocation. A call expression's type is its resolved signature's
// return type (union-widened with undefined for optional chains, which the
// conservative union walk handles), so a conclusively non-Effect declared
// return type rules the node out.
func (tp *TypeParser) callCouldReturnStrictEffect(node *ast.Node) (result bool) {
	defer func() {
		if r := recover(); r != nil {
			result = true
		}
	}()
	signature := tp.checker.GetResolvedSignature(node)
	if signature == nil {
		return true
	}
	return couldBeNamed(tp.checker.GetReturnTypeOfSignature(signature), strictEffectTypeNames, 0)
}

// SymbolCouldBeStrictEffect reports whether a reference to sym could possibly
// have a strict-Effect flow type, based on the symbol's declared type only.
func (tp *TypeParser) SymbolCouldBeStrictEffect(sym *ast.Symbol) bool {
	if tp == nil || tp.checker == nil || sym == nil {
		return true
	}
	declared := tp.getTypeOfSymbolSafe(sym)
	return tp.CouldBeStrictEffect(declared)
}

// getTypeOfSymbolSafe wraps Checker.GetTypeOfSymbol with a panic guard,
// returning nil (treated as inconclusive by callers) on any checker panic.
func (tp *TypeParser) getTypeOfSymbolSafe(sym *ast.Symbol) (result *checker.Type) {
	defer func() {
		if r := recover(); r != nil {
			result = nil
		}
	}()
	return tp.checker.GetTypeOfSymbol(sym)
}

// CouldBeStrictEffect reports whether flow narrowing starting from declared
// type t could ever produce a strict Effect type. It is deliberately
// conservative: it only returns false when t is conclusively non-Effect (a
// primitive/never type, or a plain object type with a non-nil symbol whose
// name is not "Effect"). Any/unknown, type variables, unions, intersections
// and symbol-less types all return true.
func (tp *TypeParser) CouldBeStrictEffect(t *checker.Type) bool {
	return couldBeNamed(t, strictEffectTypeNames, 0)
}

// couldBeNamed reports whether flow narrowing starting from declared type t
// could ever produce a type whose symbol name is in names. False only on a
// conclusive negative.
func couldBeNamed(t *checker.Type, names map[string]bool, depth int) bool {
	return couldBe(t, depth, func(object *checker.Type) bool {
		sym := object.Symbol()
		return sym == nil || names[ast.TSGetSymbolName(sym)]
	})
}

// couldBe walks declared type t conservatively and asks objectCouldMatch
// about each plain object constituent. False only on a conclusive negative.
func couldBe(t *checker.Type, depth int, objectCouldMatch func(*checker.Type) bool) bool {
	if t == nil {
		return true
	}
	flags := t.Flags()
	if flags&checker.TypeFlagsAnyOrUnknown != 0 {
		return true
	}
	if flags&checker.TypeFlagsUnionOrIntersection != 0 {
		if depth > 4 {
			return true
		}
		for _, member := range t.Types() {
			if couldBe(member, depth+1, objectCouldMatch) {
				return true
			}
		}
		return false
	}
	// Type parameters, indexed accesses, conditionals, substitutions, etc.
	// can instantiate to anything.
	if flags&checker.TypeFlagsInstantiable != 0 {
		return true
	}
	// Primitives and never can never narrow to an object type.
	if flags&(checker.TypeFlagsPrimitive|checker.TypeFlagsNever) != 0 {
		return false
	}
	// Anything that is not a plain object type at this point is unexpected;
	// stay conservative.
	if flags&checker.TypeFlagsObject == 0 {
		return true
	}
	return objectCouldMatch(t)
}

// NodeCouldBeEffect reports whether node's type could possibly be an Effect
// type as recognized by EffectType. Only calls are inspected beyond the
// syntactic NodeCannotBeEffectLibraryType check: a call's type is its
// resolved signature's return type and is never narrowed, while a reference
// can be narrowed by a type guard such as Effect.isEffect into an Effect
// whatever its declared type. Only v4 identifies Effects by a single
// property, so calls stay possible for other versions.
func (tp *TypeParser) NodeCouldBeEffect(node *ast.Node) (result bool) {
	if tp == nil || tp.checker == nil || node == nil {
		return true
	}
	if tp.NodeCannotBeEffectLibraryType(node) {
		return false
	}
	if node.Kind != ast.KindCallExpression || tp.DetectEffectVersion() != EffectMajorV4 {
		return true
	}
	defer func() {
		if r := recover(); r != nil {
			result = true
		}
	}()
	signature := tp.checker.GetResolvedSignature(node)
	if signature == nil {
		return true
	}
	return couldBe(tp.checker.GetReturnTypeOfSignature(signature), 0, func(object *checker.Type) bool {
		return tp.GetTypeOfPropertyByName(object, EffectTypeId) != nil
	})
}
