package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

// SchemaSync discourages synchronous Schema decoding and encoding in any context.
var SchemaSync = rule.Rule{
	Name:            "schemaSync",
	Group:           "effectNative",
	Description:     "Suggests Effect-based Schema decoding and encoding instead of synchronous methods",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.X_0_executes_synchronously_Use_Schema_1_to_compose_this_operation_through_Effect_without_throwing_effect_schemaSync.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		methods := syncToEffectMethodV3
		if ctx.TypeParser.SupportedEffectVersion() == typeparser.EffectMajorV4 {
			methods = syncToEffectMethodV4
		}

		var diags []*ast.Diagnostic
		var walk ast.Visitor
		walk = func(node *ast.Node) bool {
			if node == nil {
				return false
			}
			if node.Kind == ast.KindCallExpression {
				callee := node.AsCallExpression().Expression
				reference := ast.SkipParentheses(callee)
				if reference.Kind == ast.KindElementAccessExpression {
					reference = reference.AsElementAccessExpression().ArgumentExpression
				}
				for syncName, effectName := range methods {
					if ctx.TypeParser.IsNodeReferenceToEffectSchemaModuleApi(reference, syncName) ||
						ctx.TypeParser.IsNodeReferenceToEffectParseResultModuleApi(reference, syncName) ||
						ctx.TypeParser.IsNodeReferenceToEffectSchemaParserModuleApi(reference, syncName) {
						diags = append(diags, ctx.NewDiagnostic(
							ctx.SourceFile,
							ctx.GetErrorRange(callee),
							tsdiag.X_0_executes_synchronously_Use_Schema_1_to_compose_this_operation_through_Effect_without_throwing_effect_schemaSync,
							nil,
							scanner.GetSourceTextOfNodeFromSourceFile(ctx.SourceFile, callee, false),
							effectName,
						))
						break
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
