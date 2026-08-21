package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

// strictEffectTypeNames are the type symbol names that StrictEffectType can
// match. couldBeNamed takes a set so future prefilters for other wrapper
// types (Stream, Layer, ...) can reuse the same conservative walk.
var strictEffectTypeNames = map[string]bool{"Effect": true}

// NodeCouldBeStrictEffect reports whether node's flow type could possibly be
// a strict Effect type (a type whose symbol is named "Effect", see
// StrictEffectType). For reference nodes (identifiers and property accesses)
// it inspects the referenced symbol's declared type, which is cheap compared
// to the flow analysis performed by GetTypeAtLocation. Flow narrowing can
// only refine the declared type — select union constituents, narrow
// any/unknown, or intersect it — so a declared type that conclusively
// contains no possibly-Effect constituent can never produce a strict-Effect
// flow type.
//
// It returns true ("cannot rule out") for every other node kind and whenever
// the answer is not conclusively negative, so whole-file walker rules may use
// a false result to skip expensive GetTypeAtLocation queries without ever
// missing a strict Effect type.
func (tp *TypeParser) NodeCouldBeStrictEffect(node *ast.Node) bool {
	if tp == nil || tp.checker == nil || node == nil {
		return true
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
			if couldBeNamed(member, names, depth+1) {
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
	sym := t.Symbol()
	if sym == nil {
		return true
	}
	return names[sym.Name]
}
