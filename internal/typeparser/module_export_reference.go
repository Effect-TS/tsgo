package typeparser

import (
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

type PackageSourceFileDescriptor struct {
	PackageName       string
	MatchesSourceFile func(*TypeParser, *checker.Checker, *ast.SourceFile) bool
}

func (tp *TypeParser) ReferenceSymbolAtNode(node *ast.Node) *ast.Symbol {
	if tp == nil || tp.checker == nil || node == nil {
		return nil
	}
	c := tp.checker

	sym := c.GetSymbolAtLocation(node)
	if sym == nil && node.Kind == ast.KindPropertyAccessExpression {
		if prop := node.AsPropertyAccessExpression(); prop != nil && prop.Name() != nil {
			sym = c.GetSymbolAtLocation(prop.Name())
		}
	}

	return resolveAliasedSymbol(c, sym)
}

func (tp *TypeParser) IsSourceFileInPackage(sf *ast.SourceFile, packageName string) bool {
	if tp == nil || tp.checker == nil || sf == nil {
		return false
	}
	pkg := tp.PackageJsonForSourceFile(sf)
	if pkg == nil {
		return false
	}
	name, ok := pkg.Name.GetValue()
	return ok && strings.EqualFold(name, packageName)
}

func (tp *TypeParser) IsNodeReferenceToModuleExport(node *ast.Node, desc PackageSourceFileDescriptor, memberName string) bool {
	sym := tp.ReferenceSymbolAtNode(node)
	if sym == nil {
		return false
	}

	for _, decl := range sym.Declarations {
		if decl == nil {
			continue
		}
		sf := ast.GetSourceFileOfNode(decl)
		if sf == nil || !tp.IsSourceFileInPackage(sf, desc.PackageName) {
			continue
		}
		if desc.MatchesSourceFile != nil && !desc.MatchesSourceFile(tp, tp.checker, sf) {
			continue
		}
		moduleSym := checker.Checker_getSymbolOfDeclaration(tp.checker, sf.AsNode())
		if moduleSym == nil {
			continue
		}
		exportSym := tp.checker.TryGetMemberInModuleExportsAndProperties(memberName, moduleSym)
		exportSym = resolveAliasedSymbol(tp.checker, exportSym)
		if checker.Checker_getSymbolIfSameReference(tp.checker, exportSym, sym) != nil {
			return true
		}
	}

	return false
}

func (tp *TypeParser) IsNodeReferenceToModule(node *ast.Node, desc PackageSourceFileDescriptor) bool {
	sym := tp.ReferenceSymbolAtNode(node)
	if sym == nil {
		return false
	}

	for _, decl := range sym.Declarations {
		if decl == nil {
			continue
		}
		sf := ast.GetSourceFileOfNode(decl)
		if sf == nil || !tp.IsSourceFileInPackage(sf, desc.PackageName) {
			continue
		}
		if desc.MatchesSourceFile == nil || desc.MatchesSourceFile(tp, tp.checker, sf) {
			return true
		}
	}

	return false
}
