package typeparser

import (
	"strings"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

type packageSourceFileDescriptor struct {
	id                moduleDescriptorID
	packageName       string
	matchesSourceFile func(*TypeParser, *checker.Checker, *ast.SourceFile) bool
}

type moduleDescriptorID uint8

const (
	moduleDescriptorEffect moduleDescriptorID = iota
	moduleDescriptorEffectPackage
	moduleDescriptorLayer
	moduleDescriptorSchema
	moduleDescriptorParseResult
	moduleDescriptorSchemaParser
	moduleDescriptorPredicate
	moduleDescriptorOption
	moduleDescriptorDuration
	moduleDescriptorEffectModel
	moduleDescriptorSqlModel
	moduleDescriptorContext
	moduleDescriptorCause
	moduleDescriptorData
	moduleDescriptorVitestRunner
	moduleDescriptorVitest
	moduleDescriptorEffectVitest
	moduleDescriptorCount
)

type moduleDescriptorMask uint32

const maxModuleDescriptors = 32

type moduleExportFilter struct {
	checked        moduleDescriptorMask
	hasDeclaration moduleDescriptorMask
}

type moduleExportReferenceCacheKey struct {
	descriptor moduleDescriptorID
	memberName string
}

func newPackageSourceFileDescriptor(
	id moduleDescriptorID,
	packageName string,
	matchesSourceFile func(*TypeParser, *checker.Checker, *ast.SourceFile) bool,
) packageSourceFileDescriptor {
	if id >= maxModuleDescriptors {
		panic("typeparser: package source file descriptor id does not fit moduleDescriptorMask")
	}
	return packageSourceFileDescriptor{
		id:                id,
		packageName:       packageName,
		matchesSourceFile: matchesSourceFile,
	}
}

func (tp *TypeParser) ReferenceSymbolAtNode(node *ast.Node) *ast.Symbol {
	if tp == nil || tp.checker == nil || node == nil {
		return nil
	}

	return CachedNode(&tp.links.ReferenceSymbol, node, func() *ast.Symbol {
		return tp.referenceSymbolFrom(node, tp.GetSymbolAtLocation(node))
	})
}

func (tp *TypeParser) SymbolAndReferenceSymbolAtNode(node *ast.Node) (*ast.Symbol, *ast.Symbol) {
	symbol := tp.GetSymbolAtLocation(node)
	if tp == nil || tp.checker == nil || node == nil {
		return symbol, nil
	}

	return symbol, CachedNode(&tp.links.ReferenceSymbol, node, func() *ast.Symbol {
		return tp.referenceSymbolFrom(node, symbol)
	})
}

func (tp *TypeParser) referenceSymbolFrom(node *ast.Node, sym *ast.Symbol) *ast.Symbol {
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
}

func (tp *TypeParser) resolveConstantAliasSymbol(sym *ast.Symbol) *ast.Symbol {
	seen := make(map[*ast.Symbol]struct{})
	for sym != nil {
		if _, ok := seen[sym]; ok {
			return sym
		}
		seen[sym] = struct{}{}

		declarationNode := ast.TSGetSymbolValueDeclaration(sym)
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

func (tp *TypeParser) IsNodeReferenceToModuleExport(node *ast.Node, desc packageSourceFileDescriptor, memberName string) bool {
	sym := tp.ReferenceSymbolAtNode(node)
	if sym == nil {
		return false
	}
	if !tp.symbolHasDeclarationInModuleCached(sym, desc) {
		return false
	}

	answers := tp.links.ModuleExportReference.Get(sym)
	key := moduleExportReferenceCacheKey{descriptor: desc.id, memberName: memberName}
	if answer, ok := (*answers)[key]; ok {
		return answer
	}
	answer := tp.isSymbolReferenceToModuleExport(sym, desc, memberName)
	if *answers == nil {
		*answers = make(map[moduleExportReferenceCacheKey]bool)
	}
	(*answers)[key] = answer
	return answer
}

func (tp *TypeParser) symbolHasDeclarationInModuleCached(sym *ast.Symbol, desc packageSourceFileDescriptor) bool {
	filter := tp.links.ModuleExportFilter.Get(sym)
	bit := moduleDescriptorMask(1) << desc.id
	if filter.checked&bit == 0 {
		filter.checked |= bit
		if tp.symbolHasDeclarationInModule(sym, desc) {
			filter.hasDeclaration |= bit
		}
	}
	return filter.hasDeclaration&bit != 0
}

func (tp *TypeParser) symbolHasDeclarationInModule(sym *ast.Symbol, desc packageSourceFileDescriptor) bool {
	for _, decl := range ast.TSGetSymbolDeclarations(sym) {
		if decl == nil {
			continue
		}
		sf := ast.GetSourceFileOfNode(decl)
		if sf == nil || !tp.IsSourceFileInPackage(sf, desc.packageName) {
			continue
		}
		if desc.matchesSourceFile == nil || desc.matchesSourceFile(tp, tp.checker, sf) {
			return true
		}
	}
	return false
}

func (tp *TypeParser) isSymbolReferenceToModuleExport(sym *ast.Symbol, desc packageSourceFileDescriptor, memberName string) bool {
	for _, decl := range ast.TSGetSymbolDeclarations(sym) {
		if decl == nil {
			continue
		}
		sf := ast.GetSourceFileOfNode(decl)
		if sf == nil || !tp.IsSourceFileInPackage(sf, desc.packageName) {
			continue
		}
		if desc.matchesSourceFile != nil && !desc.matchesSourceFile(tp, tp.checker, sf) {
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

func (tp *TypeParser) IsNodeReferenceToModule(node *ast.Node, desc packageSourceFileDescriptor) bool {
	sym := tp.ReferenceSymbolAtNode(node)
	if sym == nil {
		return false
	}
	return tp.symbolHasDeclarationInModuleCached(sym, desc)
}
