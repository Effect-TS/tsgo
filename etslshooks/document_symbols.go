package etslshooks

import (
	"context"
	"slices"
	"strings"

	"github.com/effect-ts/tsgo/internal/typeparser"
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
	tp := typeparser.NewTypeParser(program, c)

	layerChildren := collectLayerDocumentSymbols(tp, c, sf, langService)
	serviceChildren := collectServiceDocumentSymbols(tp, c, sf, langService)
	errorChildren := collectErrorDocumentSymbols(tp, c, sf, langService)
	schemaChildren := collectSchemaDocumentSymbols(tp, c, sf, langService)
	if len(layerChildren) == 0 && len(serviceChildren) == 0 && len(errorChildren) == 0 && len(schemaChildren) == 0 {
		return symbols
	}

	effectChildren := make([]*lsproto.DocumentSymbol, 0, 4)
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
	if len(schemaChildren) > 0 {
		schemas := newSyntheticNamespaceSymbol("Schemas")
		schemas.Children = &schemaChildren
		effectChildren = append(effectChildren, schemas)
	}
	effect := newSyntheticNamespaceSymbol("Effect")
	effect.Children = &effectChildren

	return append([]*lsproto.DocumentSymbol{effect}, symbols...)
}

func collectLayerDocumentSymbols(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, langService *ls.LanguageService) []*lsproto.DocumentSymbol {
	var symbols []*lsproto.DocumentSymbol
	seen := map[*ast.Node]struct{}{}
	var walk ast.Visitor
	walk = func(current *ast.Node) bool {
		if current == nil {
			return false
		}
		if isEffectSymbolDeclaration(current) {
			if isLayerDeclaration(tp, c, current) {
				displayNode := resolveLayerDisplayNode(current)
				if _, ok := seen[displayNode]; !ok {
					seen[displayNode] = struct{}{}
					symbols = append(symbols, newEffectDocumentSymbol(tp, c, sf, langService, current, displayNode, layerSymbolDetail))
				}
				return false
			}
		}
		current.ForEachChild(walk)
		return false
	}
	sf.AsNode().ForEachChild(walk)
	return symbols
}

func collectServiceDocumentSymbols(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, langService *ls.LanguageService) []*lsproto.DocumentSymbol {
	var symbols []*lsproto.DocumentSymbol
	seen := map[*ast.Node]struct{}{}
	var walk ast.Visitor
	walk = func(current *ast.Node) bool {
		if current == nil {
			return false
		}
		if isEffectSymbolDeclaration(current) {
			if isServiceDeclaration(tp, c, current) {
				displayNode := resolveServiceDisplayNode(current)
				if _, ok := seen[displayNode]; !ok {
					seen[displayNode] = struct{}{}
					symbols = append(symbols, newEffectDocumentSymbol(tp, c, sf, langService, current, displayNode, nil))
				}
				return false
			}
			t := tp.GetTypeAtLocation(current)
			if tp.IsLayerType(t, current) {
				return false
			}
		}
		current.ForEachChild(walk)
		return false
	}
	sf.AsNode().ForEachChild(walk)
	return symbols
}

func collectErrorDocumentSymbols(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, langService *ls.LanguageService) []*lsproto.DocumentSymbol {
	var symbols []*lsproto.DocumentSymbol
	seen := map[*ast.Node]struct{}{}
	var walk ast.Visitor
	walk = func(current *ast.Node) bool {
		if current == nil {
			return false
		}
		if isEffectSymbolDeclaration(current) {
			if isErrorDeclaration(tp, c, current) {
				displayNode := resolveErrorDisplayNode(current)
				if _, ok := seen[displayNode]; !ok {
					seen[displayNode] = struct{}{}
					symbols = append(symbols, newEffectDocumentSymbol(tp, c, sf, langService, current, displayNode, nil))
				}
				return false
			}
		}
		current.ForEachChild(walk)
		return false
	}
	sf.AsNode().ForEachChild(walk)
	return symbols
}

func collectSchemaDocumentSymbols(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, langService *ls.LanguageService) []*lsproto.DocumentSymbol {
	var symbols []*lsproto.DocumentSymbol
	seen := map[*ast.Node]struct{}{}
	var walk ast.Visitor
	walk = func(current *ast.Node) bool {
		if current == nil {
			return false
		}
		if isEffectSymbolDeclaration(current) {
			if isSchemaDeclaration(tp, c, current) {
				displayNode := resolveSchemaDisplayNode(current)
				if _, ok := seen[displayNode]; !ok {
					seen[displayNode] = struct{}{}
					symbols = append(symbols, newEffectDocumentSymbol(tp, c, sf, langService, current, displayNode, nil))
				}
				return false
			}
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
	tp *typeparser.TypeParser,
	c *checker.Checker,
	sf *ast.SourceFile,
	langService *ls.LanguageService,
	node *ast.Node,
	displayNode *ast.Node,
	detail func(*typeparser.TypeParser, *checker.Checker, *ast.Node) *string,
) *lsproto.DocumentSymbol {
	converters := ls.LanguageService_converters(langService)
	startPos := scanner.SkipTrivia(sf.Text(), node.Pos())
	endPos := max(startPos, node.End())
	start := converters.PositionToLineAndCharacter(sf, core.TextPos(startPos))
	end := converters.PositionToLineAndCharacter(sf, core.TextPos(endPos))
	children := []*lsproto.DocumentSymbol{}
	var symbolDetail *string
	if detail != nil {
		symbolDetail = detail(tp, c, node)
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

func layerSymbolDetail(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node) *string {
	typeCheckNode, types := classificationTypes(tp, c, node)
	for _, t := range types {
		layer := tp.LayerType(t, typeCheckNode)
		if layer == nil {
			continue
		}
		rOut := c.TypeToStringEx(layer.ROut, typeCheckNode, checker.TypeFormatFlagsNoTruncation)
		e := c.TypeToStringEx(layer.E, typeCheckNode, checker.TypeFormatFlagsNoTruncation)
		rIn := c.TypeToStringEx(layer.RIn, typeCheckNode, checker.TypeFormatFlagsNoTruncation)
		detail := "<" + rOut + ", " + e + ", " + rIn + ">"
		return &detail
	}
	return nil
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
		case ast.KindClassDeclaration,
			ast.KindVariableDeclaration,
			ast.KindPropertyDeclaration:
			return current
		}
	}
	return node
}

func classificationTypes(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node) (*ast.Node, []*checker.Type) {
	if node == nil {
		return nil, nil
	}
	t := tp.GetTypeAtLocation(node)
	if t == nil {
		return node, nil
	}
	types := []*checker.Type{t}
	if node.Kind == ast.KindClassDeclaration {
		if className := node.Name(); className != nil {
			if classSymbol := c.GetSymbolAtLocation(className); classSymbol != nil {
				if classType := c.GetTypeOfSymbolAtLocation(classSymbol, node); classType != nil && classType != t {
					types = append(types, classType)
				}
			}
		}
	}
	if constructSignatures := c.GetConstructSignatures(t); len(constructSignatures) > 0 {
		if returnType := c.GetReturnTypeOfSignature(constructSignatures[0]); returnType != nil {
			types = append(types, returnType)
		}
	}
	if callSignatures := c.GetSignaturesOfType(t, checker.SignatureKindCall); len(callSignatures) > 0 {
		if returnType := c.GetReturnTypeOfSignature(callSignatures[0]); returnType != nil {
			types = append(types, returnType)
		}
	}
	return node, types
}

func isLayerDeclaration(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node) bool {
	typeCheckNode, types := classificationTypes(tp, c, node)
	for _, t := range types {
		if tp.IsLayerType(t, typeCheckNode) {
			return true
		}
	}
	return false
}

func isServiceDeclaration(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node) bool {
	if node == nil {
		return false
	}
	typeCheckNode, types := classificationTypes(tp, c, node)
	for _, t := range types {
		if tp.IsServiceType(t, typeCheckNode) || tp.IsContextTag(t, typeCheckNode) {
			return true
		}
	}
	return false
}

func isErrorDeclaration(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node) bool {
	if node == nil {
		return false
	}
	switch node.Kind {
	case ast.KindClassDeclaration, ast.KindVariableDeclaration:
	default:
		return false
	}
	_, types := classificationTypes(tp, c, node)
	return slices.ContainsFunc(types, tp.IsYieldableErrorType)
}

func isSchemaDeclaration(tp *typeparser.TypeParser, c *checker.Checker, node *ast.Node) bool {
	if node == nil {
		return false
	}
	switch node.Kind {
	case ast.KindClassDeclaration, ast.KindVariableDeclaration, ast.KindPropertyDeclaration:
	default:
		return false
	}
	typeCheckNode, types := classificationTypes(tp, c, node)
	for _, t := range types {
		if tp.IsSchemaType(t, typeCheckNode) {
			return true
		}
	}
	return false
}

func resolveErrorDisplayNode(node *ast.Node) *ast.Node {
	for current := node; current != nil; current = current.Parent {
		switch current.Kind {
		case ast.KindClassDeclaration,
			ast.KindVariableDeclaration,
			ast.KindPropertyDeclaration:
			return current
		}
	}
	return node
}

func resolveSchemaDisplayNode(node *ast.Node) *ast.Node {
	for current := node; current != nil; current = current.Parent {
		switch current.Kind {
		case ast.KindClassDeclaration,
			ast.KindVariableDeclaration,
			ast.KindPropertyDeclaration:
			return current
		}
	}
	return node
}

func isEffectSymbolDeclaration(node *ast.Node) bool {
	if node == nil {
		return false
	}
	switch node.Kind {
	case ast.KindClassDeclaration, ast.KindVariableDeclaration, ast.KindPropertyDeclaration:
	default:
		return false
	}
	for current := node.Parent; current != nil; current = current.Parent {
		if current.Kind == ast.KindObjectLiteralExpression ||
			current.Kind == ast.KindForOfStatement ||
			current.Kind == ast.KindForInStatement {
			return false
		}
	}
	return true
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
