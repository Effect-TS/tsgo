package rules

import (
	"slices"
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

var ExperimentalApiUsage = rule.Rule{
	Name:            "experimentalApiUsage",
	Group:           "correctness",
	Description:     "Warns when using an API marked @stability experimental",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.X_0_is_an_experimental_API_effect_experimentalApiUsage.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runStabilityApiUsage(ctx, "experimental")
	},
}

var UnstableApiUsage = rule.Rule{
	Name:            "unstableApiUsage",
	Group:           "correctness",
	Description:     "Warns when using an API marked @stability unstable",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.X_0_is_an_unstable_API_Breaking_changes_may_happen_between_versions_effect_unstableApiUsage.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runStabilityApiUsage(ctx, "unstable")
	},
}

func runStabilityApiUsage(ctx *rule.Context, wanted string) []*ast.Diagnostic {
	// Most references to a given API share its symbol. Cache declaration lookups so
	// lazy JSDoc parsing happens only once per declaration in this source file.
	declarationStability := make(map[*ast.Node]string)
	readDeclaration := func(declaration *ast.Node) string {
		if stability, ok := declarationStability[declaration]; ok {
			return stability
		}
		stability := stabilityOfDeclaration(declaration)
		declarationStability[declaration] = stability
		return stability
	}
	readSymbol := func(symbol *ast.Symbol) string {
		for depth := 0; symbol != nil && depth < 32; depth++ {
			for _, declaration := range symbol.Declarations {
				if stability := readDeclaration(declaration); stability != "" {
					return stability
				}
			}
			if symbol.Flags&ast.SymbolFlagsAlias == 0 {
				break
			}
			next := ctx.Checker.GetImmediateAliasedSymbol(symbol)
			if next == symbol {
				break
			}
			symbol = next
		}
		return ""
	}

	var diagnostics []*ast.Diagnostic
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindIdentifier && !ast.IsDeclarationNameOrImportPropertyName(node) {
			// The selected overload is authoritative for calls. A tagged overload
			// may differ from other declarations of the same symbol.
			stability := ""
			selectedDeclaration := (*ast.Node)(nil)
			callee := node
			if parent := node.Parent; parent != nil && parent.Kind == ast.KindPropertyAccessExpression && parent.AsPropertyAccessExpression().Name() == node {
				callee = parent
			}
			if parent := callee.Parent; parent != nil && (parent.Kind == ast.KindCallExpression || parent.Kind == ast.KindNewExpression) && parent.Expression() == callee {
				if signature := ctx.Checker.GetResolvedSignature(parent); signature != nil && signature.Declaration() != nil {
					selectedDeclaration = signature.Declaration()
					stability = readDeclaration(selectedDeclaration)
				}
			}
			symbol := ctx.Checker.GetSymbolAtLocation(node)
			resolvedSymbol := ctx.TypeParser.ReferenceSymbolAtNode(node)
			useSymbol := selectedDeclaration == nil || !symbolHasDeclaration(symbol, selectedDeclaration) && !symbolHasDeclaration(resolvedSymbol, selectedDeclaration)
			if stability == "" && useSymbol {
				stability = readSymbol(symbol)
			}
			if stability == "" && useSymbol {
				stability = readSymbol(resolvedSymbol)
			}
			if stability == wanted {
				message := tsdiag.X_0_is_an_unstable_API_Breaking_changes_may_happen_between_versions_effect_unstableApiUsage
				if wanted == "experimental" {
					message = tsdiag.X_0_is_an_experimental_API_effect_experimentalApiUsage
				}
				diagnostics = append(diagnostics, ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(node), message, nil, node.Text()))
			}
		}
		node.ForEachChild(walk)
		return false
	}
	walk(ctx.SourceFile.AsNode())
	return diagnostics
}

func symbolHasDeclaration(symbol *ast.Symbol, declaration *ast.Node) bool {
	if symbol == nil || declaration == nil {
		return false
	}
	return slices.Contains(symbol.Declarations, declaration)
}

func stabilityOfDeclaration(declaration *ast.Node) string {
	if declaration == nil {
		return ""
	}
	// A variable's JSDoc is usually attached to its VariableStatement.
	nodes := []*ast.Node{declaration}
	if declaration.Parent != nil && declaration.Parent.Kind == ast.KindVariableDeclarationList && declaration.Parent.Parent != nil && declaration.Parent.Parent.Kind == ast.KindVariableStatement {
		nodes = append(nodes, declaration.Parent.Parent)
	}
	for _, node := range nodes {
		if node.Flags&ast.NodeFlagsPossiblyContainsStabilityTag == 0 {
			continue
		}
		for _, doc := range node.JSDoc(nil) {
			if doc.AsJSDoc().Tags == nil {
				continue
			}
			for _, tag := range doc.AsJSDoc().Tags.Nodes {
				if tag.Kind != ast.KindJSDocUnknownTag || tag.TagName().Text() != "stability" {
					continue
				}
				comment := tag.AsJSDocUnknownTag().Comment
				if comment != nil && len(comment.Nodes) > 0 && comment.Nodes[0].Kind == ast.KindJSDocText {
					value := strings.TrimSpace(comment.Nodes[0].Text())
					if value == "unstable" || value == "experimental" {
						return value
					}
				}
			}
		}
	}
	return ""
}
