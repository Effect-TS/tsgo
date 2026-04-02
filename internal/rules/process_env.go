package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var ProcessEnv = rule.Rule{
	Name:            "processEnv",
	Group:           "effectNative",
	Description:     "Warns when reading process.env outside Effect generators instead of using Effect Config",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_code_reads_from_process_env_environment_configuration_is_represented_through_Config_from_Effect_effect_processEnv.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runProcessEnv(ctx, false)
	},
}

var ProcessEnvInEffect = rule.Rule{
	Name:            "processEnvInEffect",
	Group:           "effectNative",
	Description:     "Warns when reading process.env inside Effect generators instead of using Effect Config",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_Effect_code_reads_from_process_env_environment_configuration_in_Effect_code_is_represented_through_Config_from_Effect_effect_processEnvInEffect.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runProcessEnv(ctx, true)
	},
}

func runProcessEnv(ctx *rule.Context, checkInEffect bool) []*ast.Diagnostic {
	processSymbol := ctx.Checker.ResolveName("process", nil, ast.SymbolFlagsValue, false)
	if processSymbol == nil {
		return nil
	}

	message := tsdiag.This_code_reads_from_process_env_environment_configuration_is_represented_through_Config_from_Effect_effect_processEnv
	if checkInEffect {
		message = tsdiag.This_Effect_code_reads_from_process_env_environment_configuration_in_Effect_code_is_represented_through_Config_from_Effect_effect_processEnvInEffect
	}

	var diags []*ast.Diagnostic
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}

		inEffect := ctx.TypeParser.GetEffectContextFlags(node)&typeparser.EffectContextFlagCanYieldEffect != 0
		if inEffect == checkInEffect {
			if processNode := processEnvRoot(node); processNode != nil {
				if ctx.TypeParser.ResolveToGlobalSymbol(ctx.Checker.GetSymbolAtLocation(processNode)) == processSymbol {
					diags = append(diags, ctx.NewDiagnostic(
						ctx.SourceFile,
						scanner.GetErrorRangeForNode(ctx.SourceFile, node),
						message,
						nil,
					))
				}
			}
		}

		node.ForEachChild(walk)
		return false
	}

	walk(ctx.SourceFile.AsNode())

	return diags
}

func processEnvRoot(node *ast.Node) *ast.Node {
	if node == nil {
		return nil
	}
	if node.Kind != ast.KindPropertyAccessExpression && node.Kind != ast.KindElementAccessExpression {
		return nil
	}
	access := node.Expression()
	if !isEnvPropertyAccess(access) {
		return nil
	}
	return access.Expression()
}

func isEnvPropertyAccess(node *ast.Node) bool {
	if node == nil || node.Kind != ast.KindPropertyAccessExpression {
		return false
	}
	return node.AsPropertyAccessExpression().Name().Text() == "env"
}
