package etslshooks

import (
	"context"
	"strings"

	"github.com/effect-ts/effect-typescript-go/internal/checkerutils"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/compiler"
	"github.com/microsoft/typescript-go/shim/core"
	"github.com/microsoft/typescript-go/shim/ls"
	"github.com/microsoft/typescript-go/shim/lsp/lsproto"
	"github.com/microsoft/typescript-go/shim/scanner"
)

func afterDocumentSymbols(ctx context.Context, sf *ast.SourceFile, symbols []*lsproto.DocumentSymbol, program *compiler.Program, langService *ls.LanguageService) []*lsproto.DocumentSymbol {
	if program.Options().Effect == nil {
		return symbols
	}

	c, done := program.GetTypeCheckerForFile(ctx, sf)
	defer done()

	layerChildren := collectLayerDocumentSymbols(c, sf, langService)
	serviceChildren := collectServiceDocumentSymbols(c, sf, langService)
	errorChildren := collectErrorDocumentSymbols(c, sf, langService)
	if len(layerChildren) == 0 && len(serviceChildren) == 0 && len(errorChildren) == 0 {
		return symbols
	}

	effectChildren := make([]*lsproto.DocumentSymbol, 0, 3)
	if len(layerChildren) > 0 {
		layers := newSyntheticNamespaceSymbol("Layers")
		layers.Children = &layerChildren
		effectChildren = append(effectChildren, layers)
	}
	if len(serviceChildren) > 0 {
		services := newSyntheticNamespaceSymbol("Services")
		services.Children = &serviceChildren
		effectChildren = append(effectChildren, services)
	}
	if len(errorChildren) > 0 {
		errors := newSyntheticNamespaceSymbol("Errors")
		errors.Children = &errorChildren
		effectChildren = append(effectChildren, errors)
	}
	effect := newSyntheticNamespaceSymbol("Effect")
	effect.Children = &effectChildren

	return append([]*lsproto.DocumentSymbol{effect}, symbols...)
}

func collectLayerDocumentSymbols(c *checker.Checker, sf *ast.SourceFile, langService *ls.LanguageService) []*lsproto.DocumentSymbol {
	var symbols []*lsproto.DocumentSymbol
	seen := map[*ast.Node]struct{}{}
	var walk ast.Visitor
	walk = func(current *ast.Node) bool {
		if current == nil {
			return false
		}
		t := checkerutils.GetTypeAtLocation(c, current)
		if typeparser.IsLayerType(c, t, current) {
			displayNode := resolveLayerDisplayNode(current)
			if _, ok := seen[displayNode]; !ok {
				seen[displayNode] = struct{}{}
				symbols = append(symbols, newEffectDocumentSymbol(c, sf, langService, current, displayNode, layerSymbolDetail))
			}
			return false
		}
		current.ForEachChild(walk)
		return false
	}
	sf.AsNode().ForEachChild(walk)
	return symbols
}

func collectServiceDocumentSymbols(c *checker.Checker, sf *ast.SourceFile, langService *ls.LanguageService) []*lsproto.DocumentSymbol {
	var symbols []*lsproto.DocumentSymbol
	seen := map[*ast.Node]struct{}{}
	var walk ast.Visitor
	walk = func(current *ast.Node) bool {
		if current == nil {
			return false
		}
		t := checkerutils.GetTypeAtLocation(c, current)
		if typeparser.IsServiceType(c, t, current) || typeparser.IsContextTag(c, t, current) {
			displayNode := resolveServiceDisplayNode(current)
			if _, ok := seen[displayNode]; !ok {
				seen[displayNode] = struct{}{}
				symbols = append(symbols, newEffectDocumentSymbol(c, sf, langService, current, displayNode, nil))
			}
			return false
		}
		if typeparser.IsLayerType(c, t, current) {
			return false
		}
		current.ForEachChild(walk)
		return false
	}
	sf.AsNode().ForEachChild(walk)
	return symbols
}

func collectErrorDocumentSymbols(c *checker.Checker, sf *ast.SourceFile, langService *ls.LanguageService) []*lsproto.DocumentSymbol {
	var symbols []*lsproto.DocumentSymbol
	seen := map[*ast.Node]struct{}{}
	var walk ast.Visitor
	walk = func(current *ast.Node) bool {
		if current == nil {
			return false
		}
		t := checkerutils.GetTypeAtLocation(c, current)
		if typeparser.IsYieldableErrorType(c, t) {
			displayNode := resolveErrorDisplayNode(current)
			if _, ok := seen[displayNode]; !ok {
				seen[displayNode] = struct{}{}
				symbols = append(symbols, newEffectDocumentSymbol(c, sf, langService, current, displayNode, nil))
			}
			return false
		}
		current.ForEachChild(walk)
		return false
	}
	sf.AsNode().ForEachChild(walk)
	return symbols
}

func newSyntheticNamespaceSymbol(name string) *lsproto.DocumentSymbol {
	children := []*lsproto.DocumentSymbol{}
	zero := lsproto.Position{}
	return &lsproto.DocumentSymbol{
		Name: name,
		Kind: lsproto.SymbolKindPackage,
		Range: lsproto.Range{
			Start: zero,
			End:   zero,
		},
		SelectionRange: lsproto.Range{
			Start: zero,
			End:   zero,
		},
		Children: &children,
	}
}

func newEffectDocumentSymbol(
	c *checker.Checker,
	sf *ast.SourceFile,
	langService *ls.LanguageService,
	node *ast.Node,
	displayNode *ast.Node,
	detail func(*checker.Checker, *ast.Node) *string,
) *lsproto.DocumentSymbol {
	converters := ls.LanguageService_converters(langService)
	startPos := scanner.SkipTrivia(sf.Text(), node.Pos())
	endPos := max(startPos, node.End())
	start := converters.PositionToLineAndCharacter(sf, core.TextPos(startPos))
	end := converters.PositionToLineAndCharacter(sf, core.TextPos(endPos))
	children := []*lsproto.DocumentSymbol{}
	var symbolDetail *string
	if detail != nil {
		symbolDetail = detail(c, node)
	}

	return &lsproto.DocumentSymbol{
		Name:   layerSymbolName(sf, displayNode),
		Detail: symbolDetail,
		Kind:   layerSymbolKind(displayNode),
		Range: lsproto.Range{
			Start: start,
			End:   end,
		},
		SelectionRange: lsproto.Range{
			Start: start,
			End:   end,
		},
		Children: &children,
	}
}

func layerSymbolDetail(c *checker.Checker, node *ast.Node) *string {
	t := checkerutils.GetTypeAtLocation(c, node)
	if t == nil {
		return nil
	}
	layer := typeparser.LayerType(c, t, node)
	if layer == nil {
		return nil
	}
	rOut := c.TypeToStringEx(layer.ROut, node, checker.TypeFormatFlagsNoTruncation)
	e := c.TypeToStringEx(layer.E, node, checker.TypeFormatFlagsNoTruncation)
	rIn := c.TypeToStringEx(layer.RIn, node, checker.TypeFormatFlagsNoTruncation)
	detail := "<" + rOut + ", " + e + ", " + rIn + ">"
	return &detail
}

func resolveLayerDisplayNode(node *ast.Node) *ast.Node {
	if node == nil || node.Parent == nil {
		return node
	}
	switch node.Parent.Kind {
	case ast.KindVariableDeclaration,
		ast.KindPropertyDeclaration,
		ast.KindPropertyAssignment,
		ast.KindShorthandPropertyAssignment,
		ast.KindPropertySignature,
		ast.KindBindingElement:
		return node.Parent
	default:
		return node
	}
}

func resolveServiceDisplayNode(node *ast.Node) *ast.Node {
	for current := node; current != nil; current = current.Parent {
		switch current.Kind {
		case ast.KindClassDeclaration, ast.KindClassExpression,
			ast.KindVariableDeclaration,
			ast.KindPropertyDeclaration,
			ast.KindPropertyAssignment,
			ast.KindShorthandPropertyAssignment,
			ast.KindPropertySignature,
			ast.KindBindingElement:
			return current
		}
	}
	return node
}

func resolveErrorDisplayNode(node *ast.Node) *ast.Node {
	for current := node; current != nil; current = current.Parent {
		switch current.Kind {
		case ast.KindClassDeclaration, ast.KindClassExpression,
			ast.KindVariableDeclaration,
			ast.KindPropertyDeclaration,
			ast.KindPropertyAssignment,
			ast.KindShorthandPropertyAssignment,
			ast.KindPropertySignature,
			ast.KindBindingElement:
			return current
		}
	}
	return node
}

func layerSymbolName(sf *ast.SourceFile, node *ast.Node) string {
	if node.Kind == ast.KindPropertyDeclaration {
		if classLike := node.Parent; classLike != nil && ast.IsClassLike(classLike) {
			className := strings.TrimSpace(scanner.GetTextOfNode(classLike.Name()))
			propertyName := strings.TrimSpace(scanner.GetTextOfNode(node.Name()))
			if className != "" && propertyName != "" {
				return className + "." + propertyName
			}
		}
	}
	if ast.IsDeclaration(node) {
		if name := ast.GetNameOfDeclaration(node); name != nil {
			text := strings.TrimSpace(scanner.GetTextOfNode(name))
			if text != "" {
				return text
			}
		}
	}
	text := strings.TrimSpace(scanner.GetSourceTextOfNodeFromSourceFile(sf, node, false))
	if text == "" {
		return "<layer>"
	}
	if len(text) > 80 {
		return text[:77] + "..."
	}
	return text
}

func layerSymbolKind(node *ast.Node) lsproto.SymbolKind {
	switch node.Kind {
	case ast.KindVariableDeclaration, ast.KindBindingElement:
		return lsproto.SymbolKindVariable
	case ast.KindPropertyDeclaration, ast.KindPropertyAssignment, ast.KindPropertySignature:
		return lsproto.SymbolKindProperty
	case ast.KindFunctionDeclaration, ast.KindFunctionExpression, ast.KindArrowFunction, ast.KindMethodDeclaration:
		return lsproto.SymbolKindFunction
	case ast.KindClassDeclaration, ast.KindClassExpression:
		return lsproto.SymbolKindClass
	default:
		return lsproto.SymbolKindVariable
	}
}
