package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

var effectContextPackageSourceFileDescriptor = newPackageSourceFileDescriptor(moduleDescriptorContext,
	"effect",
	func(tp *TypeParser, c *checker.Checker, sf *ast.SourceFile) bool {
		if c == nil || sf == nil {
			return false
		}

		moduleSym := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
		if moduleSym == nil {
			return false
		}

		// The Context module exports "Context" (the namespace/type)
		contextSym := c.TryGetMemberInModuleExportsAndProperties("Context", moduleSym)
		if contextSym == nil {
			return false
		}

		// Effect v3 exports Context.Tag, while newer v4 betas export Context.Service.
		if tp.SupportedEffectVersion() == EffectMajorV4 {
			serviceSym := c.TryGetMemberInModuleExportsAndProperties("Service", moduleSym)
			return serviceSym != nil
		}
		tagSym := c.TryGetMemberInModuleExportsAndProperties("Tag", moduleSym)
		return tagSym != nil
	},
)

func (tp *TypeParser) IsNodeReferenceToEffectContextModuleApi(node *ast.Node, memberName string) bool {
	return tp.IsNodeReferenceToModuleExport(node, effectContextPackageSourceFileDescriptor, memberName)
}

// ContextTypeId is the property key for Context's variance struct (v4).
const ContextTypeId = "~effect/Context"

// ContextType returns t itself when it is an Effect v4 Context.Context type,
// or nil otherwise. It checks for the "~effect/Context" variance struct and its
// contravariant _Services marker, so any, unknown and never never match.
func (tp *TypeParser) ContextType(t *checker.Type) *checker.Type {
	if tp == nil || tp.checker == nil || t == nil {
		return nil
	}
	return Cached(&tp.links.ContextType, t, func() *checker.Type {
		if tp.DetectEffectVersion() != EffectMajorV4 {
			return nil
		}
		if t.Flags()&(checker.TypeFlagsAny|checker.TypeFlagsUnknown|checker.TypeFlagsNever) != 0 {
			return nil
		}
		variance := tp.GetTypeOfPropertyByName(t, ContextTypeId)
		if variance == nil || tp.extractContravariantType(variance, "_Services") == nil {
			return nil
		}
		return t
	})
}

// ContextTypeAtLocation parses the type of node as a v4 Context type, without
// computing the type when nodeCannotBeEffectLibraryType rules node out.
func (tp *TypeParser) ContextTypeAtLocation(node *ast.Node) *checker.Type {
	return tp.ContextType(tp.effectLibraryTypeAtLocation(node))
}
