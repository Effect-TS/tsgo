// Package rules contains all Effect diagnostic rule implementations.
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
	Codes:           []int32{tsdiag.The_catch_callback_in_0_returns_unknown_The_catch_callback_should_be_used_to_provide_typed_errors_Consider_wrapping_unknown_errors_into_Effect_s_Data_TaggedError_for_example_or_narrow_down_the_type_to_the_specific_error_raised_effect_unknownInEffectCatch.Code()},
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
	if !isUnknownCatchCallee(ctx.Checker, callee) {
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

	for _, objectType := range typeparser.UnrollUnionMembers(paramType) {
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
			return ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(callee), tsdiag.The_catch_callback_in_0_returns_unknown_The_catch_callback_should_be_used_to_provide_typed_errors_Consider_wrapping_unknown_errors_into_Effect_s_Data_TaggedError_for_example_or_narrow_down_the_type_to_the_specific_error_raised_effect_unknownInEffectCatch, nil, calleeText)
		}
	}

	return nil
}

// isUnknownCatchCallee checks if a node references one of the Effect module catch APIs.
func isUnknownCatchCallee(c *checker.Checker, node *ast.Node) bool {
	for _, name := range unknownCatchApis {
		if typeparser.IsNodeReferenceToEffectModuleApi(c, node, name) {
			return true
		}
	}
	return false
}
