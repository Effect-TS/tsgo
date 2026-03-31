package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var OverriddenSchemaConstructorFix = fixable.Fixable{
	Name:        "overriddenSchemaConstructorFix",
	Description: "Remove or rewrite constructor in Schema class",
	ErrorCodes:  []int32{tsdiag.Classes_extending_Schema_must_not_override_the_constructor_this_is_because_it_silently_breaks_the_schema_decoding_behaviour_If_that_s_needed_we_recommend_instead_to_use_a_static_new_method_that_constructs_the_instance_effect_overriddenSchemaConstructor.Code()},
	FixIDs: []string{
		"overriddenSchemaConstructor_fix",
		"overriddenSchemaConstructor_static",
	},
	Run: runOverriddenSchemaConstructorFix,
}

func runOverriddenSchemaConstructorFix(ctx *fixable.Context) []ls.CodeAction {
	c := ctx.Checker

	sf := ctx.SourceFile

	matches := rules.AnalyzeOverriddenSchemaConstructor(ctx.TypeParser, c, sf)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		var actions []ls.CodeAction

		// _static action: Rewrite using the static 'new' pattern (only when body exists)
		if match.HasBody && constructorSupportsStaticRewrite(match.ConstructorNode) {
			if action := ctx.NewFixAction(fixable.FixAction{
				Description: "Rewrite using the static 'new' pattern",
				Run: func(tracker *change.Tracker) {
					ctor := match.ConstructorNode.AsConstructorDeclaration()

					// Build a visitor that transforms super(...) calls and this keywords
					var v *ast.NodeVisitor
					visitFn := func(node *ast.Node) *ast.Node {
						// Replace super(...) expression statements with: const _this = new this(...)
						if node.Kind == ast.KindExpressionStatement {
							expr := node.AsExpressionStatement().Expression
							if expr.Kind == ast.KindCallExpression {
								call := expr.AsCallExpression()
								if call.Expression.Kind == ast.KindSuperKeyword {
									constructThis := tracker.NewNewExpression(
										tracker.NewIdentifier("this"),
										nil,
										call.Arguments,
									)
									return tracker.NewVariableStatement(
										nil,
										tracker.NewVariableDeclarationList(
											ast.NodeFlagsConst,
											tracker.NewNodeList([]*ast.Node{
												tracker.NewVariableDeclaration(
													tracker.NewIdentifier("_this"),
													nil,
													nil,
													constructThis,
												),
											}),
										),
									)
								}
							}
						}
						// Replace this keywords with _this identifier
						if node.Kind == ast.KindThisKeyword {
							return tracker.NewIdentifier("_this")
						}
						return v.VisitEachChild(node)
					}
					v = ast.NewNodeVisitor(visitFn, tracker.NodeFactory, ast.NodeVisitorHooks{})

					// Transform the constructor body
					newBody := v.VisitNode(ctor.Body)

					// Append return _this to the transformed body's statements
					bodyBlock := newBody.AsBlock()
					newStatements := make([]*ast.Node, len(bodyBlock.Statements.Nodes)+1)
					copy(newStatements, bodyBlock.Statements.Nodes)
					newStatements[len(newStatements)-1] = tracker.NewReturnStatement(tracker.NewIdentifier("_this"))
					newBlock := tracker.NewBlock(tracker.NewNodeList(newStatements), true)

					// Build modifier list with public and static
					modifiers := tracker.NewModifierList([]*ast.Node{
						tracker.NewModifier(ast.KindPublicKeyword),
						tracker.NewModifier(ast.KindStaticKeyword),
					})

					// Build the replacement method declaration
					newMethod := tracker.NewMethodDeclaration(
						modifiers,
						nil,
						tracker.NewIdentifier("new"),
						nil,
						ctor.TypeParameters,
						ctor.Parameters,
						ctor.Type,
						nil,
						newBlock,
					)

					tracker.ReplaceNode(sf, match.ConstructorNode, newMethod, nil)
				},
			}); action != nil {
				actions = append(actions, *action)
			}
		}

		// _fix action: Remove the constructor override (always available)
		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Remove the constructor override",
			Run: func(tracker *change.Tracker) {
				tracker.Delete(sf, match.ConstructorNode)
			},
		}); action != nil {
			actions = append(actions, *action)
		}

		return actions
	}

	return nil
}

func constructorSupportsStaticRewrite(ctorNode *ast.Node) bool {
	ctor := ctorNode.AsConstructorDeclaration()
	if ctor.Parameters == nil {
		return true
	}
	for _, paramNode := range ctor.Parameters.Nodes {
		if ast.IsParameterPropertyDeclaration(paramNode, ctorNode) {
			return false
		}
	}
	return true
}
