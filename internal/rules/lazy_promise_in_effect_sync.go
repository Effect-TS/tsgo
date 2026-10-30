package rules

import (
	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
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
		for _, flow := range ctx.TypeParser.PipingFlows(ctx.SourceFile, false).All() {
			for index := range flow.Transformations().All() {
				// Effect.sync(thunk) and the point-free pipe(thunk, Effect.sync)
				// both apply Effect.sync to the step input without extra arguments.
				transformation := flow.Transformations().At(index)
				if transformation.Args().Len() != 0 || !ctx.TypeParser.IsNodeReferenceToEffectModuleApi(transformation.Callee(), "sync") {
					continue
				}
				lazyArgType := flow.TransformationInputType(index)
				if lazyArgType == nil || !thunkReturnsPromise(ctx.Checker, ctx.TypeParser, lazyArgType) {
					continue
				}
				location := flow.TransformationInputNode(index)
				if location == nil {
					location = transformation.Callee()
				}
				diags = append(diags, ctx.NewDiagnostic(
					ctx.SourceFile,
					scanner.GetErrorRangeForNode(ctx.SourceFile, location),
					tsdiag.This_Effect_sync_thunk_returns_a_Promise_Use_Effect_promise_or_Effect_tryPromise_to_represent_async_work_effect_lazyPromiseInEffectSync,
					nil,
				))
			}
		}
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
