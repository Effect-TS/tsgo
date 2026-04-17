package fixables

import (
	"fmt"

	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var RunEffectInsideEffectFix = fixable.Fixable{
	Name:        "runEffectInsideEffect",
	Description: "Use the current services or a runtime to run the Effect",
	ErrorCodes: []int32{
		tsdiag.X_0_is_called_inside_an_Effect_with_a_separate_runtime_invocation_In_this_context_run_child_Effects_with_the_surrounding_runtime_which_can_be_accessed_through_Effect_runtime_and_Runtime_1_effect_runEffectInsideEffect.Code(),
		tsdiag.X_0_is_called_inside_an_Effect_with_a_separate_services_invocation_In_this_context_child_Effects_run_with_the_surrounding_services_which_can_be_accessed_through_Effect_context_and_Effect_1_With_effect_runEffectInsideEffect.Code(),
	},
	FixIDs: []string{"runEffectInsideEffect_fix"},
	Run:    runRunEffectInsideEffectFix,
}

func runRunEffectInsideEffectFix(ctx *fixable.Context) []ls.CodeAction {
	c := ctx.Checker
	tp := ctx.TypeParser

	sf := ctx.SourceFile
	supportedEffect := tp.SupportedEffectVersion()

	matches := rules.AnalyzeRunEffectInsideEffect(ctx.TypeParser, c, sf)
	for _, match := range matches {
		if !match.IsNestedScope {
			continue
		}
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		m := match

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: runEffectInsideEffectFixDescription(supportedEffect),
			Run: func(tracker *rewriter.Tracker) {
				genFn := m.GeneratorFunction
				block := genFn.Body.AsBlock()

				runtimeIdentifier := ""
				servicesIdentifier := ""
				for _, stmt := range block.Statements.Nodes {
					if stmt.Kind != ast.KindVariableStatement {
						continue
					}
					varStmt := stmt.AsVariableStatement()
					if varStmt.DeclarationList == nil {
						continue
					}
					declList := varStmt.DeclarationList.AsVariableDeclarationList()
					if declList.Declarations == nil || len(declList.Declarations.Nodes) != 1 {
						continue
					}
					decl := declList.Declarations.Nodes[0].AsVariableDeclaration()
					if decl.Initializer == nil || decl.Initializer.Kind != ast.KindYieldExpression {
						continue
					}
					yieldExpr := decl.Initializer.AsYieldExpression()
					if yieldExpr.AsteriskToken == nil || yieldExpr.Expression == nil || yieldExpr.Expression.Kind != ast.KindCallExpression {
						continue
					}
					yieldedCall := yieldExpr.Expression.AsCallExpression()
					if decl.Name() == nil || decl.Name().Kind != ast.KindIdentifier {
						continue
					}
					identifier := scanner.GetTextOfNode(decl.Name())
					if tp.IsNodeReferenceToEffectModuleApi(yieldedCall.Expression, "runtime") {
						runtimeIdentifier = identifier
					}
					if tp.IsNodeReferenceToEffectModuleApi(yieldedCall.Expression, "context") {
						servicesIdentifier = identifier
					}
				}

				effectModuleIdentifier := typeparser.FindModuleIdentifier(sf, "Effect")
				if supportedEffect == typeparser.EffectMajorV4 {
					if servicesIdentifier == "" {
						servicesIdentifier = "effectContext"
						insertYieldedEffectModuleCall(tracker, sf, block, effectModuleIdentifier, "context", servicesIdentifier)
					}
				} else if runtimeIdentifier == "" {
					runtimeIdentifier = "effectRuntime"
					insertYieldedEffectModuleCall(tracker, sf, block, effectModuleIdentifier, "runtime", runtimeIdentifier)
				}

				runtimeModuleIdentifier := typeparser.FindModuleIdentifier(sf, "Runtime")
				calleeTokenPos := scanner.GetTokenPosOfNode(m.CalleeNode, sf, false)
				firstArgPos := m.CallNode.AsCallExpression().Arguments.Nodes[0].Pos()
				tracker.DeleteRange(sf, core.NewTextRange(calleeTokenPos, firstArgPos))

				replacementText := runEffectInsideEffectReplacementText(
					supportedEffect,
					effectModuleIdentifier,
					runtimeModuleIdentifier,
					m.MethodName,
					runtimeIdentifier,
					servicesIdentifier,
				)
				tracker.InsertText(sf, ctx.BytePosToLSPPosition(firstArgPos), replacementText)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}

func runEffectInsideEffectFixDescription(supportedEffect typeparser.EffectMajorVersion) string {
	if supportedEffect == typeparser.EffectMajorV4 {
		return "Use the current services to run the Effect"
	}
	return "Use a runtime to run the Effect"
}

func runEffectInsideEffectReplacementText(
	supportedEffect typeparser.EffectMajorVersion,
	effectModuleIdentifier string,
	runtimeModuleIdentifier string,
	methodName string,
	runtimeIdentifier string,
	servicesIdentifier string,
) string {
	if supportedEffect == typeparser.EffectMajorV4 {
		return fmt.Sprintf("%s.%sWith(%s)(", effectModuleIdentifier, methodName, servicesIdentifier)
	}
	return fmt.Sprintf("%s.%s(%s, ", runtimeModuleIdentifier, methodName, runtimeIdentifier)
}

func insertYieldedEffectModuleCall(
	tracker *rewriter.Tracker,
	sf *ast.SourceFile,
	block *ast.Block,
	effectModuleIdentifier string,
	methodName string,
	variableName string,
) {
	effectId := tracker.NewIdentifier(effectModuleIdentifier)
	methodAccess := tracker.NewPropertyAccessExpression(
		effectId, nil, tracker.NewIdentifier(methodName), ast.NodeFlagsNone,
	)
	methodCall := tracker.NewCallExpression(
		methodAccess,
		nil,
		tracker.NewNodeList([]*ast.Node{tracker.NewKeywordTypeNode(ast.KindNeverKeyword)}),
		tracker.NewNodeList([]*ast.Node{}),
		ast.NodeFlagsNone,
	)
	yieldExpr := tracker.NewYieldExpression(
		tracker.NewToken(ast.KindAsteriskToken),
		methodCall,
	)
	varDecl := tracker.NewVariableDeclaration(
		tracker.NewIdentifier(variableName), nil, nil, yieldExpr,
	)
	varDeclList := tracker.NewVariableDeclarationList(
		tracker.NewNodeList([]*ast.Node{varDecl}),
		ast.NodeFlagsConst,
	)
	varStmt := tracker.NewVariableStatement(nil, varDeclList)
	ast.SetParentInChildren(varStmt)

	insertPos := core.TextPos(block.Statements.Nodes[0].Pos())
	tracker.InsertNodeAt(sf, insertPos, varStmt, rewriter.NodeOptions{Prefix: "\n", Suffix: "\n"})
}
