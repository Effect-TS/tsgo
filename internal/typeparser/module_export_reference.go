package typeparser

import (
	"slices"
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
	moduleDescriptorFunction
	moduleDescriptorMetric
	moduleDescriptorPlatformNode
	moduleDescriptorPlatformBun
	moduleDescriptorPlatformBrowser
	moduleDescriptorCount
)

type moduleDescriptorMask uint32

const maxModuleDescriptors = 32

type moduleExportFilter struct {
	checked        moduleDescriptorMask
	hasDeclaration moduleDescriptorMask
}

// moduleExportIdentity is one module file declaring a symbol, with the names
// that module exports the symbol under.
type moduleExportIdentity struct {
	file  *ast.SourceFile
	names []string
	// exportEquals marks an `export =` module. Members of its exported value
	// count as exports too, so it is answered by looking the member up.
	exportEquals bool
}

// moduleExportNames indexes a module's export names by resolved target.
type moduleExportNames struct {
	byTarget     map[*ast.Symbol][]string
	exportEquals bool
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
	if node.Flags&ast.NodeFlagsJSDoc == 0 && !canHaveSymbolAtLocation(node.Kind) {
		return nil
	}

	return CachedNode(&tp.links.ReferenceSymbol, node, func() *ast.Symbol {
		return tp.referenceSymbolFrom(node, tp.GetSymbolAtLocation(node))
	})
}

// canHaveSymbolAtLocation reports whether the checker's getSymbolAtLocation
// can return a symbol for a node of this kind: the source file, the
// declaration names it resolves before its switch, and the kinds its switch
// handles. Every other kind falls through to its nil default. JSDoc nodes can
// be swapped for their reparsed clones, so callers must not gate them.
func canHaveSymbolAtLocation(kind ast.Kind) bool {
	switch kind {
	case ast.KindSourceFile,
		ast.KindIdentifier, ast.KindPrivateIdentifier,
		ast.KindStringLiteral, ast.KindNumericLiteral, ast.KindBigIntLiteral, ast.KindNoSubstitutionTemplateLiteral,
		ast.KindComputedPropertyName,
		ast.KindPropertyAccessExpression, ast.KindElementAccessExpression, ast.KindQualifiedName,
		ast.KindThisKeyword, ast.KindThisType, ast.KindSuperKeyword,
		ast.KindConstructorKeyword, ast.KindDefaultKeyword, ast.KindFunctionKeyword,
		ast.KindEqualsGreaterThanToken, ast.KindClassKeyword,
		ast.KindImportType, ast.KindExportKeyword, ast.KindImportKeyword, ast.KindNewKeyword,
		ast.KindInstanceOfKeyword, ast.KindMetaProperty, ast.KindJsxNamespacedName:
		return true
	}
	return false
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

	for _, identity := range tp.moduleExportIdentities(sym) {
		if !tp.sourceFileMatchesModuleCached(identity.file, desc) {
			continue
		}
		if identity.exportEquals {
			if tp.isSymbolExportedFromModuleFile(sym, identity.file, memberName) {
				return true
			}
		} else if slices.Contains(identity.names, memberName) {
			return true
		}
	}
	return false
}

// moduleExportIdentities lists, once per symbol, the modules declaring sym
// and the names each exports it under. Only files inside a package can match a
// module descriptor, so other declarations are skipped.
func (tp *TypeParser) moduleExportIdentities(sym *ast.Symbol) []moduleExportIdentity {
	if identities := tp.links.ModuleExportIdentities.TryGet(sym); identities != nil {
		return *identities
	}

	var identities []moduleExportIdentity
	target := tp.resolvedTarget(sym)
	for _, decl := range ast.TSGetSymbolDeclarations(sym) {
		if decl == nil {
			continue
		}
		sf := ast.GetSourceFileOfNode(decl)
		if sf == nil || tp.PackageJsonForSourceFile(sf) == nil ||
			slices.ContainsFunc(identities, func(identity moduleExportIdentity) bool { return identity.file == sf }) {
			continue
		}
		exports := tp.moduleExportNamesOf(sf)
		if exports.exportEquals {
			identities = append(identities, moduleExportIdentity{file: sf, exportEquals: true})
		} else if names := exports.byTarget[target]; len(names) > 0 {
			identities = append(identities, moduleExportIdentity{file: sf, names: names})
		}
	}
	*tp.links.ModuleExportIdentities.Get(sym) = identities
	return identities
}

// moduleExportNamesOf groups a module's export names by the symbol each
// resolves to, comparing symbols the way getSymbolIfSameReference does.
func (tp *TypeParser) moduleExportNamesOf(sf *ast.SourceFile) moduleExportNames {
	return Cached(&tp.links.ModuleExportNames, sf, func() moduleExportNames {
		moduleSym := checker.Checker_getSymbolOfDeclaration(tp.checker, sf.AsNode())
		if moduleSym == nil {
			return moduleExportNames{}
		}
		if tp.checker.ResolveExternalModuleSymbol(moduleSym) != moduleSym {
			return moduleExportNames{exportEquals: true}
		}
		byTarget := make(map[*ast.Symbol][]string)
		tp.checker.ForEachExportAndPropertyOfModule(moduleSym, func(exportSym *ast.Symbol, name string) {
			if target := tp.resolvedTarget(tp.resolveAliasedSymbol(exportSym)); target != nil {
				byTarget[target] = append(byTarget[target], name)
			}
		})
		return moduleExportNames{byTarget: byTarget}
	})
}

// resolvedTarget mirrors the checker's getResolvedTarget, the key
// getSymbolIfSameReference compares.
func (tp *TypeParser) resolvedTarget(sym *ast.Symbol) *ast.Symbol {
	if sym == nil {
		return nil
	}
	return tp.checker.GetMergedSymbol(checker.Checker_resolveSymbol(tp.checker, tp.checker.GetMergedSymbol(sym)))
}

func (tp *TypeParser) sourceFileMatchesModuleCached(sf *ast.SourceFile, desc packageSourceFileDescriptor) bool {
	filter := tp.links.ModuleFileFilter.Get(sf)
	bit := moduleDescriptorMask(1) << desc.id
	if filter.checked&bit == 0 {
		filter.checked |= bit
		if tp.sourceFileMatchesModule(sf, desc) {
			filter.hasDeclaration |= bit
		}
	}
	return filter.hasDeclaration&bit != 0
}

func (tp *TypeParser) sourceFileMatchesModule(sf *ast.SourceFile, desc packageSourceFileDescriptor) bool {
	return tp.IsSourceFileInPackage(sf, desc.packageName) &&
		(desc.matchesSourceFile == nil || desc.matchesSourceFile(tp, tp.checker, sf))
}

func (tp *TypeParser) symbolHasDeclarationInModuleCached(sym *ast.Symbol, desc packageSourceFileDescriptor) bool {
	// With one declaration the per-file filter already answers in one lookup.
	if len(ast.TSGetSymbolDeclarations(sym)) == 1 {
		return tp.symbolHasDeclarationInModule(sym, desc)
	}
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
		if sf := ast.GetSourceFileOfNode(decl); sf != nil && tp.sourceFileMatchesModuleCached(sf, desc) {
			return true
		}
	}
	return false
}

// isSymbolExportedFromModuleFile looks memberName up in one module, including
// the members of an `export =` value.
func (tp *TypeParser) isSymbolExportedFromModuleFile(sym *ast.Symbol, sf *ast.SourceFile, memberName string) bool {
	moduleSym := checker.Checker_getSymbolOfDeclaration(tp.checker, sf.AsNode())
	if moduleSym == nil {
		return false
	}
	exportSym := tp.resolveAliasedSymbol(tp.checker.TryGetMemberInModuleExportsAndProperties(memberName, moduleSym))
	return checker.Checker_getSymbolIfSameReference(tp.checker, exportSym, sym) != nil
}

func (tp *TypeParser) IsNodeReferenceToModule(node *ast.Node, desc packageSourceFileDescriptor) bool {
	sym := tp.ReferenceSymbolAtNode(node)
	if sym == nil {
		return false
	}
	return tp.symbolHasDeclarationInModuleCached(sym, desc)
}
