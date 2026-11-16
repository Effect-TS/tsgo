package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

const exitModuleDiscriminatingExport = "isExit"

var exitModuleRequiredExports = []string{"Exit", exitModuleDiscriminatingExport, "isFailure", "isSuccess", "failCause"}

var effectExitPackageSourceFileDescriptor = newPackageSourceFileDescriptor(moduleDescriptorExit,
	"effect",
	func(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
		if c == nil || sf == nil {
			return false
		}

		moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
		if moduleSym == nil {
			return false
		}

		for _, name := range exitModuleRequiredExports {
			if c.TryGetMemberInModuleExportsAndProperties(name, moduleSym) == nil {
				return false
			}
		}
		return true
	},
)

func (tp *TypeParser) IsNodeReferenceToEffectExitModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectExitPackageSourceFileDescriptor, memberName)
}
