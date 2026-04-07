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

var LazyPromiseInEffectSync = rule.Rule{
	Name:            "lazyPromiseInEffectSync",
	Group:           "antipattern",
	Description:     "Warns when Effect.sync lazily returns a Promise instead of using an async Effect constructor",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.This_Effect_sync_thunk_returns_a_Promise_Use_Effect_promise_or_Effect_tryPromise_to_represent_async_work_effect_lazyPromiseInEffectSync.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic
		var walk ast.Visitor
		walk = func(node *ast.Node) bool {
			if node == nil {
				return false
			}

			if node.Kind == ast.KindCallExpression {
				call := node.AsCallExpression()
				if call != nil && ctx.TypeParser.IsNodeReferenceToEffectModuleApi(call.Expression, "sync") && call.Arguments != nil && len(call.Arguments.Nodes) > 0 {
					lazyArg := call.Arguments.Nodes[0]
					lazyArgType := ctx.TypeParser.GetTypeAtLocation(lazyArg)
					if lazyArgType != nil && thunkReturnsPromise(ctx.Checker, ctx.TypeParser, lazyArgType) {
						diags = append(diags, ctx.NewDiagnostic(
							ctx.SourceFile,
							scanner.GetErrorRangeForNode(ctx.SourceFile, lazyArg),
							tsdiag.This_Effect_sync_thunk_returns_a_Promise_Use_Effect_promise_or_Effect_tryPromise_to_represent_async_work_effect_lazyPromiseInEffectSync,
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
	},
}

func thunkReturnsPromise(c *checker.Checker, tp *typeparser.TypeParser, lazyArgType *checker.Type) bool {
	for _, member := range tp.UnrollUnionMembers(lazyArgType) {
		for _, signature := range c.GetSignaturesOfType(member, checker.SignatureKindCall) {
			returnType := c.GetReturnTypeOfSignature(signature)
			if tp.PromiseType(returnType) != nil {
				return true
			}
		}
	}
	return false
}
