// Package rules contains all Effect diagnostic rule implementations.
package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// unknownCatchApis lists the Effect module APIs that have a catch callback parameter.
var unknownCatchApis = []string{"tryPromise", "try", "tryMap", "tryMapPromise"}

// UnknownInEffectCatch detects when catch callbacks in Effect APIs return 'unknown' or 'any'
// instead of providing typed errors.
var UnknownInEffectCatch = rule.Rule{
	Name:            "unknownInEffectCatch",
	Group:           "antipattern",
	Description:     "Warns when catch callbacks return unknown instead of typed errors",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.The_catch_callback_in_0_returns_unknown_so_the_Effect_error_type_stays_untyped_A_specific_typed_error_preserves_error_channel_information_for_example_by_narrowing_the_value_or_wrapping_it_in_Data_TaggedError_effect_unknownInEffectCatch.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic

		var walk ast.Visitor
		walk = func(n *ast.Node) bool {
			if n == nil {
				return false
			}

			if n.Kind == ast.KindCallExpression {
				if diag := checkUnknownInEffectCatch(ctx, n); diag != nil {
					diags = append(diags, diag)
				}
			}

			n.ForEachChild(walk)
			return false
		}

		walk(ctx.SourceFile.AsNode())

		return diags
	},
}

// checkUnknownInEffectCatch checks a single call expression for the unknown-in-catch pattern.
func checkUnknownInEffectCatch(ctx *rule.Context, node *ast.Node) *ast.Diagnostic {
	if node.Kind != ast.KindCallExpression {
		return nil
	}
	call := node.AsCallExpression()

	callee := call.Expression
	if !isUnknownCatchCallee(ctx.TypeParser, ctx.Checker, callee) {
		return nil
	}

	sig := ctx.Checker.GetResolvedSignature(node)
	if sig == nil {
		return nil
	}

	params := sig.Parameters()
	if len(params) == 0 {
		return nil
	}

	paramType := ctx.Checker.GetTypeOfSymbolAtLocation(params[0], node)
	if paramType == nil {
		return nil
	}

	for _, objectType := range ctx.TypeParser.UnrollUnionMembers(paramType) {
		catchSymbol := ctx.Checker.GetPropertyOfType(objectType, "catch")
		if catchSymbol == nil {
			continue
		}

		catchType := ctx.Checker.GetTypeOfSymbolAtLocation(catchSymbol, node)
		if catchType == nil {
			continue
		}

		signatures := ctx.Checker.GetSignaturesOfType(catchType, checker.SignatureKindCall)
		if len(signatures) == 0 {
			continue
		}

		returnType := ctx.Checker.GetReturnTypeOfSignature(signatures[0])
		if returnType == nil {
			continue
		}

		if returnType.Flags()&(checker.TypeFlagsUnknown|checker.TypeFlagsAny) != 0 {
			calleeText := scanner.GetSourceTextOfNodeFromSourceFile(ctx.SourceFile, callee, false)
			return ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(callee), tsdiag.The_catch_callback_in_0_returns_unknown_so_the_Effect_error_type_stays_untyped_A_specific_typed_error_preserves_error_channel_information_for_example_by_narrowing_the_value_or_wrapping_it_in_Data_TaggedError_effect_unknownInEffectCatch, nil, calleeText)
		}
	}

	return nil
}

// isUnknownCatchCallee checks if a node references one of the Effect module catch APIs.
func isUnknownCatchCallee(tp *typeparser.TypeParser, _ *checker.Checker, node *ast.Node) bool {
	for _, name := range unknownCatchApis {
		if tp.IsNodeReferenceToEffectModuleApi(node, name) {
			return true
		}
	}
	return false
}
