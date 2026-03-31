package layergraph

import (
	"sort"

	"github.com/effect-ts/tsgo/internal/graph"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/scanner"
)

// workItem represents a node to visit in the DFS work queue.
type workItem struct {
	node  *ast.Node
	depth int
}

// ExtractLayerGraph builds a directed graph of Layer composition from the given AST node.
// It performs a DFS with a two-pass approach using an explicit work stack:
//   - First pass: push children for processing
//   - Second pass: link processed children into the graph
func ExtractLayerGraph(
	tp *typeparser.TypeParser,
	c *checker.Checker,
	node *ast.Node,
	sf *ast.SourceFile,
	opts ExtractLayerGraphOptions,
) *graph.Graph[LayerGraphNodeInfo, LayerGraphEdgeInfo] {
	g := graph.New[LayerGraphNodeInfo, LayerGraphEdgeInfo]()
	nodeToGraphIndex := make(map[*ast.Node]graph.NodeIndex)
	visitedNodes := make(map[*ast.Node]bool)
	nodeInPipeContext := make(map[*ast.Node]bool)
	depthBudget := make(map[*ast.Node]int)

	// Resolve the Layer module import name for ExplodeOnlyLayerCalls checks.
	layerModuleName := findLayerModuleName(sf)

	// Initialize the work stack with the root node.
	stack := []workItem{{node: node, depth: opts.FollowSymbolsDepth}}
	depthBudget[node] = opts.FollowSymbolsDepth

	appendNodeToVisit := func(n *ast.Node, depth int) {
		depthBudget[n] = depth
		stack = append(stack, workItem{node: n, depth: depth})
	}

	addNode := func(n *ast.Node, info LayerGraphNodeInfo) graph.NodeIndex {
		idx := g.AddNode(info)
		nodeToGraphIndex[n] = idx
		return idx
	}

	for len(stack) > 0 {
		// Pop from the stack (LIFO).
		item := stack[len(stack)-1]
		stack = stack[:len(stack)-1]
		current := item.node
		currentDepth := depthBudget[current]

		// Case 1: Pipe detection
		if pipeResult := tp.ParsePipeCall(current); pipeResult != nil {
			if !visitedNodes[current] {
				// First pass: push self back, then subject and args.
				appendNodeToVisit(current, currentDepth)
				appendNodeToVisit(pipeResult.Subject, currentDepth)
				for _, arg := range pipeResult.Args {
					appendNodeToVisit(arg, currentDepth)
					nodeInPipeContext[arg] = true
				}
				visitedNodes[current] = true
			} else {
				// Second pass: collect child graph indices.
				allChildren := make([]*ast.Node, 0, 1+len(pipeResult.Args))
				allChildren = append(allChildren, pipeResult.Subject)
				allChildren = append(allChildren, pipeResult.Args...)

				childIndices := collectChildIndices(allChildren, nodeToGraphIndex, g)

				if len(childIndices) == len(allChildren) {
					// All members are graph nodes — link them sequentially.
					var lastIdx graph.NodeIndex
					for i, childIdx := range childIndices {
						if i > 0 {
							g.AddEdge(childIdx, lastIdx, LayerGraphEdgeInfo{Relationship: EdgeRelationshipPipe})
						}
						lastIdx = childIdx
					}
					// Add a node for the pipe call itself, linking to the last child.
					pipeIdx := addNode(current, extractNodeInfo(tp, c, current, sf, nodeInPipeContext[current]))
					g.AddEdge(pipeIdx, lastIdx, LayerGraphEdgeInfo{Relationship: EdgeRelationshipPipe})
				} else {
					// Not all children are graph nodes — remove partial children and try as leaf.
					removePartialChildren(allChildren, nodeToGraphIndex, g)
					info := extractNodeInfo(tp, c, current, sf, nodeInPipeContext[current])
					if info.LayerType != nil {
						addNode(current, info)
					}
				}
			}
			continue
		}

		// Case 2: Call expression
		if current.Kind == ast.KindCallExpression {
			callExpr := current.AsCallExpression()
			shouldExplode := !opts.ExplodeOnlyLayerCalls
			if opts.ExplodeOnlyLayerCalls {
				if isLayerModuleCall(callExpr, layerModuleName) {
					shouldExplode = true
				}
			}
			if shouldExplode {
				var args []*ast.Node
				if callExpr.Arguments != nil {
					args = callExpr.Arguments.Nodes
				}
				if !visitedNodes[current] {
					// First pass: push self back, then all arguments.
					appendNodeToVisit(current, currentDepth)
					for _, arg := range args {
						appendNodeToVisit(arg, currentDepth)
					}
					visitedNodes[current] = true
				} else {
					// Second pass: collect child graph indices.
					childIndices := collectChildIndices(args, nodeToGraphIndex, g)

					if len(childIndices) == len(args) {
						// All arguments are graph nodes.
						callIdx := addNode(current, extractNodeInfo(tp, c, current, sf, nodeInPipeContext[current]))
						for i, childIdx := range childIndices {
							g.AddEdge(callIdx, childIdx, LayerGraphEdgeInfo{
								Relationship:  EdgeRelationshipCall,
								ArgumentIndex: i,
							})
						}
					} else {
						// Not all arguments are graph nodes.
						removePartialChildren(args, nodeToGraphIndex, g)
						info := extractNodeInfo(tp, c, current, sf, nodeInPipeContext[current])
						if info.LayerType != nil {
							addNode(current, info)
						}
					}
				}
				continue
			}
		}

		// Case 3: Array literal (when ArrayLiteralAsMerge is enabled)
		if opts.ArrayLiteralAsMerge && current.Kind == ast.KindArrayLiteralExpression {
			arrayExpr := current.AsArrayLiteralExpression()
			var elements []*ast.Node
			if arrayExpr.Elements != nil {
				elements = arrayExpr.Elements.Nodes
			}
			if !visitedNodes[current] {
				// First pass: push self back, then all elements.
				appendNodeToVisit(current, currentDepth)
				for _, elem := range elements {
					appendNodeToVisit(elem, currentDepth)
				}
				visitedNodes[current] = true
			} else {
				// Second pass: collect child graph indices (doesn't require ALL).
				childIndices := collectChildIndices(elements, nodeToGraphIndex, g)
				if len(childIndices) > 0 {
					arrayIdx := addNode(current, extractNodeInfo(tp, c, current, sf, nodeInPipeContext[current]))
					for i, childIdx := range childIndices {
						g.AddEdge(arrayIdx, childIdx, LayerGraphEdgeInfo{
							Relationship: EdgeRelationshipArrayLiteral,
							Index:        i,
						})
					}
				}
			}
			continue
		}

		// Case 4: Symbol following
		if currentDepth > 0 && isSimpleIdentifier(current) {
			sym := c.GetSymbolAtLocation(current)
			if sym != nil {
				if sym.Flags&ast.SymbolFlagsAlias != 0 {
					resolved := checker.SkipAlias(sym, c)
					if resolved != nil {
						sym = resolved
					}
				}
				if len(sym.Declarations) == 1 {
					declNode := getAdjustedNode(sym.Declarations[0])
					if declNode != nil {
						if !visitedNodes[current] {
							// First pass: push self back, push declaration with decremented depth.
							appendNodeToVisit(current, currentDepth)
							appendNodeToVisit(declNode, currentDepth-1)
							visitedNodes[current] = true
							continue
						}
						// Second pass: link to declaration if it's in the graph.
						if childIdx, ok := nodeToGraphIndex[declNode]; ok {
							identIdx := addNode(current, extractNodeInfo(tp, c, current, sf, nodeInPipeContext[current]))
							g.AddEdge(identIdx, childIdx, LayerGraphEdgeInfo{Relationship: EdgeRelationshipSymbol})
							continue
						}
					}
				}
			}
		}

		// Case 5: Leaf node (base case)
		if ast.IsExpression(current) {
			info := extractNodeInfo(tp, c, current, sf, nodeInPipeContext[current])
			if info.LayerType != nil {
				addNode(current, info)
			}
		}
	}

	return g
}

// extractNodeInfo computes the LayerGraphNodeInfo for a given AST node.
func extractNodeInfo(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node, _ *ast.SourceFile, inPipeContext bool) LayerGraphNodeInfo {
	info := LayerGraphNodeInfo{
		Node:        node,
		DisplayNode: getDisplayNode(node),
	}

	// Get the type of the node.
	// When in a pipe context, resolve the contextual type's return type
	// (the pipe argument is a function whose return type is the Layer).
	var t *checker.Type
	if inPipeContext && ast.IsExpression(node) {
		contextualType := c.GetContextualType(node, checker.ContextFlagsNone)
		if contextualType != nil {
			callSignatures := c.GetSignaturesOfType(contextualType, checker.SignatureKindCall)
			if len(callSignatures) == 1 {
				t = c.GetReturnTypeOfSignature(callSignatures[0])
			}
		}
	} else {
		t = tp.GetTypeAtLocation(node)
	}
	if t == nil {
		return info
	}

	// Parse as Layer type.
	layer := tp.LayerType(t, node)
	if layer == nil {
		return info
	}
	info.LayerType = layer

	// Unroll provides and requires, filtering out Never types.
	for _, p := range typeparser.UnrollUnionMembers(layer.ROut) {
		if p.Flags()&checker.TypeFlagsNever == 0 {
			info.Provides = append(info.Provides, p)
		}
	}
	for _, r := range typeparser.UnrollUnionMembers(layer.RIn) {
		if r.Flags()&checker.TypeFlagsNever == 0 {
			info.Requires = append(info.Requires, r)
		}
	}

	// ActualProvides: provides that are NOT assignable to the RIn type.
	for _, p := range info.Provides {
		if !checker.Checker_isTypeAssignableTo(c, p, layer.RIn) {
			info.ActualProvides = append(info.ActualProvides, p)
		}
	}

	// Sort all type lists deterministically by their string representation.
	// NOTE: Intentional divergence from .repos reference. The TypeScript implementation
	// preserves union member order in output/nested formats (using deterministicTypeOrder
	// only for providers/requirers extraction and quickinfo). We sort alphabetically
	// everywhere for consistent determinism independent of compiler-internal union ordering.
	sortTypes := func(types []*checker.Type) {
		sort.Slice(types, func(i, j int) bool {
			return c.TypeToString(types[i]) < c.TypeToString(types[j])
		})
	}
	sortTypes(info.Provides)
	sortTypes(info.Requires)
	sortTypes(info.ActualProvides)

	return info
}

// getDisplayNode returns the node used for display purposes.
// If the node's parent is a variable declaration and the node is the initializer,
// use the variable's name node instead.
func getDisplayNode(node *ast.Node) *ast.Node {
	if node.Parent != nil && node.Parent.Kind == ast.KindVariableDeclaration {
		varDecl := node.Parent.AsVariableDeclaration()
		if varDecl.Initializer != nil && varDecl.Initializer == node {
			return varDecl.Name()
		}
	}
	if node.Parent != nil && node.Parent.Kind == ast.KindPropertyDeclaration {
		propDecl := node.Parent.AsPropertyDeclaration()
		if propDecl.Initializer != nil && propDecl.Initializer == node {
			return propDecl.Name()
		}
	}
	return node
}

// getAdjustedNode extracts the initializer expression from a declaration node
// to follow during symbol resolution.
func getAdjustedNode(node *ast.Node) *ast.Node {
	switch node.Kind {
	case ast.KindVariableDeclaration:
		return node.AsVariableDeclaration().Initializer
	case ast.KindPropertyDeclaration:
		return node.AsPropertyDeclaration().Initializer
	default:
		if ast.IsExpression(node) {
			return node
		}
		return nil
	}
}

// isSimpleIdentifier checks if a node is a simple identifier or a chain of property accesses
// with simple identifiers (e.g., `a`, `a.b`, `a.b.c`).
func isSimpleIdentifier(node *ast.Node) bool {
	if node.Kind == ast.KindIdentifier {
		return true
	}
	if node.Kind == ast.KindPropertyAccessExpression {
		prop := node.AsPropertyAccessExpression()
		return prop.Name() != nil && prop.Name().Kind == ast.KindIdentifier && isSimpleIdentifier(prop.Expression)
	}
	return false
}

// isLayerModuleCall checks if a call expression's callee is a Layer module API call
// (e.g., Layer.provide, Layer.merge, etc.).
func isLayerModuleCall(callExpr *ast.CallExpression, layerModuleName string) bool {
	expr := callExpr.Expression
	if expr.Kind != ast.KindPropertyAccessExpression {
		return false
	}
	propAccess := expr.AsPropertyAccessExpression()
	if propAccess.Expression.Kind != ast.KindIdentifier {
		return false
	}
	return scanner.GetTextOfNode(propAccess.Expression) == layerModuleName
}

// findLayerModuleName resolves the imported identifier name for the "Layer" export
// from the "effect" package. Falls back to "Layer" if not found.
func findLayerModuleName(sf *ast.SourceFile) string {
	if sf == nil {
		return "Layer"
	}
	for _, stmt := range sf.Statements.Nodes {
		if stmt.Kind != ast.KindImportDeclaration {
			continue
		}
		importDecl := stmt.AsImportDeclaration()
		if importDecl.ModuleSpecifier == nil {
			continue
		}
		moduleName := scanner.GetTextOfNode(importDecl.ModuleSpecifier)
		// Strip quotes from module specifier.
		if len(moduleName) >= 2 && (moduleName[0] == '"' || moduleName[0] == '\'') {
			moduleName = moduleName[1 : len(moduleName)-1]
		}
		if moduleName != "effect" {
			continue
		}
		// Look for named imports.
		if importDecl.ImportClause == nil {
			continue
		}
		clause := importDecl.ImportClause.AsImportClause()
		if clause.NamedBindings == nil || clause.NamedBindings.Kind != ast.KindNamedImports {
			continue
		}
		namedImports := clause.NamedBindings.AsNamedImports()
		if namedImports.Elements == nil {
			continue
		}
		for _, elem := range namedImports.Elements.Nodes {
			spec := elem.AsImportSpecifier()
			// The "real" name is PropertyName if set (import { Layer as X }),
			// otherwise Name.
			importedName := ""
			if spec.PropertyName != nil {
				importedName = scanner.GetTextOfNode(spec.PropertyName)
			} else {
				importedName = scanner.GetTextOfNode(spec.Name())
			}
			if importedName == "Layer" {
				return scanner.GetTextOfNode(spec.Name())
			}
		}
	}
	return "Layer"
}

// collectChildIndices gathers graph node indices for the given AST nodes,
// filtering out nodes that aren't in the graph.
func collectChildIndices(
	nodes []*ast.Node,
	nodeToGraphIndex map[*ast.Node]graph.NodeIndex,
	g *graph.Graph[LayerGraphNodeInfo, LayerGraphEdgeInfo],
) []graph.NodeIndex {
	var indices []graph.NodeIndex
	for _, n := range nodes {
		if idx, ok := nodeToGraphIndex[n]; ok && g.HasNode(idx) {
			indices = append(indices, idx)
		}
	}
	return indices
}

// removePartialChildren removes graph nodes for the given AST nodes.
func removePartialChildren(
	nodes []*ast.Node,
	nodeToGraphIndex map[*ast.Node]graph.NodeIndex,
	g *graph.Graph[LayerGraphNodeInfo, LayerGraphEdgeInfo],
) {
	for _, n := range nodes {
		if idx, ok := nodeToGraphIndex[n]; ok {
			g.RemoveNode(idx)
			delete(nodeToGraphIndex, n)
		}
	}
}
