package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

var serviceMapModuleDescriptor = PackageSourceFileDescriptor{
	PackageName:       "effect",
	MatchesSourceFile: isServiceMapTypeSourceFile,
}

// isServiceMapTypeSourceFile checks if a source file is the ServiceMap module
// by verifying it exports "ServiceMap".
func isServiceMapTypeSourceFile(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
	if c == nil || sf == nil {
		return false
	}

	moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
	if moduleSym == nil {
		return false
	}

	serviceMapSym := c.TryGetMemberInModuleExportsAndProperties("ServiceMap", moduleSym)
	return serviceMapSym != nil
}

// IsNodeReferenceToServiceMapModuleApi reports whether node resolves to a member
// exported by the "effect" package from a module that exports the ServiceMap type.
func (tp *TypeParser) IsNodeReferenceToServiceMapModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, serviceMapModuleDescriptor, memberName)
}
