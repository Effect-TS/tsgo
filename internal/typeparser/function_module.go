package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

var effectFunctionPackageSourceFileDescriptor = newPackageSourceFileDescriptor(moduleDescriptorFunction,
	"effect",
	func(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
		if c == nil || sf == nil {
			return false
		}
		moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
		if moduleSym == nil {
			return false
		}

		return c.TryGetMemberInModuleExportsAndProperties("identity", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("dual", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("pipe", moduleSym) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("flow", moduleSym) != nil
	},
)

// IsNodeReferenceToEffectFunctionModuleApi reports whether node resolves to a
// member exported by Effect's Function module.
func (tp *TypeParser) IsNodeReferenceToEffectFunctionModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectFunctionPackageSourceFileDescriptor, memberName)
}

// IsNodeDirectReferenceToEffectFunctionModuleApi is like
// IsNodeReferenceToEffectFunctionModuleApi but does not follow local variable
// aliases: node must name the export itself, through imports and re-exports only.
func (tp *TypeParser) IsNodeDirectReferenceToEffectFunctionModuleApi(node *ast.Node, memberName string) bool {
	sym := tp.resolveAliasedSymbol(tp.GetSymbolAtLocation(node))
	return sym != nil && tp.symbolHasDeclarationInModuleCached(sym, effectFunctionPackageSourceFileDescriptor) &&
		tp.IsNodeReferenceToEffectFunctionModuleApi(node, memberName)
}
