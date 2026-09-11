package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var CryptoRandomUUID = rule.Rule{
	Name:            "cryptoRandomUUID",
	Group:           "effectNative",
	Description:     "Warns when using crypto.randomUUID() outside Effect generators instead of the Effect Crypto module",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.This_code_uses_crypto_randomUUID_prefer_the_Effect_Crypto_module_instead_effect_cryptoRandomUUID.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runCryptoRandomUUID(ctx, false)
	},
}

var CryptoRandomUUIDInEffect = rule.Rule{
	Name:            "cryptoRandomUUIDInEffect",
	Group:           "effectNative",
	Description:     "Warns when using crypto.randomUUID() inside Effect generators instead of the Effect Crypto module",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.This_Effect_code_uses_crypto_randomUUID_prefer_the_Effect_Crypto_module_instead_effect_cryptoRandomUUIDInEffect.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runCryptoRandomUUID(ctx, true)
	},
}

func runCryptoRandomUUID(ctx *rule.Context, checkInEffect bool) []*ast.Diagnostic {
	message := tsdiag.This_code_uses_crypto_randomUUID_prefer_the_Effect_Crypto_module_instead_effect_cryptoRandomUUID
	if checkInEffect {
		message = tsdiag.This_Effect_code_uses_crypto_randomUUID_prefer_the_Effect_Crypto_module_instead_effect_cryptoRandomUUIDInEffect
	}

	var diags []*ast.Diagnostic
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindCallExpression {
			call := node.AsCallExpression()
			inEffect := ctx.TypeParser.GetEffectContextFlags(node)&typeparser.EffectContextFlagInEffect != 0
			if inEffect == checkInEffect {
				if ctx.TypeParser.IsNodeReferenceToGlobalMember(call.Expression, "crypto", "randomUUID") {
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
