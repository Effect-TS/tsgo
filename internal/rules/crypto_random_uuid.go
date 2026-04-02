package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var CryptoRandomUUID = rule.Rule{
	Name:            "cryptoRandomUUID",
	Group:           "effectNative",
	Description:     "Warns when using crypto.randomUUID() outside Effect generators instead of the Effect Random module, which uses Effect-injected randomness rather than the crypto module behind the scenes",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.This_code_uses_crypto_randomUUID_prefer_the_Effect_Random_module_instead_because_it_uses_Effect_injected_randomness_rather_than_the_crypto_module_behind_the_scenes_effect_cryptoRandomUUID.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runCryptoRandomUUID(ctx, false)
	},
}

var CryptoRandomUUIDInEffect = rule.Rule{
	Name:            "cryptoRandomUUIDInEffect",
	Group:           "effectNative",
	Description:     "Warns when using crypto.randomUUID() inside Effect generators instead of the Effect Random module, which uses Effect-injected randomness rather than the crypto module behind the scenes",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.This_Effect_code_uses_crypto_randomUUID_prefer_the_Effect_Random_module_instead_because_it_uses_Effect_injected_randomness_rather_than_the_crypto_module_behind_the_scenes_effect_cryptoRandomUUIDInEffect.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runCryptoRandomUUID(ctx, true)
	},
}

func runCryptoRandomUUID(ctx *rule.Context, checkInEffect bool) []*ast.Diagnostic {
	cryptoSymbol := ctx.Checker.ResolveName("crypto", nil, ast.SymbolFlagsValue, false)
	if cryptoSymbol == nil {
		return nil
	}

	message := tsdiag.This_code_uses_crypto_randomUUID_prefer_the_Effect_Random_module_instead_because_it_uses_Effect_injected_randomness_rather_than_the_crypto_module_behind_the_scenes_effect_cryptoRandomUUID
	if checkInEffect {
		message = tsdiag.This_Effect_code_uses_crypto_randomUUID_prefer_the_Effect_Random_module_instead_because_it_uses_Effect_injected_randomness_rather_than_the_crypto_module_behind_the_scenes_effect_cryptoRandomUUIDInEffect
	}

	var diags []*ast.Diagnostic
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindCallExpression {
			call := node.AsCallExpression()
			inEffect := ctx.TypeParser.GetEffectContextFlags(node)&typeparser.EffectContextFlagCanYieldEffect != 0
			if inEffect == checkInEffect {
				if receiver := cryptoRandomUUIDReceiver(call); receiver != nil {
					if ctx.TypeParser.ResolveToGlobalSymbol(ctx.Checker.GetSymbolAtLocation(receiver)) == cryptoSymbol {
						diags = append(diags, ctx.NewDiagnostic(
							ctx.SourceFile,
							scanner.GetErrorRangeForNode(ctx.SourceFile, node),
							message,
							nil,
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
}

func cryptoRandomUUIDReceiver(call *ast.CallExpression) *ast.Node {
	if call == nil || call.Expression.Kind != ast.KindPropertyAccessExpression {
		return nil
	}
	prop := call.Expression.AsPropertyAccessExpression()
	if prop.Name().Text() != "randomUUID" {
		return nil
	}
	return prop.Expression
}
