package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// EffectDoNotation suggests using Effect.gen or Effect.fn instead of Effect.Do.
var EffectDoNotation = rule.Rule{
	Name:            "effectDoNotation",
	Group:           "style",
	Description:     "Suggests using Effect.gen or Effect.fn instead of the Effect.Do notation helpers",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.This_uses_the_Effect_do_emulation_Effect_gen_or_Effect_fn_achieve_the_same_result_with_native_JS_scopes_effect_effectDoNotation.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeEffectDoNotation(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, match := range matches {
			diags[i] = ctx.NewDiagnostic(
				match.SourceFile,
				match.Location,
				tsdiag.This_uses_the_Effect_do_emulation_Effect_gen_or_Effect_fn_achieve_the_same_result_with_native_JS_scopes_effect_effectDoNotation,
				nil,
			)
		}
		return diags
	},
}

type EffectDoNotationMatch struct {
	SourceFile *ast.SourceFile
	Location   core.TextRange
	Node       *ast.Node
}

// AnalyzeEffectDoNotation finds all references to Effect.Do-style helpers.
func AnalyzeEffectDoNotation(tp *typeparser.TypeParser, _ *checker.Checker, sf *ast.SourceFile) []EffectDoNotationMatch {
	var matches []EffectDoNotationMatch

	var walk ast.Visitor
	walk = func(n *ast.Node) bool {
		if n == nil {
			return false
		}

		if tp.IsNodeReferenceToEffectModuleApi(n, "Do") {
			matches = append(matches, EffectDoNotationMatch{
				SourceFile: sf,
				Location:   scanner.GetErrorRangeForNode(sf, n),
				Node:       n,
			})
		} else {
			n.ForEachChild(walk)
		}

		return false
	}

	walk(sf.AsNode())
	return matches
}
