package typeparser

import (
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

type PackageSourceFileDescriptor struct {
	PackageName       string
	MatchesSourceFile func(*checker.Checker, *ast.SourceFile) bool
}

func ReferenceSymbolAtNode(c *checker.Checker, node *ast.Node) *ast.Symbol {
	if c == nil || node == nil {
		return nil
	}

	sym := c.GetSymbolAtLocation(node)
	if sym == nil && node.Kind == ast.KindPropertyAccessExpression {
		if prop := node.AsPropertyAccessExpression(); prop != nil && prop.Name() != nil {
			sym = c.GetSymbolAtLocation(prop.Name())
		}
	}

	return resolveAliasedSymbol(c, sym)
}

func IsSourceFileInPackage(c *checker.Checker, sf *ast.SourceFile, packageName string) bool {
	if c == nil || sf == nil {
		return false
	}
	pkg := PackageJsonForSourceFile(c, sf)
	if pkg == nil {
		return false
	}
	name, ok := pkg.Name.GetValue()
	return ok && strings.EqualFold(name, packageName)
}

func IsNodeReferenceToModuleExport(c *checker.Checker, node *ast.Node, desc PackageSourceFileDescriptor, memberName string) bool {
	sym := ReferenceSymbolAtNode(c, node)
	if sym == nil {
		return false
	}

	for _, decl := range sym.Declarations {
		if decl == nil {
			continue
		}
		sf := ast.GetSourceFileOfNode(decl)
		if sf == nil || !IsSourceFileInPackage(c, sf, desc.PackageName) {
			continue
		}
		if desc.MatchesSourceFile != nil && !desc.MatchesSourceFile(c, sf) {
			continue
		}
		moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
		if moduleSym == nil {
			continue
		}
		exportSym := c.TryGetMemberInModuleExportsAndProperties(memberName, moduleSym)
		exportSym = resolveAliasedSymbol(c, exportSym)
		if checker.Checker_getSymbolIfSameReference(c, exportSym, sym) != nil {
			return true
		}
	}

	return false
}

func IsNodeReferenceToModule(c *checker.Checker, node *ast.Node, desc PackageSourceFileDescriptor) bool {
	sym := ReferenceSymbolAtNode(c, node)
	if sym == nil {
		return false
	}

	for _, decl := range sym.Declarations {
		if decl == nil {
			continue
		}
		sf := ast.GetSourceFileOfNode(decl)
		if sf == nil || !IsSourceFileInPackage(c, sf, desc.PackageName) {
			continue
		}
		if desc.MatchesSourceFile == nil || desc.MatchesSourceFile(c, sf) {
			return true
		}
	}

	return false
}
