package rules

import (
	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
)

// GenericEffectServices detects Effect Service class declarations that have type
// parameters (generics), which cannot be properly discriminated at runtime.
// This is a V3-only rule.
var GenericEffectServices = rule.Rule{
	Name:            "genericEffectServices",
	Group:           "correctness",
	Description:     "Prevents services with type parameters that cannot be discriminated at runtime",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Effect_Services_with_type_parameters_are_not_supported_because_they_cannot_be_properly_discriminated_at_runtime_which_may_cause_unexpected_behavior_effect_genericEffectServices.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		// V3-only rule
		if typeparser.SupportedEffectVersion(ctx.Checker) != typeparser.EffectMajorV3 {
			return nil
		}

		var diags []*ast.Diagnostic

		// Stack-based traversal
		nodeToVisit := make([]*ast.Node, 0)
		pushChild := func(child *ast.Node) bool {
			nodeToVisit = append(nodeToVisit, child)
			return false
		}
		ctx.SourceFile.AsNode().ForEachChild(pushChild)

		for len(nodeToVisit) > 0 {
			node := nodeToVisit[len(nodeToVisit)-1]
			nodeToVisit = nodeToVisit[:len(nodeToVisit)-1]

			if node.Kind == ast.KindClassDeclaration {
				classDecl := node.AsClassDeclaration()
				if node.Name() != nil && classDecl.TypeParameters != nil && classDecl.HeritageClauses != nil {
					classSym := ctx.Checker.GetSymbolAtLocation(node.Name())
					if classSym != nil {
						classType := ctx.Checker.GetTypeOfSymbolAtLocation(classSym, node)
						if classType != nil && typeparser.IsContextTag(ctx.Checker, classType, node) {
							diags = append(diags, ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(node.Name()), tsdiag.Effect_Services_with_type_parameters_are_not_supported_because_they_cannot_be_properly_discriminated_at_runtime_which_may_cause_unexpected_behavior_effect_genericEffectServices, nil))
							continue // skip children
						}
					}
				}
			}

			// Enqueue children for further traversal
			node.ForEachChild(pushChild)
		}

		return diags
	},
}
