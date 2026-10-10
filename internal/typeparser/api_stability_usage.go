package typeparser

import (
	"slices"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// ApiStabilityUsage records the declared stability of one reference or
// contextual property. Contextual union candidates retain their original order
// so each rule can select its first non-allowed declaration independently.
type ApiStabilityUsage struct {
	Node         *ast.Node
	Name         string
	Declarations []ApiStabilityDeclaration
}

// ApiStabilityUsages shares one analysis per source file and checker between
// the experimental and unstable usage rules. Allowlist decisions, severities,
// and diagnostics belong to the individual rule invocations and are not cached.
func (tp *TypeParser) ApiStabilityUsages(sourceFile *ast.SourceFile) []ApiStabilityUsage {
	return Cached(&tp.links.ApiStabilityUsages, sourceFile, func() []ApiStabilityUsage {
		return tp.collectApiStabilityUsages(sourceFile)
	})
}

func (tp *TypeParser) collectApiStabilityUsages(sourceFile *ast.SourceFile) []ApiStabilityUsage {
	constraintTypes := make(map[*ast.Node]*checker.Type)
	var usages []ApiStabilityUsage
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		// Object literal keys declare local properties, but also use the matching
		// properties of their contextual type. Resolve those declarations before
		// the ordinary reference path excludes declaration names.
		if ast.IsObjectLiteralElement(node) && node.Name() != nil && node.Parent != nil && node.Parent.Kind == ast.KindObjectLiteralExpression {
			if name := ast.GetTextOfPropertyName(node.Name()); name != "" {
				if contextualType := tp.checker.GetContextualType(node.Parent, checker.ContextFlagsNone); contextualType != nil {
					// Keep the original symbols and let the checker filter union
					// branches by discriminants before reading their own tags.
					properties := tp.checker.GetPropertySymbolsFromContextualType(node, contextualType, false)
					// Generic inference can point back to the literal's own
					// property. Use the constraint instead, as go-to-definition
					// does, so its stability tag is not lost to inference.
					if slices.ContainsFunc(properties, func(symbol *ast.Symbol) bool { return symbol.ValueDeclaration == node }) {
						constraintType, cached := constraintTypes[node.Parent]
						if !cached {
							constraintType = tp.checker.GetContextualType(node.Parent, checker.ContextFlagsIgnoreNodeInferences)
							if len(node.Parent.AsObjectLiteralExpression().Properties.Nodes) > 1 {
								constraintTypes[node.Parent] = constraintType
							}
						}
						if constraintType != nil {
							if constraintProperties := tp.checker.GetPropertySymbolsFromContextualType(node, constraintType, false); len(constraintProperties) > 0 {
								properties = constraintProperties
							}
						}
					}
					var declarations []ApiStabilityDeclaration
					for _, symbol := range properties {
						if symbol.ValueDeclaration == node {
							continue
						}
						if stability := tp.DeclaredApiStabilityOfSymbol(symbol); stability.Level != ApiStabilityStable {
							declarations = append(declarations, stability)
						}
					}
					if len(declarations) > 0 {
						usages = append(usages, ApiStabilityUsage{Node: node.Name(), Name: name, Declarations: declarations})
					}
				}
			}
		}
		if node.Kind == ast.KindIdentifier && !ast.IsDeclarationNameOrImportPropertyName(node) {
			// The selected overload is authoritative for calls. A tagged overload
			// may differ from other declarations of the same symbol.
			stability := ApiStabilityDeclaration{}
			selectedDeclaration := (*ast.Node)(nil)
			callee := node
			if parent := node.Parent; parent != nil && parent.Kind == ast.KindPropertyAccessExpression && parent.AsPropertyAccessExpression().Name() == node {
				callee = parent
			}
			if parent := callee.Parent; parent != nil && (parent.Kind == ast.KindCallExpression || parent.Kind == ast.KindNewExpression) && parent.Expression() == callee {
				if signature := tp.checker.GetResolvedSignature(parent); signature != nil && signature.Declaration() != nil {
					selectedDeclaration = signature.Declaration()
					stability = tp.DeclaredApiStabilityOfSignature(signature)
					if stability.Level == ApiStabilityStable {
						// A signature with no own tag still carries the selected
						// declaration so the symbol fallback can be suppressed for it.
						stability = ApiStabilityDeclaration{Declaration: selectedDeclaration}
					}
				}
			}
			symbol := tp.checker.GetSymbolAtLocation(node)
			resolvedSymbol := tp.ReferenceSymbolAtNode(node)
			useSymbol := selectedDeclaration == nil || !symbolHasDeclaration(symbol, selectedDeclaration) && !symbolHasDeclaration(resolvedSymbol, selectedDeclaration)
			if stability.Level == ApiStabilityStable && useSymbol {
				stability = tp.DeclaredApiStabilityOfSymbol(symbol)
			}
			if stability.Level == ApiStabilityStable && useSymbol {
				stability = tp.DeclaredApiStabilityOfSymbol(resolvedSymbol)
			}
			if stability.Level != ApiStabilityStable {
				usages = append(usages, ApiStabilityUsage{Node: node, Name: node.Text(), Declarations: []ApiStabilityDeclaration{stability}})
			}
		}
		node.ForEachChild(walk)
		return false
	}
	walk(sourceFile.AsNode())
	return usages
}

func symbolHasDeclaration(symbol *ast.Symbol, declaration *ast.Node) bool {
	if symbol == nil || declaration == nil {
		return false
	}
	return slices.Contains(symbol.Declarations, declaration)
}
