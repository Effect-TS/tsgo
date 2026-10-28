package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

// schemaClassInstanceSelfAndBrand returns the Self and brand type arguments of a Schema
// class heritage, along with its kind (one of the etscore.BrandedSchemaClass* names).
func schemaClassInstanceSelfAndBrand(tp *typeparser.TypeParser, node *ast.Node) (*ast.Node, *ast.Node, string) {
	if result := tp.ExtendsSchemaClass(node); result != nil {
		return result.SelfTypeNode(), result.BrandTypeNode(), etscore.BrandedSchemaClassClass
	}
	if result := tp.ExtendsSchemaTaggedClass(node); result != nil {
		return result.SelfTypeNode(), result.BrandTypeNode(), etscore.BrandedSchemaClassTaggedClass
	}
	if result := tp.ExtendsSchemaError(node); result != nil {
		return result.SelfTypeNode(), result.BrandTypeNode(), etscore.BrandedSchemaClassError
	}
	if result := tp.ExtendsSchemaTaggedError(node); result != nil {
		return result.SelfTypeNode(), result.BrandTypeNode(), etscore.BrandedSchemaClassTaggedError
	}
	return nil, nil, ""
}
