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
		if len(ctx.Options.GetBrandedSchemaClasses()) == 0 {
			return nil
		}
		var diagnostics []*ast.Diagnostic
		var walk ast.Visitor
		walk = func(node *ast.Node) bool {
			if node.Kind == ast.KindClassDeclaration || node.Kind == ast.KindClassExpression {
				if self := schemaClassMissingBrandSelf(ctx.TypeParser, ctx.Options, node); self != nil {
					diagnostics = append(diagnostics, ctx.NewDiagnostic(
						ctx.SourceFile,
						ctx.GetErrorRange(self),
						tsdiag.Add_readonly_brand_Colon_unique_symbol_as_the_second_type_argument_Without_a_brand_TypeScript_may_accept_structurally_compatible_values_where_this_class_instance_type_is_expected_effect_schemaClassMissingBrand,
						nil,
					))
				}
			}
			//ets:ast-walk finds class declarations and expressions extending Schema.Class
			node.ForEachChild(walk)
			return false
		}
		walk(ctx.SourceFile.AsNode())
		return diagnostics
	},
}

func schemaClassMissingBrandSelf(tp *typeparser.TypeParser, options *etscore.ResolvedEffectPluginOptions, classNode *ast.Node) *ast.Node {
	self, brand, kind := schemaClassInstanceSelfAndBrand(tp, classNode)
	if self == nil {
		if result := tp.ExtendsSchemaOpaque(classNode); result != nil {
			self, brand, kind = result.SelfTypeNode(), result.BrandTypeNode(), etscore.BrandedSchemaClassOpaque
		}
	}
	if brand != nil || !options.BrandedSchemaClassesIncludes(kind) {
		return nil
	}
	return self
}
