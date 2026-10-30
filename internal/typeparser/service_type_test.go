package typeparser

import (
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

func findClassTypeByName(t *testing.T, c *checker.Checker, sf *ast.SourceFile, className string) (*ast.Node, *checker.Type) {
	t.Helper()

	var foundName *ast.Node
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil || foundName != nil {
			return false
		}
		if node.Kind == ast.KindClassDeclaration {
			classDecl := node.AsClassDeclaration()
			if classDecl != nil && node.Name() != nil && node.Name().Text() == className {
				foundName = node.Name()
				return false
			}
		}
		node.ForEachChild(walk)
		return false
	}
	walk(sf.AsNode())
	if foundName == nil {
		t.Fatalf("class %q not found", className)
	}

	tp := NewTypeParser(c.Program(), c)
	classSym := tp.GetSymbolAtLocation(foundName)
	if classSym == nil {
		t.Fatalf("symbol for class %q not found", className)
	}
	classType := c.GetTypeOfSymbolAtLocation(classSym, foundName)
	if classType == nil {
		t.Fatalf("type for class %q not found", className)
	}
	return foundName, classType
}
