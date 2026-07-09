package rules

import (
	"slices"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

var UnnecessaryTypeofType = rule.Rule{
	Name:            "unnecessaryTypeofType",
	Group:           "style",
	Description:     "Suggests replacing typeof Schema.Type style annotations with the matching named type when available",
	DefaultSeverity: etscore.SeveritySuggestion,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.This_typeof_Type_query_can_be_replaced_with_0_effect_unnecessaryTypeofType.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeUnnecessaryTypeofType(ctx.TypeParser, ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(m.SourceFile, m.Location, tsdiag.This_typeof_Type_query_can_be_replaced_with_0_effect_unnecessaryTypeofType, nil, m.ReplacementText)
		}
		return diags
	},
}

type UnnecessaryTypeofTypeMatch struct {
	SourceFile      *ast.SourceFile
	Location        core.TextRange
	QueryNode       *ast.Node
	InnerEntityName *ast.Node
	ReplacementText string
}

func AnalyzeUnnecessaryTypeofType(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile) []UnnecessaryTypeofTypeMatch {
	var matches []UnnecessaryTypeofTypeMatch

	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}

		if node.Kind == ast.KindTypeQuery {
			if match := analyzeUnnecessaryTypeofTypeNode(tp, c, sf, node); match != nil {
				matches = append(matches, *match)
			}
		}

		node.ForEachChild(walk)
		return false
	}

	walk(sf.AsNode())
	return matches
}

func analyzeUnnecessaryTypeofTypeNode(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, node *ast.Node) *UnnecessaryTypeofTypeMatch {
	query := node.AsTypeQueryNode()
	if query == nil || query.ExprName == nil {
		return nil
	}
	if query.TypeArguments != nil && len(query.TypeArguments.Nodes) > 0 {
		return nil
	}

	exprName := query.ExprName.AsNode()
	if exprName == nil || exprName.Kind != ast.KindQualifiedName {
		return nil
	}

	qualifiedName := exprName.AsQualifiedName()
	if qualifiedName == nil || qualifiedName.Right == nil || qualifiedName.Right.Text() != "Type" || qualifiedName.Left == nil {
		return nil
	}

	innerEntityName := qualifiedName.Left.AsNode()
	if innerEntityName == nil {
		return nil
	}

	queryType := tp.GetTypeAtLocation(node)
	if queryType == nil {
		return nil
	}

	innerSymbol, innerType := resolveEntityNameAsType(c, node, qualifiedName.Left)
	if innerType == nil {
		return nil
	}
	if isSelfReferentialTypeAliasReference(innerSymbol, node) {
		return nil
	}

	if !checker.Checker_isTypeAssignableTo(c, queryType, innerType) || !checker.Checker_isTypeAssignableTo(c, innerType, queryType) {
		return nil
	}

	return &UnnecessaryTypeofTypeMatch{
		SourceFile:      sf,
		Location:        scanner.GetErrorRangeForNode(sf, node),
		QueryNode:       node,
		InnerEntityName: innerEntityName,
		ReplacementText: scanner.GetTextOfNode(innerEntityName),
	}
}

func resolveEntityNameAsType(c *checker.Checker, location *ast.Node, entityName *ast.EntityName) (*ast.Symbol, *checker.Type) {
	if c == nil || location == nil || entityName == nil {
		return nil, nil
	}

	entityNode := entityName.AsNode()
	if entityNode == nil {
		return nil, nil
	}

	switch entityNode.Kind {
	case ast.KindIdentifier:
		symbol := c.ResolveName(entityNode.Text(), location, ast.SymbolFlagsType|ast.SymbolFlagsNamespace, false)
		symbol = resolveAliasSymbol(c, symbol)
		return symbol, resolvedSymbolType(c, symbol, location)
	case ast.KindQualifiedName:
		qualifiedName := entityNode.AsQualifiedName()
		if qualifiedName == nil || qualifiedName.Left == nil || qualifiedName.Right == nil {
			return nil, nil
		}

		leftSymbol, leftType := resolveEntityNameAsType(c, location, qualifiedName.Left)
		memberSymbol := resolveQualifiedTypeMember(c, location, leftSymbol, leftType, qualifiedName.Right.Text())
		memberSymbol = resolveAliasSymbol(c, memberSymbol)
		return memberSymbol, resolvedSymbolType(c, memberSymbol, location)
	default:
		return nil, nil
	}
}

func resolveQualifiedTypeMember(c *checker.Checker, location *ast.Node, leftSymbol *ast.Symbol, leftType *checker.Type, memberName string) *ast.Symbol {
	if c == nil || location == nil || memberName == "" {
		return nil
	}

	if leftSymbol != nil {
		if member := c.TryGetMemberInModuleExportsAndProperties(memberName, leftSymbol); member != nil {
			return member
		}

		if declaredType := c.GetDeclaredTypeOfSymbol(leftSymbol); declaredType != nil {
			if member := c.GetPropertyOfType(declaredType, memberName); member != nil {
				return member
			}
		}
	}

	if leftType != nil {
		if member := c.GetPropertyOfType(leftType, memberName); member != nil {
			return member
		}
	}

	return nil
}

func resolvedSymbolType(c *checker.Checker, symbol *ast.Symbol, location *ast.Node) *checker.Type {
	if c == nil || symbol == nil || location == nil {
		return nil
	}

	if symbol.Flags&ast.SymbolFlagsType != 0 {
		if declaredType := c.GetDeclaredTypeOfSymbol(symbol); declaredType != nil {
			return declaredType
		}
	}

	return c.GetTypeOfSymbolAtLocation(symbol, location)
}

func resolveAliasSymbol(c *checker.Checker, symbol *ast.Symbol) *ast.Symbol {
	for c != nil && symbol != nil && symbol.Flags&ast.SymbolFlagsAlias != 0 {
		symbol = c.GetAliasedSymbol(symbol)
	}
	return symbol
}

func isSelfReferentialTypeAliasReference(symbol *ast.Symbol, location *ast.Node) bool {
	if symbol == nil || location == nil {
		return false
	}

	for current := location.Parent; current != nil; current = current.Parent {
		if current.Kind != ast.KindTypeAliasDeclaration {
			continue
		}
		if slices.Contains(symbol.Declarations, current) {
			return true
		}
	}

	return false
}
