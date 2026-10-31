package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

var effectManagedRuntimeModuleDescriptor = newPackageSourceFileDescriptor(moduleDescriptorManagedRuntime,
	"effect",
	func(_ *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
		if c == nil || sf == nil {
			return false
		}
		module := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
		return module != nil &&
			c.TryGetMemberInModuleExportsAndProperties("ManagedRuntime", module) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("make", module) != nil &&
			c.TryGetMemberInModuleExportsAndProperties("isManagedRuntime", module) != nil
	},
)

// IsNodeReferenceToEffectManagedRuntimeMember reports whether node references
// the memberName property of Effect's ManagedRuntime interface, as in
// runtime.runPromise for a runtime built with ManagedRuntime.make.
func (tp *TypeParser) IsNodeReferenceToEffectManagedRuntimeMember(node *ast.Node, memberName string) bool {
	sym := tp.ReferenceSymbolAtNode(node)
	if sym == nil || ast.TSGetSymbolName(sym) != memberName {
		return false
	}
	for _, decl := range ast.TSGetSymbolDeclarations(sym) {
		if decl == nil || decl.Kind != ast.KindPropertySignature || decl.Parent == nil ||
			decl.Parent.Kind != ast.KindInterfaceDeclaration {
			continue
		}
		name := decl.Parent.Name()
		if name == nil || name.Kind != ast.KindIdentifier || name.Text() != "ManagedRuntime" {
			continue
		}
		if sf := ast.GetSourceFileOfNode(decl); sf != nil && tp.sourceFileMatchesModuleCached(sf, effectManagedRuntimeModuleDescriptor) {
			return true
		}
	}
	return false
}

// IsExpressionPlainEffectManagedRuntime reports whether the non-nullable type
// of node is a plain instantiation of Effect's ManagedRuntime interface, such
// as ManagedRuntime<R, ER>. Unions, intersections, mapped types like Pick, and
// interfaces extending ManagedRuntime do not count, so every ManagedRuntime
// member is known to be present with its declared type.
func (tp *TypeParser) IsExpressionPlainEffectManagedRuntime(node *ast.Node) bool {
	if tp == nil || tp.checker == nil || node == nil {
		return false
	}
	t := tp.GetTypeAtLocation(node)
	if t == nil {
		return false
	}
	t = tp.checker.GetNonNullableType(t)
	if t.Flags()&checker.TypeFlagsObject == 0 || t.ObjectFlags()&checker.ObjectFlagsReference == 0 || t.Target() == nil {
		return false
	}
	target := t.Target().Symbol()
	if target == nil {
		return false
	}
	for _, decl := range ast.TSGetSymbolDeclarations(target) {
		if decl == nil || decl.Kind != ast.KindInterfaceDeclaration {
			continue
		}
		name := decl.Name()
		if name == nil || name.Kind != ast.KindIdentifier || name.Text() != "ManagedRuntime" {
			continue
		}
		if sf := ast.GetSourceFileOfNode(decl); sf != nil && tp.sourceFileMatchesModuleCached(sf, effectManagedRuntimeModuleDescriptor) {
			return true
		}
	}
	return false
}
