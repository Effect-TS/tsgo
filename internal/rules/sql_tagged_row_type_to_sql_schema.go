package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

var SqlTaggedRowTypeToSqlSchema = rule.Rule{
	Name:            "sqlTaggedRowTypeToSqlSchema",
	Group:           "style",
	Description:     "Suggests SqlSchema instead of an explicit row type argument on Effect's sql tagged template",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.The_sql_tagged_template_only_asserts_its_row_type_0_and_does_not_validate_it_at_runtime_use_SqlSchema_findAll_findOne_findOneOption_findNonEmpty_void_with_a_Schema_for_Result_to_decode_rows_effect_sqlTaggedRowTypeToSqlSchema.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic
		var walk ast.Visitor
		walk = func(node *ast.Node) bool {
			if node == nil {
				return false
			}

			if node.Kind == ast.KindTaggedTemplateExpression {
				tagged := node.AsTaggedTemplateExpression()
				if tagged.TypeArguments != nil && len(tagged.TypeArguments.Nodes) > 0 &&
					ctx.TypeParser.IsEffectSqlStatementConstructorTaggedTemplate(node) {
					rowType := ctx.Checker.TypeToString(ctx.Checker.GetTypeFromTypeNode(tagged.TypeArguments.Nodes[0]))
					diags = append(diags, ctx.NewDiagnostic(
						ctx.SourceFile,
						ctx.GetErrorRange(tagged.Tag),
						tsdiag.The_sql_tagged_template_only_asserts_its_row_type_0_and_does_not_validate_it_at_runtime_use_SqlSchema_findAll_findOne_findOneOption_findNonEmpty_void_with_a_Schema_for_Result_to_decode_rows_effect_sqlTaggedRowTypeToSqlSchema,
						nil,
						rowType,
					))
				}
			}

			//ets:ast-walk finds tagged template expressions, which are not calls and never become piping-flow steps
			node.ForEachChild(walk)
			return false
		}

		walk(ctx.SourceFile.AsNode())
		return diags
	},
}
