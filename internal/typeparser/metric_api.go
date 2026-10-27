package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

var effectMetricModuleDescriptor = newPackageSourceFileDescriptor(moduleDescriptorMetric,
	"effect",
	func(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
		if c == nil || sf == nil {
			return false
		}
		module := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
		return module != nil &&
			c.TryGetMemberInModuleExportsAndProperties("Metric", module) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("update", module) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("counter", module) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("gauge", module) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("withConstantInput", module) != nil
	},
)

// IsNodeReferenceToEffectMetricModuleApi reports whether node resolves to a
// member exported by Effect's Metric module.
func (tp *TypeParser) IsNodeReferenceToEffectMetricModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectMetricModuleDescriptor, memberName)
}

// IsExpressionEffectMetricModule reports whether node references Effect's Metric module.
func (tp *TypeParser) IsExpressionEffectMetricModule(node *ast.Node) bool {
	return tp.IsNodeReferenceToModule(node, effectMetricModuleDescriptor)
}
