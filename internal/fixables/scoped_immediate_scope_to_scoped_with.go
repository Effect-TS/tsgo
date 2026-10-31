package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
)

var ScopedImmediateScopeToScopedWithFix = fixable.Fixable{
	Name:        "scopedImmediateScopeToScopedWith",
	Description: "Replace with Effect.scopedWith",
	ErrorCodes: []int32{
		tsdiag.Effect_scoped_provides_a_scope_that_this_body_immediately_reads_back_with_Effect_scope_Effect_scopedWith_passes_the_scope_to_the_callback_directly_effect_scopedImmediateScopeToScopedWith.Code(),
	},
	FixIDs: []string{"scopedImmediateScopeToScopedWith_fix"},
	Run:    runScopedImmediateScopeToScopedWithFix,
}

func runScopedImmediateScopeToScopedWithFix(ctx *fixable.Context) []ls.CodeAction {
	sf := ctx.SourceFile
	for _, match := range rules.AnalyzeScopedImmediateScopeToScopedWith(ctx.TypeParser, ctx.Checker, sf) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if !match.Fixable {
			continue
		}
		member := effectModuleMember(ctx, "scopedWith")
		scopedCallee := match.Scoped.Callee()
		// Keep the qualifier of Effect.scoped when it is the Effect module, which
		// already resolves where the edit lands; otherwise go through the file's
		// Effect import.
		var qualifier *ast.Node
		var target *moduleMemberReference
		if ast.IsPropertyAccessExpression(scopedCallee) {
			expression := scopedCallee.AsPropertyAccessExpression().Expression
			if ctx.TypeParser.IsExpressionEffectModule(expression) && typeHasMember(ctx, ctx.TypeParser.GetTypeAtLocation(expression), member) {
				qualifier = expression
			}
		}
		if qualifier == nil {
			target = importedModuleMember(ctx, scopedCallee, typeparser.FindEffectModuleReference(sf), member)
			if target == nil {
				continue
			}
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Replace with Effect.scopedWith",
			Run: func(tracker *rewriter.Tracker) {
				var callee *ast.Node
				if qualifier != nil {
					callee = tracker.NewPropertyAccessExpression(tracker.DeepCloneNode(qualifier), nil, tracker.NewIdentifier("scopedWith"), ast.NodeFlagsNone)
				} else {
					callee = target.expression(tracker, member)
				}
				var argument *ast.Node
				if match.GenFunction != nil {
					argument = newScopedWithGenCallback(tracker, match)
				} else {
					argument = tracker.DeepCloneNode(match.Callback)
				}
				replacement := tracker.NewCallExpression(callee, nil, nil, tracker.NewNodeList([]*ast.Node{argument}), ast.NodeFlagsNone)
				tracker.ReplacePipingFlowPrefix(sf, match.Flow, match.PrefixLength, replacement)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
	}

	return nil
}

// newScopedWithGenCallback builds `(<binding>) => <gen>(function*() { <rest> })`,
// dropping the leading Effect.scope retrieval from the generator body.
func newScopedWithGenCallback(tracker *rewriter.Tracker, match rules.ScopedImmediateScopeToScopedWithMatch) *ast.Node {
	genFn := match.GenFunction
	statements := genFn.Body.AsBlock().Statements.Nodes[1:]
	cloned := make([]*ast.Node, len(statements))
	for i, statement := range statements {
		cloned[i] = tracker.DeepCloneNode(statement)
	}
	var name *ast.Node
	if genFn.Name() != nil {
		name = tracker.DeepCloneNode(genFn.Name())
	}
	generator := tracker.NewFunctionExpression(
		nil,
		tracker.NewToken(ast.KindAsteriskToken),
		name,
		nil,
		cloneScopedWithNodeList(tracker, genFn.Parameters),
		nil,
		nil,
		tracker.NewBlock(tracker.NewNodeList(cloned), true),
	)
	genCall := tracker.NewCallExpression(tracker.DeepCloneNode(match.GenCallee), nil, nil, tracker.NewNodeList([]*ast.Node{generator}), ast.NodeFlagsNone)
	parameter := tracker.NewParameterDeclaration(nil, nil, tracker.NewIdentifier(match.BindingName), nil, nil, nil)
	return tracker.NewArrowFunction(nil, nil, tracker.NewNodeList([]*ast.Node{parameter}), nil, nil, tracker.NewToken(ast.KindEqualsGreaterThanToken), genCall)
}

func cloneScopedWithNodeList(tracker *rewriter.Tracker, list *ast.NodeList) *ast.NodeList {
	var nodes []*ast.Node
	if list != nil {
		for _, node := range list.Nodes {
			nodes = append(nodes, tracker.DeepCloneNode(node))
		}
	}
	return tracker.NewNodeList(nodes)
}
