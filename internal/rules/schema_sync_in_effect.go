package rules

import (
	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// syncToEffectMethodV3 maps Schema sync method names to their Effect-based V3 equivalents.
var syncToEffectMethodV3 = map[string]string{
	"decodeSync":        "decode",
	"decodeUnknownSync": "decodeUnknown",
	"encodeSync":        "encode",
	"encodeUnknownSync": "encodeUnknown",
}

// syncToEffectMethodV4 maps Schema sync method names to their Effect-based V4 equivalents.
var syncToEffectMethodV4 = map[string]string{
	"decodeSync":        "decodeEffect",
	"decodeUnknownSync": "decodeUnknownEffect",
	"encodeSync":        "encodeEffect",
	"encodeUnknownSync": "encodeUnknownEffect",
}

// SchemaSyncInEffect detects Schema sync methods (decodeSync, encodeSync, etc.) used inside
// Effect generators and suggests using the Effect-based variants instead.
var SchemaSyncInEffect = rule.Rule{
	Name:            "schemaSyncInEffect",
	Group:           "antipattern",
	Description:     "Suggests using Effect-based Schema methods instead of sync methods inside Effect generators",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3"},
	Codes:           []int32{tsdiag.Using_0_inside_an_Effect_generator_is_not_recommended_Use_Schema_1_instead_to_get_properly_typed_error_channel_effect_schemaSyncInEffect.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		version := typeparser.SupportedEffectVersion(ctx.Checker)
		var syncToEffectMethod map[string]string
		if version == typeparser.EffectMajorV4 {
			syncToEffectMethod = syncToEffectMethodV4
		} else {
			syncToEffectMethod = syncToEffectMethodV3
		}

		var diags []*ast.Diagnostic

		var walk ast.Visitor
		walk = func(n *ast.Node) bool {
			if n == nil {
				return false
			}

			if n.Kind == ast.KindCallExpression {
				if d := checkSchemaSyncInEffect(ctx, n, syncToEffectMethod); d != nil {
					diags = append(diags, d)
				}
			}

			n.ForEachChild(walk)
			return false
		}

		walk(ctx.SourceFile.AsNode())
		return diags
	},
}

// checkSchemaSyncInEffect checks a single call expression for Schema sync methods inside an Effect generator.
func checkSchemaSyncInEffect(ctx *rule.Context, node *ast.Node, syncToEffectMethod map[string]string) *ast.Diagnostic {
	if node.Kind != ast.KindCallExpression {
		return nil
	}
	call := node.AsCallExpression()

	callee := call.Expression

	// Check if the callee is one of the Schema sync methods (try both ParseResult and SchemaParser modules)
	methodName := matchSchemaSyncMethod(ctx.Checker, callee, syncToEffectMethod)
	if methodName == "" {
		return nil
	}

	if typeparser.GetEffectContextFlags(ctx.Checker, node)&typeparser.EffectContextFlagCanYieldEffect == 0 {
		return nil
	}

	genFn := typeparser.GetEffectYieldGeneratorFunction(ctx.Checker, node)
	if genFn == nil {
		return nil
	}

	// Check that the generator body has at least one statement
	if genFn.Body == nil || genFn.Body.Kind != ast.KindBlock {
		return nil
	}
	block := genFn.Body.AsBlock()
	if block.Statements == nil || len(block.Statements.Nodes) == 0 {
		return nil
	}

	calleeText := scanner.GetSourceTextOfNodeFromSourceFile(ctx.SourceFile, callee, false)
	effectMethodName := syncToEffectMethod[methodName]

	return ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(callee), tsdiag.Using_0_inside_an_Effect_generator_is_not_recommended_Use_Schema_1_instead_to_get_properly_typed_error_channel_effect_schemaSyncInEffect, nil, calleeText, effectMethodName)
}

// matchSchemaSyncMethod checks if the node references one of the Schema sync methods via
// either the ParseResult module (V3) or the SchemaParser module (V4).
func matchSchemaSyncMethod(c *checker.Checker, node *ast.Node, syncToEffectMethod map[string]string) string {
	for methodName := range syncToEffectMethod {
		if typeparser.IsNodeReferenceToEffectParseResultModuleApi(c, node, methodName) {
			return methodName
		}
		if typeparser.IsNodeReferenceToEffectSchemaParserModuleApi(c, node, methodName) {
			return methodName
		}
	}
	return ""
}
