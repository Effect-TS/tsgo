package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

type nativeArrayNamespace uint8

const (
	nativeArrayInstance nativeArrayNamespace = iota + 1
	nativeArrayConstructor
)

type nativeArrayMember struct {
	namespace nativeArrayNamespace
	name      string
}

type nativeArrayRegistry struct {
	instance    map[string][]*ast.Symbol
	constructor map[string][]*ast.Symbol
}

// IsArrayType recognizes mutable and readonly arrays, but not tuples.
// It does not normalize unions or type parameter constraints.
func (tp *TypeParser) IsArrayType(t *checker.Type) bool {
	return t != nil && checker.Checker_isArrayType(tp.checker, t)
}

// IsNativeArrayMethodReference compares the reference's roots with the named
// Array or ReadonlyArray member, even when a mapped type renames the access.
func (tp *TypeParser) IsNativeArrayMethodReference(node *ast.Node, methodName string) bool {
	return tp.nativeArrayReference(node, nativeArrayMember{nativeArrayInstance, methodName})
}

// IsNativeArrayConstructorMethodReference compares the reference's roots with
// the named Array constructor member.
func (tp *TypeParser) IsNativeArrayConstructorMethodReference(node *ast.Node, methodName string) bool {
	return tp.nativeArrayReference(node, nativeArrayMember{nativeArrayConstructor, methodName})
}

func (tp *TypeParser) getNativeArrayRegistry() *nativeArrayRegistry {
	if tp.links.nativeArrayRegistry != nil {
		return tp.links.nativeArrayRegistry
	}
	r := &nativeArrayRegistry{
		instance:    make(map[string][]*ast.Symbol),
		constructor: make(map[string][]*ast.Symbol),
	}
	add := func(t *checker.Type, members map[string][]*ast.Symbol) {
		owner := t.Symbol()
		if owner == nil {
			return
		}
		for _, symbol := range ast.TSGetSymbolTableEntries(checker.Checker_getMembersOfSymbol(tp.checker, owner)) {
			if ast.TSGetSymbolFlags(symbol)&ast.SymbolFlagsValue == 0 || len(ast.TSGetSymbolDeclarations(symbol)) == 0 {
				continue
			}
			libraryOnly := true
			for _, declaration := range ast.TSGetSymbolDeclarations(symbol) {
				sf := ast.GetSourceFileOfNode(declaration)
				if sf == nil || !tp.program.IsSourceFileDefaultLibrary(ast.SourceFilePath(sf)) {
					libraryOnly = false
					break
				}
			}
			if libraryOnly {
				members[ast.TSGetSymbolName(symbol)] = append(members[ast.TSGetSymbolName(symbol)], tp.checker.GetRootSymbols(symbol)...)
			}
		}
	}
	for _, name := range []string{"Array", "ReadonlyArray"} {
		if symbol := tp.checker.ResolveName(name, nil, ast.SymbolFlagsType, false); symbol != nil {
			add(tp.checker.GetDeclaredTypeOfSymbol(symbol), r.instance)
		}
	}
	if symbol := tp.checker.ResolveName("Array", nil, ast.SymbolFlagsValue, false); symbol != nil {
		add(tp.checker.GetTypeOfSymbol(symbol), r.constructor)
	}
	tp.links.nativeArrayRegistry = r
	return r
}

func (tp *TypeParser) nativeArrayReference(node *ast.Node, requested nativeArrayMember) bool {
	if node == nil || requested.name == "" {
		return false
	}
	var symbol *ast.Symbol
	switch node.Kind {
	case ast.KindPropertyAccessExpression:
		symbol = tp.GetSymbolAtLocation(node)
	case ast.KindElementAccessExpression:
		access := node.AsElementAccessExpression()
		if access.ArgumentExpression == nil || !ast.IsStringLiteralLike(access.ArgumentExpression) {
			return false
		}
		symbol = tp.GetSymbolAtLocation(access.ArgumentExpression)
		if symbol == nil {
			receiverType := tp.checker.GetNonNullableType(tp.GetTypeAtLocation(access.Expression))
			symbol = tp.checker.GetPropertyOfType(receiverType, access.ArgumentExpression.Text())
		}
	default:
		return false
	}
	if symbol == nil {
		return false
	}
	member := Cached(&tp.links.nativeArrayMember, symbol, func() nativeArrayMember {
		registry := tp.getNativeArrayRegistry()
		var member nativeArrayMember
		for _, root := range tp.checker.GetRootSymbols(symbol) {
			var matched nativeArrayMember
			for _, canonical := range registry.instance[ast.TSGetSymbolName(root)] {
				if checker.Checker_getSymbolIfSameReference(tp.checker, root, canonical) != nil {
					matched = nativeArrayMember{nativeArrayInstance, ast.TSGetSymbolName(root)}
					break
				}
			}
			if matched.namespace == 0 {
				for _, canonical := range registry.constructor[ast.TSGetSymbolName(root)] {
					if checker.Checker_getSymbolIfSameReference(tp.checker, root, canonical) != nil {
						matched = nativeArrayMember{nativeArrayConstructor, ast.TSGetSymbolName(root)}
						break
					}
				}
			}
			if matched.namespace == 0 || member.namespace != 0 && matched != member {
				return nativeArrayMember{}
			}
			member = matched
		}
		return member
	})
	return member == requested
}
