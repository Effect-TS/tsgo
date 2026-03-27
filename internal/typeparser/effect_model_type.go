package typeparser

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

var effectModelPackageSourceFileDescriptor = PackageSourceFileDescriptor{
	PackageName:       "effect",
	MatchesSourceFile: isEffectModelTypeSourceFile,
}

// isEffectModelTypeSourceFile checks if a source file is the effect/unstable/schema Model module
// by verifying it exports "Class", "Generated", and "FieldOption".
// These symbols are chosen to disambiguate Model from Schema (which also exports "Class"),
// matching the TypeScript reference implementation.
func isEffectModelTypeSourceFile(c *checker.Checker, sf *ast.SourceFile) bool {
	if c == nil || sf == nil {
		return false
	}

	moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
	if moduleSym == nil {
		return false
	}

	if c.TryGetMemberInModuleExportsAndProperties("Class", moduleSym) == nil {
		return false
	}
	// Generated is unique to Model, not present in Schema
	if c.TryGetMemberInModuleExportsAndProperties("Generated", moduleSym) == nil {
		return false
	}
	// FieldOption is unique to v4 Model
	if c.TryGetMemberInModuleExportsAndProperties("FieldOption", moduleSym) == nil {
		return false
	}

	return true
}

// IsNodeReferenceToEffectModelModuleApi reports whether node resolves to a member
// exported by the "effect" package from a module that exports the Model API
// (effect/unstable/schema).
func IsNodeReferenceToEffectModelModuleApi(c *checker.Checker, node *ast.Node, memberName string) bool {
	return IsNodeReferenceToModuleExport(c, node, effectModelPackageSourceFileDescriptor, memberName)
}
