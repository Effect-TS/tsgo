package refactors

import (
	"sort"
	"strings"

	"github.com/effect-ts/effect-typescript-go/internal/effectutil"
	"github.com/effect-ts/effect-typescript-go/internal/layergraph"
	"github.com/effect-ts/effect-typescript-go/internal/refactor"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/astnav"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/ls/change"
)

var LayerMagic = refactor.Refactor{
	Name:        "layerMagic",
	Description: "Layer Magic",
	Kind:        "rewrite.effect.layerMagic",
	Run:         runLayerMagic,
}

func runLayerMagic(ctx *refactor.Context) []ls.CodeAction {
	token := astnav.GetTokenAtPosition(ctx.SourceFile, ctx.Span.Pos())
	if token == nil {
		return nil
	}

	c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)
	if c == nil {
		return nil
	}
	defer done()

	layerIdentifier := effectutil.FindModuleIdentifier(ctx.SourceFile, "Layer")

	// Collect ancestor nodes from the token up to the source file.
	var ancestors []*ast.Node
	for node := token; node != nil && node.Kind != ast.KindSourceFile; node = node.Parent {
		ancestors = append(ancestors, node)
	}

	// Try build first on all ancestors, then prepare as fallback.
	for _, node := range ancestors {
		action := tryBuildRefactor(ctx, c, node, layerIdentifier)
		if action != nil {
			return action
		}
	}
	for _, node := range ancestors {
		action := tryPrepareRefactor(ctx, c, node, layerIdentifier)
		if action != nil {
			return action
		}
	}

	return nil
}

// adjustedNode redirects from a variable/property declaration name to the initializer.
func adjustedNode(node *ast.Node) *ast.Node {
	if node.Parent != nil && node.Kind == ast.KindIdentifier {
		parent := node.Parent
		if parent.Kind == ast.KindVariableDeclaration {
			vd := parent.AsVariableDeclaration()
			if vd.Initializer != nil && vd.Name() == node {
				return vd.Initializer
			}
		}
		if parent.Kind == ast.KindPropertyDeclaration {
			pd := parent.AsPropertyDeclaration()
			if pd.Initializer != nil && pd.Name() == node {
				return pd.Initializer
			}
		}
	}
	return node
}

// tryBuildRefactor tries to produce the "build" refactor action for the given node.
// It detects the `(expr as any) as Layer.Layer<ROut>` pattern.
func tryBuildRefactor(ctx *refactor.Context, c *checker.Checker, node *ast.Node, layerIdentifier string) []ls.CodeAction {
	atLocation := adjustedNode(node)

	// Must be an AsExpression: `... as Layer.Layer<ROut>`
	if atLocation.Kind != ast.KindAsExpression {
		return nil
	}
	outerAs := atLocation.AsAsExpression()
	if outerAs.Type == nil || outerAs.Type.Kind != ast.KindTypeReference {
		return nil
	}

	// Inner expression must be `expr as any`
	innerExpr := outerAs.Expression
	if innerExpr.Kind != ast.KindAsExpression {
		return nil
	}
	innerAs := innerExpr.AsAsExpression()
	if innerAs.Type == nil || innerAs.Type.Kind != ast.KindAnyKeyword {
		return nil
	}

	// Parse the outer type as a Layer type to get ROut
	outerType := typeparser.GetTypeAtLocation(c, outerAs.Type)
	if outerType == nil {
		return nil
	}
	layer := typeparser.LayerType(c, outerType, outerAs.Type)
	if layer == nil {
		return nil
	}

	// The inner expression (before `as any`) is the casted structure
	castedStructure := innerAs.Expression

	// Extract layer graph from the casted structure
	layerGraph := layergraph.ExtractLayerGraph(c, castedStructure, ctx.SourceFile, layergraph.ExtractLayerGraphOptions{
		ArrayLiteralAsMerge:   true,
		ExplodeOnlyLayerCalls: true,
		FollowSymbolsDepth:    0,
	})
	if layerGraph == nil {
		return nil
	}

	// Extract outline graph
	outlineGraph := layergraph.ExtractOutlineGraph(c, layerGraph)
	if outlineGraph == nil || outlineGraph.NodeCount() <= 1 {
		return nil
	}

	// Get target output types from the Layer ROut
	targetOutputTypes := typeparser.UnrollUnionMembers(layer.ROut)

	// Convert to magic result
	magicResult := layergraph.ConvertOutlineGraphToLayerMagic(c, outlineGraph, targetOutputTypes)
	if magicResult == nil || len(magicResult.Nodes) == 0 {
		return nil
	}

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Compose layers automatically with target output services",
		Run: func(tracker *change.Tracker) {
			buildLayerMagicBuild(tracker, ctx, c, atLocation, magicResult, layerIdentifier)
		},
	})
	if action == nil {
		return nil
	}
	action.Kind = "refactor.rewrite.effect.layerMagicBuild"
	return []ls.CodeAction{*action}
}

// buildLayerMagicBuild generates: firstLayer.pipe(Layer.provideMerge(...), Layer.provide(...), ...)
func buildLayerMagicBuild(tracker *change.Tracker, ctx *refactor.Context, c *checker.Checker, oldNode *ast.Node, magicResult *layergraph.LayerMagicResult, layerIdentifier string) {
	nodes := magicResult.Nodes
	if len(nodes) == 0 {
		return
	}

	// Clone the first node's expression
	firstExpr := tracker.DeepCloneNode(nodes[0].Node)

	// Build pipe arguments for remaining nodes
	var pipeArgs []*ast.Node
	for _, mn := range nodes[1:] {
		// Determine combinator name
		var combinatorName string
		if mn.Merges && mn.Provides {
			combinatorName = "provideMerge"
		} else if mn.Merges {
			combinatorName = "merge"
		} else {
			combinatorName = "provide"
		}

		// Layer.provideMerge(expr) or Layer.provide(expr) or Layer.merge(expr)
		layerId := tracker.NewIdentifier(layerIdentifier)
		combinatorAccess := tracker.NewPropertyAccessExpression(
			layerId, nil, tracker.NewIdentifier(combinatorName), ast.NodeFlagsNone,
		)
		clonedExpr := tracker.DeepCloneNode(mn.Node)
		call := tracker.NewCallExpression(
			combinatorAccess, nil, nil,
			tracker.NewNodeList([]*ast.Node{clonedExpr}),
			ast.NodeFlagsNone,
		)
		pipeArgs = append(pipeArgs, call)
	}

	// firstExpr.pipe(args...)
	pipeAccess := tracker.NewPropertyAccessExpression(
		firstExpr, nil, tracker.NewIdentifier("pipe"), ast.NodeFlagsNone,
	)
	newDeclaration := tracker.NewCallExpression(
		pipeAccess, nil, nil,
		tracker.NewNodeList(pipeArgs),
		ast.NodeFlagsNone,
	)

	// Add trailing comment for missing output types
	if len(magicResult.MissingOutputTypes) > 0 {
		var typeNames []string
		for _, t := range magicResult.MissingOutputTypes {
			typeNames = append(typeNames, c.TypeToStringEx(t, nil, checker.TypeFormatFlagsNoTruncation))
		}
		comment := " Unable to find " + strings.Join(typeNames, ", ") + " in the provided layers. "
		newDeclaration = tracker.AddSyntheticTrailingComment(newDeclaration, ast.KindMultiLineCommentTrivia, comment, false)
	}

	ast.SetParentInChildren(newDeclaration)

	// If the old node is an expression inside a variable declaration, use replaceExpressionViaStatement
	if oldNode.Parent != nil && oldNode.Parent.Kind == ast.KindVariableDeclaration {
		replaceExpressionViaStatement(tracker, ctx.SourceFile, oldNode, newDeclaration)
	} else {
		tracker.ReplaceNode(ctx.SourceFile, oldNode, newDeclaration, nil)
	}
}

// tryPrepareRefactor tries to produce the "prepare" refactor action for the given node.
// It flattens a layer expression into `[...] as any as Layer.Layer<T>`.
func tryPrepareRefactor(ctx *refactor.Context, c *checker.Checker, node *ast.Node, layerIdentifier string) []ls.CodeAction {
	atLocation := adjustedNode(node)

	// Skip if already in `as any as Layer.Layer<T>` form
	if isAsAnyAsLayerForm(atLocation) {
		return nil
	}

	// Must be an expression
	if !ast.IsExpression(atLocation) {
		return nil
	}

	// Extract layer graph
	layerGraph := layergraph.ExtractLayerGraph(c, atLocation, ctx.SourceFile, layergraph.ExtractLayerGraphOptions{
		ArrayLiteralAsMerge:   true,
		ExplodeOnlyLayerCalls: true,
		FollowSymbolsDepth:    0,
	})
	if layerGraph == nil {
		return nil
	}

	// Extract outline graph
	outlineGraph := layergraph.ExtractOutlineGraph(c, layerGraph)
	if outlineGraph == nil || outlineGraph.NodeCount() <= 1 {
		return nil
	}

	// Collect all Provides types from outline nodes
	layerOutputTypes := make(map[*checker.Type]bool)
	for _, nodeInfo := range outlineGraph.Nodes() {
		for _, p := range nodeInfo.Provides {
			layerOutputTypes[p] = true
		}
	}

	// Collect leaf layer nodes (sorted by position)
	var layerNodes []*ast.Node
	for _, nodeInfo := range outlineGraph.Nodes() {
		if nodeInfo.Node != nil && ast.IsExpression(nodeInfo.Node) {
			layerNodes = append(layerNodes, nodeInfo.Node)
		}
	}
	sort.Slice(layerNodes, func(i, j int) bool {
		return astnav.GetStartOfNode(layerNodes[i], ctx.SourceFile, false) <
			astnav.GetStartOfNode(layerNodes[j], ctx.SourceFile, false)
	})

	if len(layerNodes) == 0 {
		return nil
	}

	// Parse the expression's current type as a Layer to find "previously provided" types
	exprType := typeparser.GetTypeAtLocation(c, atLocation)
	var previouslyProvided *checker.Type
	if exprType != nil {
		parsedLayer := typeparser.LayerType(c, exprType, atLocation)
		if parsedLayer != nil {
			previouslyProvided = parsedLayer.ROut
		}
	}

	// Partition types into newlyIntroduced and existingBefore
	allOutputTypes := sortedTypeSlice(c, layerOutputTypes)
	var newlyIntroduced []*checker.Type
	var existingBefore []*checker.Type
	for _, t := range allOutputTypes {
		if previouslyProvided != nil && checker.Checker_isTypeAssignableTo(c, t, previouslyProvided) {
			existingBefore = append(existingBefore, t)
		} else {
			newlyIntroduced = append(newlyIntroduced, t)
		}
	}

	action := ctx.NewRefactorAction(refactor.RefactorAction{
		Description: "Prepare layers for automatic composition",
		Run: func(tracker *change.Tracker) {
			buildLayerMagicPrepare(tracker, ctx, c, atLocation, layerNodes, newlyIntroduced, existingBefore, layerIdentifier)
		},
	})
	if action == nil {
		return nil
	}
	action.Kind = "refactor.rewrite.effect.layerMagicPrepare"
	return []ls.CodeAction{*action}
}

// buildLayerMagicPrepare generates: [leaf1, leaf2, ...] as any as Layer.Layer<NewTypes /* ExistingTypes */>
func buildLayerMagicPrepare(
	tracker *change.Tracker,
	ctx *refactor.Context,
	c *checker.Checker,
	oldNode *ast.Node,
	layerNodes []*ast.Node,
	newlyIntroduced []*checker.Type,
	existingBefore []*checker.Type,
	layerIdentifier string,
) {
	// Build array of cloned layer expressions
	var elements []*ast.Node
	for _, entry := range layerNodes {
		elements = append(elements, tracker.DeepCloneNode(entry))
	}
	arrayLiteral := tracker.NewArrayLiteralExpression(tracker.NewNodeList(elements), false)

	// Build `any` type node
	anyType := tracker.NewKeywordTypeNode(ast.KindAnyKeyword)

	// Inner as expression: `[...] as any`
	innerAs := tracker.NewAsExpression(arrayLiteral, anyType)

	// Build the provides union type node
	var providesType *ast.Node
	if len(newlyIntroduced) == 0 {
		providesType = tracker.NewTypeReferenceNode(tracker.NewIdentifier("never"), nil)
	} else {
		var typeNodes []*ast.Node
		for _, t := range newlyIntroduced {
			typeStr := c.TypeToStringEx(t, nil, checker.TypeFormatFlagsNoTruncation)
			typeNodes = append(typeNodes, tracker.NewTypeReferenceNode(tracker.NewIdentifier(typeStr), nil))
		}
		if len(typeNodes) == 1 {
			providesType = typeNodes[0]
		} else {
			providesType = tracker.NewUnionTypeNode(tracker.NewNodeList(typeNodes))
		}
	}

	// Add trailing comment for existing types
	if len(existingBefore) > 0 {
		var typeStrings []string
		for _, t := range existingBefore {
			typeStrings = append(typeStrings, c.TypeToStringEx(t, nil, checker.TypeFormatFlagsNoTruncation))
		}
		comment := " " + strings.Join(typeStrings, " | ") + " "
		providesType = tracker.AddSyntheticTrailingComment(providesType, ast.KindMultiLineCommentTrivia, comment, false)
	}

	// Build Layer.Layer<providesType>
	qualifiedName := tracker.NewQualifiedName(tracker.NewIdentifier(layerIdentifier), tracker.NewIdentifier("Layer"))
	layerTypeRef := tracker.NewTypeReferenceNode(qualifiedName, tracker.NewNodeList([]*ast.Node{providesType}))

	// Outer as expression: `([...] as any) as Layer.Layer<T>`
	outerAs := tracker.NewAsExpression(innerAs, layerTypeRef)

	ast.SetParentInChildren(outerAs)

	// If the old node is an expression inside a variable declaration, use replaceExpressionViaStatement
	if oldNode.Parent != nil && oldNode.Parent.Kind == ast.KindVariableDeclaration {
		replaceExpressionViaStatement(tracker, ctx.SourceFile, oldNode, outerAs)
	} else {
		tracker.ReplaceNode(ctx.SourceFile, oldNode, outerAs, nil)
	}
}

// isAsAnyAsLayerForm checks if a node is already in the `(expr as any) as Layer.Layer<T>` form.
func isAsAnyAsLayerForm(node *ast.Node) bool {
	if node.Kind != ast.KindAsExpression {
		return false
	}
	outerAs := node.AsAsExpression()
	if outerAs.Type == nil || outerAs.Type.Kind != ast.KindTypeReference {
		return false
	}
	innerExpr := outerAs.Expression
	if innerExpr.Kind != ast.KindAsExpression {
		return false
	}
	innerAs := innerExpr.AsAsExpression()
	return innerAs.Type != nil && innerAs.Type.Kind == ast.KindAnyKeyword
}

// sortedTypeSlice converts a type set to a sorted slice using TypeToString for deterministic ordering.
func sortedTypeSlice(c *checker.Checker, types map[*checker.Type]bool) []*checker.Type {
	result := make([]*checker.Type, 0, len(types))
	for t := range types {
		result = append(result, t)
	}
	sort.Slice(result, func(i, j int) bool {
		return c.TypeToStringEx(result[i], nil, checker.TypeFormatFlagsNoTruncation) <
			c.TypeToStringEx(result[j], nil, checker.TypeFormatFlagsNoTruncation)
	})
	return result
}
