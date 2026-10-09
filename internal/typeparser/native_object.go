package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

type nativeObjectNamespace uint8

const (
	nativeObjectInstance nativeObjectNamespace = iota + 1
	nativeObjectConstructor
)

type nativeObjectMember struct {
	namespace nativeObjectNamespace
	name      string
}

type nativeObjectRegistry struct {
	instance    map[string][]*ast.Symbol
	constructor map[string][]*ast.Symbol
}

// IsNativeObjectMethodReference compares the reference's roots with the named
// Object prototype member, even when a mapped type renames the access.
func (tp *TypeParser) IsNativeObjectMethodReference(node *ast.Node, methodName string) bool {
	return tp.nativeObjectReference(node, nativeObjectMember{nativeObjectInstance, methodName})
}

// IsNativeObjectConstructorMethodReference compares the reference's roots with
// the named Object constructor member.
func (tp *TypeParser) IsNativeObjectConstructorMethodReference(node *ast.Node, methodName string) bool {
	return tp.nativeObjectReference(node, nativeObjectMember{nativeObjectConstructor, methodName})
}

func (tp *TypeParser) getNativeObjectRegistry() *nativeObjectRegistry {
	if tp.links.nativeObjectRegistry != nil {
		return tp.links.nativeObjectRegistry
	}
	r := &nativeObjectRegistry{
		instance:    make(map[string][]*ast.Symbol),
		constructor: make(map[string][]*ast.Symbol),
	}
	add := func(t *checker.Type, members map[string][]*ast.Symbol) {
		owner := t.Symbol()
		if owner == nil {
			return
		}
		for _, symbol := range checker.Checker_getMembersOfSymbol(tp.checker, owner) {
			if symbol.Flags&ast.SymbolFlagsValue == 0 || len(symbol.Declarations) == 0 {
				continue
			}
			libraryOnly := true
			for _, declaration := range symbol.Declarations {
				sf := ast.GetSourceFileOfNode(declaration)
				if sf == nil || !tp.program.IsSourceFileDefaultLibrary(ast.SourceFilePath(sf)) {
					libraryOnly = false
					break
				}
			}
			if libraryOnly {
				members[symbol.Name] = append(members[symbol.Name], tp.checker.GetRootSymbols(symbol)...)
			}
		}
	}
	if symbol := tp.checker.ResolveName("Object", nil, ast.SymbolFlagsType, false); symbol != nil {
		add(tp.checker.GetDeclaredTypeOfSymbol(symbol), r.instance)
	}
	if symbol := tp.checker.ResolveName("Object", nil, ast.SymbolFlagsValue, false); symbol != nil {
		add(tp.checker.GetTypeOfSymbol(symbol), r.constructor)
	}
	tp.links.nativeObjectRegistry = r
	return r
}

func (tp *TypeParser) nativeObjectReference(node *ast.Node, requested nativeObjectMember) bool {
	if node == nil || requested.name == "" {
		return false
	}
	var symbol *ast.Symbol
	switch node.Kind {
	case ast.KindPropertyAccessExpression:
		symbol = tp.checker.GetSymbolAtLocation(node)
	case ast.KindElementAccessExpression:
		access := node.AsElementAccessExpression()
		if access.ArgumentExpression == nil || !ast.IsStringLiteralLike(access.ArgumentExpression) {
			return false
		}
		symbol = tp.checker.GetSymbolAtLocation(access.ArgumentExpression)
		if symbol == nil {
			receiverType := tp.checker.GetNonNullableType(tp.checker.GetTypeAtLocation(access.Expression))
			symbol = tp.checker.GetPropertyOfType(receiverType, access.ArgumentExpression.Text())
		}
	default:
		return false
	}
	if symbol == nil {
		return false
	}
	member := Cached(&tp.links.nativeObjectMember, symbol, func() nativeObjectMember {
		registry := tp.getNativeObjectRegistry()
		var member nativeObjectMember
		for _, root := range tp.checker.GetRootSymbols(symbol) {
			var matched nativeObjectMember
			for _, canonical := range registry.instance[root.Name] {
				if checker.Checker_getSymbolIfSameReference(tp.checker, root, canonical) != nil {
					matched = nativeObjectMember{nativeObjectInstance, root.Name}
					break
				}
			}
			if matched.namespace == 0 {
				for _, canonical := range registry.constructor[root.Name] {
					if checker.Checker_getSymbolIfSameReference(tp.checker, root, canonical) != nil {
						matched = nativeObjectMember{nativeObjectConstructor, root.Name}
						break
					}
				}
			}
			if matched.namespace == 0 || member.namespace != 0 && matched != member {
				return nativeObjectMember{}
			}
			member = matched
		}
		return member
	})
	return member == requested
}
