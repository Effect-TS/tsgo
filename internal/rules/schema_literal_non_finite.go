package rules

import (
	"math"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var SchemaLiteralNonFinite = rule.Rule{
	Name:            "schemaLiteralNonFinite",
	Group:           "correctness",
	Description:     "Reports statically known non-finite numbers passed to Schema literal constructors",
	DefaultSeverity: etscore.SeverityError,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.Schema_literal_values_must_be_finite_numbers_0_throws_during_schema_construction_Use_a_finite_literal_or_model_non_finite_values_with_a_refined_Schema_Number_effect_schemaLiteralNonFinite.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic
		var walk ast.Visitor
		walk = func(node *ast.Node) bool {
			if node == nil {
				return false
			}

			if node.Kind == ast.KindCallExpression {
				call := node.AsCallExpression()
				if call != nil && call.Arguments != nil && len(call.Arguments.Nodes) > 0 {
					var values []*ast.Node
					switch {
					case ctx.TypeParser.IsNodeReferenceToEffectSchemaModuleApi(call.Expression, "Literals"):
						arg := call.Arguments.Nodes[0]
						if arg != nil && arg.Kind == ast.KindArrayLiteralExpression {
							values = arg.AsArrayLiteralExpression().Elements.Nodes
						}
					case ctx.TypeParser.IsNodeReferenceToEffectSchemaModuleApi(call.Expression, "Literal"),
						ctx.TypeParser.IsNodeReferenceToEffectSchemaModuleApi(call.Expression, "tag"):
						values = call.Arguments.Nodes[:1]
					}

					for _, value := range values {
						if n, ok := ctx.TypeParser.EvaluateConstantNumber(value, value); ok && (math.IsInf(n, 0) || math.IsNaN(n)) {
							diags = append(diags, ctx.NewDiagnostic(
								ctx.SourceFile,
								scanner.GetErrorRangeForNode(ctx.SourceFile, value),
								tsdiag.Schema_literal_values_must_be_finite_numbers_0_throws_during_schema_construction_Use_a_finite_literal_or_model_non_finite_values_with_a_refined_Schema_Number_effect_schemaLiteralNonFinite,
								nil,
								scanner.GetTextOfNode(value),
							))
						}
					}
				}
			}

			node.ForEachChild(walk)
			return false
		}

		walk(ctx.SourceFile.AsNode())
		return diags
	},
}
