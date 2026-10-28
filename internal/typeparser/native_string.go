package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

type nativeStringMember struct {
	name string
}

type nativeStringRegistry struct {
	instance map[string][]*ast.Symbol
}

func (tp *TypeParser) IsNativeStringMethodReference(node *ast.Node, methodName string) bool {
	return tp.nativeStringReference(node, nativeStringMember{methodName})
}

func (tp *TypeParser) getNativeStringRegistry() *nativeStringRegistry {
	if tp.links.nativeStringRegistry != nil {
		return tp.links.nativeStringRegistry
	}
	r := &nativeStringRegistry{
		instance: make(map[string][]*ast.Symbol),
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
	if symbol := tp.checker.ResolveName("String", nil, ast.SymbolFlagsType, false); symbol != nil {
		add(tp.checker.GetDeclaredTypeOfSymbol(symbol), r.instance)
	}
	tp.links.nativeStringRegistry = r
	return r
}

func (tp *TypeParser) nativeStringReference(node *ast.Node, requested nativeStringMember) bool {
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
	member := Cached(&tp.links.nativeStringMember, symbol, func() nativeStringMember {
		registry := tp.getNativeStringRegistry()
		var member nativeStringMember
		for _, root := range tp.checker.GetRootSymbols(symbol) {
			var matched nativeStringMember
			for _, canonical := range registry.instance[ast.TSGetSymbolName(root)] {
				if checker.Checker_getSymbolIfSameReference(tp.checker, root, canonical) != nil {
					matched = nativeStringMember{ast.TSGetSymbolName(root)}
					break
				}
			}
			if matched.name == "" || member.name != "" && matched != member {
				return nativeStringMember{}
			}
			member = matched
		}
		return member
	})
	return member == requested
}
