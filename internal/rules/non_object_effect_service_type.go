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

// NonObjectEffectServiceType checks that Effect.Service option properties
// (succeed, sync, effect, scoped) do not resolve to primitive types.
// V3-only, default severity error.
var NonObjectEffectServiceType = rule.Rule{
	Name:            "nonObjectEffectServiceType",
	Group:           "correctness",
	Description:     "Ensures Effect.Service types are objects, not primitives",
	DefaultSeverity: etscore.SeverityError,
	SupportedEffect: []string{"v3"},
	Codes: []int32{
		tsdiag.Effect_Service_requires_the_service_type_to_be_an_object_and_not_a_primitive_type_Consider_wrapping_the_value_in_an_object_or_manually_using_Context_Tag_or_Effect_Tag_if_you_want_to_use_a_primitive_instead_effect_nonObjectEffectServiceType.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		// V3-only rule
		if ctx.TypeParser.SupportedEffectVersion() != typeparser.EffectMajorV3 {
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
				if d := checkServicePropertyTypes(ctx, node); len(d) > 0 {
					diags = append(diags, d...)
					continue // skip children
				}
			}

			// Enqueue children
			node.ForEachChild(pushChild)
		}

		return diags
	},
}

// checkServicePropertyTypes checks if a class extending Effect.Service has option
// properties that resolve to primitive types.
func checkServicePropertyTypes(ctx *rule.Context, node *ast.Node) []*ast.Diagnostic {
	serviceResult := ctx.TypeParser.ExtendsEffectService(node)
	if serviceResult == nil {
		return nil
	}

	options := serviceResult.Options
	if options == nil || options.Kind != ast.KindObjectLiteralExpression {
		return nil
	}

	objLit := options.AsObjectLiteralExpression()
	if objLit == nil || objLit.Properties == nil {
		return nil
	}

	var diags []*ast.Diagnostic

	for _, prop := range objLit.Properties.Nodes {
		if prop == nil || prop.Kind != ast.KindPropertyAssignment {
			continue
		}
		pa := prop.AsPropertyAssignment()
		if pa == nil || pa.Name() == nil || pa.Name().Kind != ast.KindIdentifier {
			continue
		}

		propertyName := scanner.GetTextOfNode(pa.Name())
		initializer := pa.Initializer
		if initializer == nil {
			continue
		}

		switch propertyName {
		case "succeed":
			valueType := ctx.TypeParser.GetTypeAtLocation(initializer)
			if valueType != nil && isPrimitiveType(valueType) {
				diags = append(diags, ctx.NewDiagnostic(
					ctx.SourceFile,
					ctx.GetErrorRange(pa.Name()),
					tsdiag.Effect_Service_requires_the_service_type_to_be_an_object_and_not_a_primitive_type_Consider_wrapping_the_value_in_an_object_or_manually_using_Context_Tag_or_Effect_Tag_if_you_want_to_use_a_primitive_instead_effect_nonObjectEffectServiceType,
					nil,
				))
			}

		case "sync":
			valueType := ctx.TypeParser.GetTypeAtLocation(initializer)
			if valueType == nil {
				continue
			}
			signatures := ctx.Checker.GetSignaturesOfType(valueType, checker.SignatureKindCall)
			for _, sig := range signatures {
				returnType := ctx.Checker.GetReturnTypeOfSignature(sig)
				if returnType != nil && isPrimitiveType(returnType) {
					diags = append(diags, ctx.NewDiagnostic(
						ctx.SourceFile,
						ctx.GetErrorRange(pa.Name()),
						tsdiag.Effect_Service_requires_the_service_type_to_be_an_object_and_not_a_primitive_type_Consider_wrapping_the_value_in_an_object_or_manually_using_Context_Tag_or_Effect_Tag_if_you_want_to_use_a_primitive_instead_effect_nonObjectEffectServiceType,
						nil,
					))
					break
				}
			}

		case "effect", "scoped":
			valueType := ctx.TypeParser.GetTypeAtLocation(initializer)
			if valueType == nil {
				continue
			}

			// Try direct EffectType parse first
			effectResult := ctx.TypeParser.EffectType(valueType, initializer)
			if effectResult != nil {
				if isPrimitiveType(effectResult.A) {
					diags = append(diags, ctx.NewDiagnostic(
						ctx.SourceFile,
						ctx.GetErrorRange(pa.Name()),
						tsdiag.Effect_Service_requires_the_service_type_to_be_an_object_and_not_a_primitive_type_Consider_wrapping_the_value_in_an_object_or_manually_using_Context_Tag_or_Effect_Tag_if_you_want_to_use_a_primitive_instead_effect_nonObjectEffectServiceType,
						nil,
					))
				}
				continue
			}

			// Fall back to call signatures
			signatures := ctx.Checker.GetSignaturesOfType(valueType, checker.SignatureKindCall)
			for _, sig := range signatures {
				returnType := ctx.Checker.GetReturnTypeOfSignature(sig)
				if returnType == nil {
					continue
				}
				effectReturnResult := ctx.TypeParser.EffectType(returnType, initializer)
				if effectReturnResult != nil && isPrimitiveType(effectReturnResult.A) {
					diags = append(diags, ctx.NewDiagnostic(
						ctx.SourceFile,
						ctx.GetErrorRange(pa.Name()),
						tsdiag.Effect_Service_requires_the_service_type_to_be_an_object_and_not_a_primitive_type_Consider_wrapping_the_value_in_an_object_or_manually_using_Context_Tag_or_Effect_Tag_if_you_want_to_use_a_primitive_instead_effect_nonObjectEffectServiceType,
						nil,
					))
					break
				}
			}
		}
	}

	return diags
}

// isPrimitiveType checks if a type (or any member of a union type) is a primitive type.
func isPrimitiveType(t *checker.Type) bool {
	const primitiveFlags = checker.TypeFlagsString |
		checker.TypeFlagsNumber |
		checker.TypeFlagsBoolean |
		checker.TypeFlagsStringLiteral |
		checker.TypeFlagsNumberLiteral |
		checker.TypeFlagsBooleanLiteral |
		checker.TypeFlagsUndefined |
		checker.TypeFlagsNull

	for _, member := range typeparser.UnrollUnionMembers(t) {
		if member.Flags()&primitiveFlags != 0 {
			return true
		}
	}
	return false
}
