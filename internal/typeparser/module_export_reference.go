package typeparser

import (
	"strings"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

type PackageSourceFileDescriptor struct {
	PackageName       string
	MatchesSourceFile func(*TypeParser, *checker.Checker, *ast.SourceFile) bool
	id                moduleDescriptorID
}

// moduleDescriptorID names each package source file descriptor so module export
// answers can be cached per symbol in a bitmask. The zero value marks
// descriptors built as struct literals, which have no stable identity and are
// not cached.
type moduleDescriptorID uint8

const (
	moduleDescriptorUncached moduleDescriptorID = iota
	moduleDescriptorEffect
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

// A symbol's module export filter keeps one "checked" bit per descriptor in
// the low half and one "has a declaration in that module" bit in the high half.
const moduleExportFilterCandidateShift = 32

// Fails to compile once the descriptors no longer fit in one half of the filter.
var _ = [moduleExportFilterCandidateShift - int(moduleDescriptorCount)]struct{}{}

type moduleExportReferenceCacheKey struct {
	descriptor moduleDescriptorID
	memberName string
}

func newPackageSourceFileDescriptor(
	id moduleDescriptorID,
	packageName string,
	matchesSourceFile func(*TypeParser, *checker.Checker, *ast.SourceFile) bool,
) PackageSourceFileDescriptor {
	if id == moduleDescriptorUncached || int(id) > moduleExportFilterCandidateShift {
		panic("typeparser: package source file descriptor id outside the module export filter")
	}
	return PackageSourceFileDescriptor{
		PackageName:       packageName,
		MatchesSourceFile: matchesSourceFile,
		id:                id,
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

// ReferenceSymbolAtNodeFrom is ReferenceSymbolAtNode for a caller that has just
// computed GetSymbolAtLocation(node), so a cache miss does not look it up again.
func (tp *TypeParser) ReferenceSymbolAtNodeFrom(node *ast.Node, symbolAtLocation *ast.Symbol) *ast.Symbol {
	if tp == nil || tp.checker == nil || node == nil {
		return nil
	}

	return CachedNode(&tp.links.ReferenceSymbol, node, func() *ast.Symbol {
		return tp.referenceSymbolFrom(node, symbolAtLocation)
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

func (tp *TypeParser) IsNodeReferenceToModuleExport(node *ast.Node, desc PackageSourceFileDescriptor, memberName string) bool {
	sym := tp.ReferenceSymbolAtNode(node)
	if sym == nil {
		return false
	}
	if desc.id == moduleDescriptorUncached {
		return tp.isSymbolReferenceToModuleExport(sym, desc, memberName)
	}
	// Only a symbol with a declaration in the descriptor's module can be one of
	// its exports, whatever the member name.
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

func (tp *TypeParser) symbolHasDeclarationInModuleCached(sym *ast.Symbol, desc PackageSourceFileDescriptor) bool {
	filter := tp.links.ModuleExportFilter.Get(sym)
	checked := uint64(1) << (desc.id - 1)
	candidate := checked << moduleExportFilterCandidateShift
	if *filter&checked == 0 {
		*filter |= checked
		if tp.symbolHasDeclarationInModule(sym, desc) {
			*filter |= candidate
		}
	}
	return *filter&candidate != 0
}

func (tp *TypeParser) symbolHasDeclarationInModule(sym *ast.Symbol, desc PackageSourceFileDescriptor) bool {
	for _, decl := range ast.TSGetSymbolDeclarations(sym) {
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

func (tp *TypeParser) isSymbolReferenceToModuleExport(sym *ast.Symbol, desc PackageSourceFileDescriptor, memberName string) bool {
	for _, decl := range ast.TSGetSymbolDeclarations(sym) {
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
	if desc.id == moduleDescriptorUncached {
		return tp.symbolHasDeclarationInModule(sym, desc)
	}
	return tp.symbolHasDeclarationInModuleCached(sym, desc)
}
