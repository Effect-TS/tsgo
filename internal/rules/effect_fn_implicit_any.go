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

// EffectFnImplicitAny mirrors TypeScript's noImplicitAny behavior for
// Effect.fn-family callbacks, which are otherwise
// contextually typed by the helper's internal any[] fallback.
var EffectFnImplicitAny = rule.Rule{
	Name:            "effectFnImplicitAny",
	Group:           "correctness",
	Description:     "Mirrors noImplicitAny for unannotated Effect.fn, Effect.fnUntraced, and Effect.fnUntracedEager callback parameters when no outer contextual function type exists. Requires TS's noImplicitAny: true",
	DefaultSeverity: etscore.SeverityError,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Parameter_0_implicitly_has_type_any_in_Effect_fn_Effect_fnUntraced_or_Effect_fnUntracedEager_No_parameter_type_is_available_from_an_explicit_annotation_or_contextual_function_type_effect_effectFnImplicitAny.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		tp := ctx.TypeParser
		if !ctx.Program.Options().GetStrictOptionValue(ctx.Program.Options().NoImplicitAny) {
			return nil
		}

		var diags []*ast.Diagnostic

		var walk ast.Visitor
		walk = func(n *ast.Node) bool {
			if n == nil {
				return false
			}

			if result := tp.EffectFnCall(n); result != nil {
				if result.IsGenerator() {
					diags = append(diags, checkEffectFnImplicitAny(ctx, result)...)
				} else {
					diags = append(diags, checkEffectFnImplicitAnyBody(ctx, result.Call.AsNode(), result.FunctionNode)...)
				}
			}

			n.ForEachChild(walk)
			return false
		}

		walk(ctx.SourceFile.AsNode())

		return diags
	},
}

func checkEffectFnImplicitAny(ctx *rule.Context, result *typeparser.EffectFnCallResult) []*ast.Diagnostic {
	genFn := result.GeneratorFunction()
	if result == nil || genFn == nil || genFn.Parameters == nil {
		return nil
	}
	return checkEffectFnImplicitAnyParameters(ctx, result.Call.AsNode(), genFn.Parameters.Nodes)
}

func checkEffectFnImplicitAnyBody(ctx *rule.Context, callNode *ast.Node, fnNode *ast.Node) []*ast.Diagnostic {
	if fnNode == nil {
		return nil
	}

	switch fnNode.Kind {
	case ast.KindArrowFunction:
		fn := fnNode.AsArrowFunction()
		if fn == nil || fn.Parameters == nil {
			return nil
		}
		return checkEffectFnImplicitAnyParameters(ctx, callNode, fn.Parameters.Nodes)
	case ast.KindFunctionExpression:
		fn := fnNode.AsFunctionExpression()
		if fn == nil || fn.Parameters == nil {
			return nil
		}
		return checkEffectFnImplicitAnyParameters(ctx, callNode, fn.Parameters.Nodes)
	default:
		return nil
	}
}

func checkEffectFnImplicitAnyParameters(ctx *rule.Context, callNode *ast.Node, params []*ast.Node) []*ast.Diagnostic {
	if hasOuterContextualFunctionType(ctx.TypeParser, ctx.Checker, callNode) {
		return nil
	}

	var diags []*ast.Diagnostic
	for _, param := range params {
		if param == nil || param.Type() != nil || param.Initializer() != nil {
			continue
		}

		name := scanner.DeclarationNameToString(param.Name())
		if name == "" {
			name = "parameter"
		}

		diags = append(diags, ctx.NewDiagnostic(
			ctx.SourceFile,
			ctx.GetErrorRange(param.Name()),
			tsdiag.Parameter_0_implicitly_has_type_any_in_Effect_fn_Effect_fnUntraced_or_Effect_fnUntracedEager_No_parameter_type_is_available_from_an_explicit_annotation_or_contextual_function_type_effect_effectFnImplicitAny,
			nil,
			name,
		))
	}

	return diags
}

func hasOuterContextualFunctionType(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node) bool {
	if c == nil || node == nil || !ast.IsExpression(node) {
		return false
	}

	contextualType := c.GetContextualType(node, checker.ContextFlagsNone)
	if contextualType == nil {
		return false
	}

	for _, member := range tp.UnrollUnionMembers(contextualType) {
		if len(c.GetSignaturesOfType(member, checker.SignatureKindCall)) > 0 {
			return true
		}
	}

	return false
}
