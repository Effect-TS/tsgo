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
	"github.com/microsoft/typescript-go/shim/ls/change"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var RunEffectInsideEffectFix = fixable.Fixable{
	Name:        "runEffectInsideEffect",
	Description: "Use the current services or a runtime to run the Effect",
	ErrorCodes: []int32{
		tsdiag.Using_0_inside_an_Effect_is_not_recommended_The_same_runtime_should_generally_be_used_instead_to_run_child_effects_Consider_extracting_the_Runtime_by_using_for_example_Effect_runtime_and_then_use_Runtime_1_with_the_extracted_runtime_instead_effect_runEffectInsideEffect.Code(),
		tsdiag.Using_0_inside_an_Effect_is_not_recommended_The_same_services_should_generally_be_used_instead_to_run_child_effects_Consider_extracting_the_current_services_by_using_for_example_Effect_services_and_then_use_Effect_1_With_with_the_extracted_services_instead_effect_runEffectInsideEffect.Code(),
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
			Run: func(tracker *change.Tracker) {
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
					if tp.IsNodeReferenceToEffectModuleApi(yieldedCall.Expression, "services") {
						servicesIdentifier = identifier
					}
				}

				effectModuleIdentifier := typeparser.FindModuleIdentifier(sf, "Effect")
				if supportedEffect == typeparser.EffectMajorV4 {
					if servicesIdentifier == "" {
						servicesIdentifier = "effectServices"
						insertYieldedEffectModuleCall(tracker, sf, block, effectModuleIdentifier, "services", servicesIdentifier)
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
	tracker *change.Tracker,
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
		ast.NodeFlagsConst,
		tracker.NewNodeList([]*ast.Node{varDecl}),
	)
	varStmt := tracker.NewVariableStatement(nil, varDeclList)
	ast.SetParentInChildren(varStmt)

	insertPos := core.TextPos(block.Statements.Nodes[0].Pos())
	tracker.InsertNodeAt(sf, insertPos, varStmt, change.NodeOptions{Prefix: "\n", Suffix: "\n"})
}
