package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

var effectAiToolModuleDescriptor = newPackageSourceFileDescriptor(moduleDescriptorAiTool,
	"effect",
	func(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
		if c == nil || sf == nil {
			return false
		}
		module := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
		return module != nil &&
			c.TryGetMemberInModuleExportsAndProperties("make", module) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("dynamic", module) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("providerDefined", module) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("EmptyParams", module) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("isUserDefined", module) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("getJsonSchema", module) != nil
	},
)

// IsNodeReferenceToEffectAiToolModuleApi reports whether node resolves to a
// member exported by Effect's AI Tool module.
func (tp *TypeParser) IsNodeReferenceToEffectAiToolModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectAiToolModuleDescriptor, memberName)
}

// IsExpressionEffectAiToolModule reports whether node references Effect's AI Tool module.
func (tp *TypeParser) IsExpressionEffectAiToolModule(node *ast.Node) bool {
	return tp.IsNodeReferenceToModule(node, effectAiToolModuleDescriptor)
}
