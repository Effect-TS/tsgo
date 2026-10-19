package rules

import (
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

func schemaClassInstanceSelfAndBrand(tp *typeparser.TypeParser, node *ast.Node) (*ast.Node, *ast.Node) {
	if result := tp.ExtendsSchemaClass(node); result != nil {
		return result.SelfTypeNode(), result.BrandTypeNode()
	}
	if result := tp.ExtendsSchemaTaggedClass(node); result != nil {
		return result.SelfTypeNode(), result.BrandTypeNode()
	}
	if result := tp.ExtendsSchemaError(node); result != nil {
		return result.SelfTypeNode(), result.BrandTypeNode()
	}
	if result := tp.ExtendsSchemaTaggedError(node); result != nil {
		return result.SelfTypeNode(), result.BrandTypeNode()
	}
	return nil, nil
}
