package typeparser

import (
	"strings"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

type PackageSourceFileDescriptor struct {
	PackageName       string
	MatchesSourceFile func(*TypeParser, *checker.Checker, *ast.SourceFile) bool
	cacheKey          *packageSourceFileDescriptorCacheKey
}

type packageSourceFileDescriptorCacheKey byte

type moduleExportReferenceCacheKey struct {
	symbol     *ast.Symbol
	descriptor *packageSourceFileDescriptorCacheKey
	memberName string
}

func newPackageSourceFileDescriptor(
	packageName string,
	matchesSourceFile func(*TypeParser, *checker.Checker, *ast.SourceFile) bool,
) PackageSourceFileDescriptor {
	return PackageSourceFileDescriptor{
		PackageName:       packageName,
		MatchesSourceFile: matchesSourceFile,
		cacheKey:          new(packageSourceFileDescriptorCacheKey),
	}
}

func (tp *TypeParser) ReferenceSymbolAtNode(node *ast.Node) *ast.Symbol {
	if tp == nil || tp.checker == nil || node == nil {
		return nil
	}

	return Cached(&tp.links.ReferenceSymbol, node, func() *ast.Symbol {
		sym := tp.GetSymbolAtLocation(node)
		if sym == nil && node.Kind == ast.KindPropertyAccessExpression {
			if prop := node.AsPropertyAccessExpression(); prop != nil && prop.Name() != nil {
				sym = tp.GetSymbolAtLocation(prop.Name())
			}
		}

		sym = tp.resolveAliasedSymbol(sym)
		if node.Kind == ast.KindIdentifier && !ast.IsDeclarationName(node) {
			return tp.resolveConstantAliasSymbol(sym)
		}
		return sym
	})
}

func (tp *TypeParser) resolveConstantAliasSymbol(sym *ast.Symbol) *ast.Symbol {
	seen := make(map[*ast.Symbol]struct{})
	for sym != nil {
		if _, ok := seen[sym]; ok {
			return sym
		}
		seen[sym] = struct{}{}

		declarationNode := sym.ValueDeclaration
		if declarationNode == nil || declarationNode.Kind != ast.KindVariableDeclaration ||
			declarationNode.Parent == nil || declarationNode.Parent.Kind != ast.KindVariableDeclarationList ||
			declarationNode.Parent.Flags&ast.NodeFlagsConst == 0 {
			return sym
		}
		declaration := declarationNode.AsVariableDeclaration()
		if declaration == nil || declaration.Initializer == nil {
			return sym
		}

		next := tp.GetSymbolAtLocation(ast.SkipParentheses(declaration.Initializer))
		if next == nil {
			return sym
		}
		sym = tp.resolveAliasedSymbol(next)
	}
	return nil
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
	// Exported descriptors built as struct literals have no stable matcher identity.
	if desc.cacheKey == nil {
		return tp.isSymbolReferenceToModuleExport(sym, desc, memberName)
	}

	key := moduleExportReferenceCacheKey{
		symbol:     sym,
		descriptor: desc.cacheKey,
		memberName: memberName,
	}
	return Cached(&tp.links.ModuleExportReference, key, func() bool {
		return tp.isSymbolReferenceToModuleExport(sym, desc, memberName)
	})
}

func (tp *TypeParser) isSymbolReferenceToModuleExport(sym *ast.Symbol, desc PackageSourceFileDescriptor, memberName string) bool {
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
		exportSym = tp.resolveAliasedSymbol(exportSym)
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
