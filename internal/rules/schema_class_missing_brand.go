package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

var SchemaClassMissingBrand = rule.Rule{
	Name:            "schemaClassMissingBrand",
	Group:           "correctness",
	Description:     "Warns when Schema classes, errors, or Opaque classes omit the brand type argument",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.Add_readonly_brand_Colon_unique_symbol_as_the_second_type_argument_Without_a_brand_TypeScript_may_accept_structurally_compatible_values_where_this_class_instance_type_is_expected_effect_schemaClassMissingBrand.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		if ctx.TypeParser.SupportedEffectVersion() != typeparser.EffectMajorV4 {
			return nil
		}
		var diagnostics []*ast.Diagnostic
		var walk ast.Visitor
		walk = func(node *ast.Node) bool {
			if node.Kind == ast.KindClassDeclaration || node.Kind == ast.KindClassExpression {
				if self := schemaClassMissingBrandSelf(ctx.TypeParser, node); self != nil {
					diagnostics = append(diagnostics, ctx.NewDiagnostic(
						ctx.SourceFile,
						ctx.GetErrorRange(self),
						tsdiag.Add_readonly_brand_Colon_unique_symbol_as_the_second_type_argument_Without_a_brand_TypeScript_may_accept_structurally_compatible_values_where_this_class_instance_type_is_expected_effect_schemaClassMissingBrand,
						nil,
					))
				}
			}
			node.ForEachChild(walk)
			return false
		}
		walk(ctx.SourceFile.AsNode())
		return diagnostics
	},
}

func schemaClassMissingBrandSelf(tp *typeparser.TypeParser, classNode *ast.Node) *ast.Node {
	var self, brand *ast.Node
	if result := tp.ExtendsSchemaClass(classNode); result != nil {
		self, brand = result.SelfTypeNode, result.BrandTypeNode
	} else if result := tp.ExtendsSchemaTaggedClass(classNode); result != nil {
		self, brand = result.SelfTypeNode, result.BrandTypeNode
	} else if result := tp.ExtendsSchemaError(classNode); result != nil {
		self, brand = result.SelfTypeNode, result.BrandTypeNode
	} else if result := tp.ExtendsSchemaTaggedError(classNode); result != nil {
		self, brand = result.SelfTypeNode, result.BrandTypeNode
	} else if result := tp.ExtendsSchemaOpaque(classNode); result != nil {
		self, brand = result.SelfTypeNode, result.BrandTypeNode
	}
	if brand != nil {
		return nil
	}
	return self
}
