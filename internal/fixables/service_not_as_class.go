package fixables

import (
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/microsoft/typescript-go/shim/ast"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var ServiceNotAsClassFix = fixable.Fixable{
	Name:        "serviceNotAsClass",
	Description: "Convert to class declaration",
	ErrorCodes:  []int32{tsdiag.ServiceMap_Service_is_assigned_to_a_variable_here_but_this_API_is_intended_for_a_class_declaration_shape_such_as_0_effect_serviceNotAsClass.Code()},
	FixIDs:      []string{"serviceNotAsClass_fix"},
	Run:         runServiceNotAsClassFix,
}

func runServiceNotAsClassFix(ctx *fixable.Context) []ls.CodeAction {
	c := ctx.Checker

	sf := ctx.SourceFile

	matches := rules.AnalyzeServiceNotAsClass(ctx.TypeParser, c, sf)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		if action := ctx.NewFixAction(fixable.FixAction{
			Description: "Convert to class declaration",
			Run: func(tracker *change.Tracker) {
				callExpr := match.CallExprNode.AsCallExpression()

				// Build ServiceMap.Service property access
				serviceMapService := tracker.NewPropertyAccessExpression(
					tracker.NewIdentifier("ServiceMap"),
					nil,
					tracker.NewIdentifier("Service"),
					ast.NodeFlagsNone,
				)

				// Build combined type arguments: <ClassName, ...OriginalTypeArgs>
				selfTypeRef := tracker.NewTypeReferenceNode(tracker.NewIdentifier(match.VariableName), nil)
				typeArgNodes := []*ast.Node{selfTypeRef}
				if callExpr.TypeArguments != nil {
					for _, ta := range callExpr.TypeArguments.Nodes {
						typeArgNodes = append(typeArgNodes, tracker.DeepCloneNode(ta))
					}
				}

				// Build inner call: ServiceMap.Service<Self, ...TypeArgs>()
				innerCall := tracker.NewCallExpression(
					serviceMapService,
					nil,
					tracker.NewNodeList(typeArgNodes),
					nil,
					ast.NodeFlagsNone,
				)

				// Build outer call: innerCall(args...)
				var clonedArgs *ast.NodeList
				if callExpr.Arguments != nil && len(callExpr.Arguments.Nodes) > 0 {
					argNodes := make([]*ast.Node, len(callExpr.Arguments.Nodes))
					for i, arg := range callExpr.Arguments.Nodes {
						argNodes[i] = tracker.DeepCloneNode(arg)
					}
					clonedArgs = tracker.NewNodeList(argNodes)
				}
				outerCall := tracker.NewCallExpression(
					innerCall,
					nil,
					nil,
					clonedArgs,
					ast.NodeFlagsNone,
				)

				// Build heritage clause: extends outerCall
				exprWithTypeArgs := tracker.NewExpressionWithTypeArguments(outerCall, nil)
				heritageClause := tracker.NewHeritageClause(
					ast.KindExtendsKeyword,
					tracker.NewNodeList([]*ast.Node{exprWithTypeArgs}),
				)

				// Build modifiers for the class declaration
				var modifiers *ast.ModifierList
				if match.ModifierNodes != nil && len(match.ModifierNodes.Nodes) > 0 {
					modNodes := make([]*ast.Node, len(match.ModifierNodes.Nodes))
					for i, mod := range match.ModifierNodes.Nodes {
						modNodes[i] = tracker.NewModifier(mod.Kind)
					}
					modifiers = tracker.NewModifierList(modNodes)
				}

				// Build class declaration
				classDecl := tracker.NewClassDeclaration(
					modifiers,
					tracker.NewIdentifier(match.VariableName),
					nil, // no type parameters
					tracker.NewNodeList([]*ast.Node{heritageClause}),
					tracker.NewNodeList([]*ast.Node{}), // empty members
				)

				ast.SetParentInChildren(classDecl)
				tracker.ReplaceNode(sf, match.TargetNode, classDecl, nil)
			},
		}); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}
