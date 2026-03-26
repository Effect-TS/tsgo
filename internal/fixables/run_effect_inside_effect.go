package fixables

import (
	"fmt"

	"github.com/effect-ts/tsgo/internal/effectutil"
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
	Description: "Use a runtime to run the Effect",
	ErrorCodes:  []int32{tsdiag.Using_0_inside_an_Effect_is_not_recommended_The_same_runtime_should_generally_be_used_instead_to_run_child_effects_Consider_extracting_the_Runtime_by_using_for_example_Effect_runtime_and_then_use_Runtime_1_with_the_extracted_runtime_instead_effect_runEffectInsideEffect.Code()},
	FixIDs:      []string{"runEffectInsideEffect_fix"},
	Run:         runRunEffectInsideEffectFix,
}

func runRunEffectInsideEffectFix(ctx *fixable.Context) []ls.CodeAction {
	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	sf := ctx.SourceFile

	matches := rules.AnalyzeRunEffectInsideEffect(c, sf)
	for _, match := range matches {
		if !match.IsNestedScope {
			continue
		}
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		// Capture loop variables for the closure
		m := match

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Use a runtime to run the Effect",
			Run: func(tracker *change.Tracker) {
				genFn := m.GeneratorFunction
				block := genFn.Body.AsBlock()

				// Step 1: Scan generator body for existing `const X = yield* Effect.runtime()` declaration
				runtimeIdentifier := ""
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
					if yieldExpr.AsteriskToken == nil || yieldExpr.Expression == nil {
						continue
					}
					if yieldExpr.Expression.Kind != ast.KindCallExpression {
						continue
					}
					yieldedCall := yieldExpr.Expression.AsCallExpression()
					if typeparser.IsNodeReferenceToEffectModuleApi(c, yieldedCall.Expression, "runtime") {
						if decl.Name() != nil && decl.Name().Kind == ast.KindIdentifier {
							runtimeIdentifier = scanner.GetTextOfNode(decl.Name())
						}
					}
				}

				// Step 2: If no existing runtime variable, insert one at the top of the generator body
				if runtimeIdentifier == "" {
					runtimeIdentifier = "effectRuntime"

					// Resolve the Effect module identifier from imports
					effectModuleIdentifier := effectutil.FindModuleIdentifier(sf,"Effect")

					// Build: const effectRuntime = yield* Effect.runtime<never>()
					effectId := tracker.NewIdentifier(effectModuleIdentifier)
					runtimeAccess := tracker.NewPropertyAccessExpression(
						effectId, nil, tracker.NewIdentifier("runtime"), ast.NodeFlagsNone,
					)
					runtimeCall := tracker.NewCallExpression(
						runtimeAccess,
						nil,
						tracker.NewNodeList([]*ast.Node{tracker.NewKeywordTypeNode(ast.KindNeverKeyword)}),
						tracker.NewNodeList([]*ast.Node{}),
						ast.NodeFlagsNone,
					)
					yieldExpr := tracker.NewYieldExpression(
						tracker.NewToken(ast.KindAsteriskToken),
						runtimeCall,
					)
					varDecl := tracker.NewVariableDeclaration(
						tracker.NewIdentifier("effectRuntime"), nil, nil, yieldExpr,
					)
					varDeclList := tracker.NewVariableDeclarationList(
						ast.NodeFlagsConst,
						tracker.NewNodeList([]*ast.Node{varDecl}),
					)
					varStmt := tracker.NewVariableStatement(nil, varDeclList)
					ast.SetParentInChildren(varStmt)

					insertPos := core.TextPos(block.Statements.Nodes[0].Pos())
					tracker.InsertNodeAt(sf, insertPos, varStmt, change.NodeOptions{Suffix: "\n"})
				}

				// Step 3: Resolve the Runtime module identifier from imports
				runtimeModuleIdentifier := effectutil.FindModuleIdentifier(sf,"Runtime")

				// Step 4: Replace the callee expression with Runtime.runXxx(runtimeIdentifier,
				// Delete from callee start to first argument start
				calleeTokenPos := scanner.GetTokenPosOfNode(m.CalleeNode, sf, false)
				firstArgPos := m.CallNode.AsCallExpression().Arguments.Nodes[0].Pos()
				tracker.DeleteRange(sf, core.NewTextRange(calleeTokenPos, firstArgPos))

				// Insert replacement text at the first argument position
				replacementText := fmt.Sprintf("%s.%s(%s, ", runtimeModuleIdentifier, m.MethodName, runtimeIdentifier)
				tracker.InsertText(sf, ctx.BytePosToLSPPosition(firstArgPos), replacementText)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
