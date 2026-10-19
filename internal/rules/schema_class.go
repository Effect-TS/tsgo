package rules

import (
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

var schemaClassInstanceDetectors = []func(*typeparser.TypeParser, *ast.Node) (*ast.Node, *ast.Node){
	func(tp *typeparser.TypeParser, node *ast.Node) (*ast.Node, *ast.Node) {
		if result := tp.ExtendsSchemaClass(node); result != nil {
			return result.SelfTypeNode(), result.BrandTypeNode()
		}
		return nil, nil
	},
	func(tp *typeparser.TypeParser, node *ast.Node) (*ast.Node, *ast.Node) {
		if result := tp.ExtendsSchemaTaggedClass(node); result != nil {
			return result.SelfTypeNode(), result.BrandTypeNode()
		}
		return nil, nil
	},
	func(tp *typeparser.TypeParser, node *ast.Node) (*ast.Node, *ast.Node) {
		if result := tp.ExtendsSchemaError(node); result != nil {
			return result.SelfTypeNode(), result.BrandTypeNode()
		}
		return nil, nil
	},
	func(tp *typeparser.TypeParser, node *ast.Node) (*ast.Node, *ast.Node) {
		if result := tp.ExtendsSchemaTaggedError(node); result != nil {
			return result.SelfTypeNode(), result.BrandTypeNode()
		}
		return nil, nil
	},
}

func schemaClassInstanceSelfAndBrand(tp *typeparser.TypeParser, node *ast.Node) (*ast.Node, *ast.Node) {
	for _, detect := range schemaClassInstanceDetectors {
		if self, brand := detect(tp, node); self != nil {
			return self, brand
		}
	}
	return nil, nil
}
