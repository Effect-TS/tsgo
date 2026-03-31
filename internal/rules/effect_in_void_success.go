// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
)

// EffectInVoidSuccess detects nested Effects in void success channels.
// When an Effect has void as its success type but the actual value contains
// an Effect type, this likely means a nested Effect<Effect<...>> that won't be executed.
var EffectInVoidSuccess = rule.Rule{
	Name:            "effectInVoidSuccess",
	Group:           "antipattern",
	Description:     "Detects nested Effects in void success channels that may cause unexecuted effects",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.There_is_a_nested_0_in_the_void_success_channel_beware_that_this_could_lead_to_nested_Effect_Effect_that_won_t_be_executed_effect_effectInVoidSuccess.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic

		for _, entry := range ctx.TypeParser.ExpectedAndRealTypes(ctx.SourceFile) {
			if entry.ExpectedType == entry.RealType {
				continue
			}

			expectedEffect := ctx.TypeParser.EffectType(entry.ExpectedType, entry.Node)
			realEffect := ctx.TypeParser.EffectType(entry.RealType, entry.ValueNode)

			if expectedEffect == nil || realEffect == nil {
				continue
			}

			// Check if the expected Effect's success type is void
			if expectedEffect.A.Flags()&checker.TypeFlagsVoid == 0 {
				continue
			}

			// Unroll the real Effect's success type into union members
			// and check if any member is strictly an Effect type
			members := typeparser.UnrollUnionMembers(realEffect.A)
			voidedEffect := findFirstStrictEffect(ctx.TypeParser, ctx.Checker, members, entry.Node)
			if voidedEffect != nil {
				diag := ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(entry.Node), tsdiag.There_is_a_nested_0_in_the_void_success_channel_beware_that_this_could_lead_to_nested_Effect_Effect_that_won_t_be_executed_effect_effectInVoidSuccess, nil, ctx.Checker.TypeToString(voidedEffect))
				diags = append(diags, diag)
			}
		}

		return diags
	},
}

// findFirstStrictEffect returns the first type in the slice that is strictly an Effect type,
// or nil if none are found. This mirrors the Nano.firstSuccessOf pattern in the TS reference.
func findFirstStrictEffect(tp *typeparser.TypeParser, _ *checker.Checker, types []*checker.Type, atLocation *ast.Node) *checker.Type {
	for _, t := range types {
		if tp.StrictIsEffectType(t, atLocation) {
			return t
		}
	}
	return nil
}
