package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

var effectDurationModuleDescriptor = newPackageSourceFileDescriptor(
	"effect",
	func(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
		if c == nil || sf == nil {
			return false
		}
		module := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
		return module != nil &&
			c.TryGetMemberInModuleExportsAndProperties("Duration", module) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("zero", module) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("millis", module) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("isDuration", module) != nil
	},
)

func (tp *TypeParser) IsNodeReferenceToEffectDurationModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectDurationModuleDescriptor, memberName)
}

func (tp *TypeParser) IsExpressionEffectDurationModule(node *ast.Node) bool {
	return tp.IsNodeReferenceToModule(node, effectDurationModuleDescriptor)
}
