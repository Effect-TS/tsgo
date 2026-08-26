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

var PromiseInEffectSuccess = rule.Rule{
	Name:            "promiseInEffectSuccess",
	Group:           "correctness",
	Description:     "Detects Promise types in Effect success channels where they are not awaited",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v3", "v4"},
	Codes: []int32{
		tsdiag.The_Effect_success_channel_contains_a_Promise_that_is_not_awaited_Use_Effect_promise_or_Effect_tryPromise_to_represent_async_work_effect_promiseInEffectSuccess.Code(),
	},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		var diags []*ast.Diagnostic
		type stackEntry struct {
			node    *ast.Node
			visited bool
		}

		stack := []stackEntry{{node: ctx.SourceFile.AsNode()}}
		matched := map[*ast.Node]bool{}
		pushChild := func(child *ast.Node) bool {
			stack = append(stack, stackEntry{node: child})
			return false
		}

		for len(stack) > 0 {
			entry := stack[len(stack)-1]
			stack = stack[:len(stack)-1]
			node := entry.node
			if node == nil {
				continue
			}

			if !entry.visited {
				if ast.IsTypeNode(node) {
					continue
				}
				stack = append(stack, stackEntry{node: node, visited: true})
				node.ForEachChild(pushChild)
				continue
			}

			if matched[node] {
				if node.Parent != nil && !ast.IsFunctionLike(node) {
					matched[node.Parent] = true
				}
				continue
			}

			if !ast.IsExpression(node) || ast.IsDeclarationName(node) {
				continue
			}

			// Declared-type prefilter: a diagnostic requires a strict Effect
			// flow type on the node, which reference nodes with a
			// conclusively non-Effect declared type — and calls whose
			// resolved signature conclusively cannot return one — can never
			// have. Skipped nodes can never match, so no matched-map
			// bookkeeping is needed.
			if !ctx.TypeParser.NodeCouldBeStrictEffect(node) {
				continue
			}

			var t *checker.Type
			if node.Kind == ast.KindCallExpression {
				if signature := ctx.Checker.GetResolvedSignature(node); signature != nil {
					t = ctx.Checker.GetReturnTypeOfSignature(signature)
				}
			}
			if t == nil {
				t = ctx.TypeParser.GetTypeAtLocation(node)
			}
			effect := ctx.TypeParser.StrictEffectType(t, node)
			if effect == nil || !typeContainsPromise(ctx.TypeParser, effect.A) {
				continue
			}

			if hasExplicitPromiseEffectContext(ctx.TypeParser, ctx.Checker, node) ||
				hasExplicitPromiseSuccessTypeArguments(ctx.TypeParser, node) ||
				isEffectSyncCall(ctx.TypeParser, node) {
				continue
			}

			diags = append(diags, ctx.NewDiagnostic(
				ctx.SourceFile,
				ctx.GetErrorRange(node),
				tsdiag.The_Effect_success_channel_contains_a_Promise_that_is_not_awaited_Use_Effect_promise_or_Effect_tryPromise_to_represent_async_work_effect_promiseInEffectSuccess,
				nil,
			))
			if node.Parent != nil {
				matched[node.Parent] = true
			}
		}

		return diags
	},
}

func hasExplicitTypeArguments(call *ast.CallExpression) bool {
	return call != nil && call.TypeArguments != nil && len(call.TypeArguments.Nodes) > 0
}

func hasExplicitPromiseEffectContext(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node) bool {
	for current := node; current != nil; current = current.Parent {
		switch current.Kind {
		case ast.KindVariableDeclaration, ast.KindPropertyDeclaration, ast.KindParameter:
			return isExplicitPromiseEffectType(tp, current.Type(), current)
		case ast.KindAsExpression, ast.KindSatisfiesExpression:
			if isExplicitPromiseEffectType(tp, current.Type(), current) {
				return true
			}
		case ast.KindArrowFunction, ast.KindFunctionExpression, ast.KindFunctionDeclaration, ast.KindMethodDeclaration:
			if current.Kind == ast.KindArrowFunction && current.AsArrowFunction().Body.Kind != ast.KindBlock {
				return hasExplicitPromiseEffectReturn(tp, c, current)
			}
			return false
		case ast.KindReturnStatement:
			function := ast.GetContainingFunction(current)
			return function != nil && hasExplicitPromiseEffectReturn(tp, c, function)
		}
	}
	return false
}

func isExplicitPromiseEffectType(tp *typeparser.TypeParser, typeNode *ast.Node, atLocation *ast.Node) bool {
	if typeNode == nil {
		return false
	}
	effect := tp.StrictEffectType(tp.GetTypeAtLocation(typeNode), atLocation)
	return effect != nil && typeContainsPromise(tp, effect.A)
}

func hasExplicitPromiseEffectReturn(tp *typeparser.TypeParser, c *checker.Checker, function *ast.Node) bool {
	if isExplicitPromiseEffectType(tp, function.Type(), function) {
		return true
	}
	parent := function.Parent
	for parent != nil && parent.Kind == ast.KindParenthesizedExpression {
		parent = parent.Parent
	}
	if parent == nil {
		return false
	}
	typeNode := parent.Type()
	if typeNode == nil {
		return false
	}
	t := tp.GetTypeAtLocation(typeNode)
	for _, member := range tp.UnrollUnionMembers(t) {
		for _, signature := range c.GetSignaturesOfType(member, checker.SignatureKindCall) {
			result := tp.StrictEffectType(c.GetReturnTypeOfSignature(signature), function)
			if result != nil && typeContainsPromise(tp, result.A) {
				return true
			}
		}
	}
	return false
}

func typeContainsPromise(tp *typeparser.TypeParser, t *checker.Type) bool {
	for _, member := range tp.UnrollUnionMembers(t) {
		if tp.PromiseType(member) != nil {
			return true
		}
	}
	return false
}

func hasExplicitPromiseSuccessTypeArguments(tp *typeparser.TypeParser, node *ast.Node) bool {
	if node == nil || node.Kind != ast.KindCallExpression {
		return false
	}
	call := node.AsCallExpression()
	if isExplicitPromiseSuccessCall(tp, call) {
		return true
	}
	if expression := ast.SkipParentheses(call.Expression); expression != nil && expression.Kind == ast.KindCallExpression && isExplicitPromiseSuccessCall(tp, expression.AsCallExpression()) {
		return true
	}
	if isPipeCall(tp, call) && call.Arguments != nil && len(call.Arguments.Nodes) > 0 {
		argument := ast.SkipParentheses(call.Arguments.Nodes[len(call.Arguments.Nodes)-1])
		return argument != nil && argument.Kind == ast.KindCallExpression && isExplicitPromiseSuccessCall(tp, argument.AsCallExpression())
	}
	return false
}

func isPipeCall(tp *typeparser.TypeParser, call *ast.CallExpression) bool {
	if call == nil || call.Expression == nil {
		return false
	}
	if tp.IsNodeReferenceToEffectPackageExport(call.Expression, "pipe") {
		return true
	}
	if call.Expression.Kind == ast.KindPropertyAccessExpression {
		name := call.Expression.AsPropertyAccessExpression().Name()
		return name != nil && scanner.GetTextOfNode(name) == "pipe"
	}
	return false
}

func isExplicitPromiseSuccessCall(tp *typeparser.TypeParser, call *ast.CallExpression) bool {
	if !hasExplicitTypeArguments(call) {
		return false
	}
	for _, name := range []string{"succeed", "as", "map", "zipWith"} {
		if tp.IsNodeReferenceToEffectModuleApi(call.Expression, name) {
			return true
		}
	}
	return false
}

func isEffectSyncCall(tp *typeparser.TypeParser, node *ast.Node) bool {
	if node == nil || node.Kind != ast.KindCallExpression {
		return false
	}
	return tp.IsNodeReferenceToEffectModuleApi(node.AsCallExpression().Expression, "sync")
}
