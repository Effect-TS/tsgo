// Effect-owned integration source for the TypeScript checker.
//
// `repoctl submodules setup` copies this file into tsc/internal/checker/ of the
// TypeScript checkout; see _integrations/README.md. It carries additive
// accessors the Effect stability analysis needs without growing the upstream
// exports patch stack. Never edit the generated shim package in its place.

package checker

import "github.com/microsoft/TypeScript/tsc/internal/ast"

// GetResolvedConditionalTypeBranch returns an already-resolved true or false
// branch of a conditional type, or nil when the branch has not been resolved
// yet. Unlike getTrueTypeFromConditionalType it never instantiates or evaluates
// the conditional, so callers that only inspect represented components can read
// a resolved branch without risking recursive instantiation.
func (c *Checker) GetResolvedConditionalTypeBranch(t *Type, trueBranch bool) *Type {
	if t == nil || t.flags&TypeFlagsConditional == 0 {
		return nil
	}
	d := t.AsConditionalType()
	if trueBranch {
		return d.resolvedTrueType
	}
	return d.resolvedFalseType
}

// GetConditionalTypeBranchNode returns the declared true or false branch node of
// a conditional type without reading or evaluating the branch type.
func (c *Checker) GetConditionalTypeBranchNode(t *Type, trueBranch bool) *ast.Node {
	if t == nil || t.flags&TypeFlagsConditional == 0 {
		return nil
	}
	d := t.AsConditionalType()
	if d.root == nil || d.root.node == nil {
		return nil
	}
	if trueBranch {
		return d.root.node.TrueType
	}
	return d.root.node.FalseType
}

// GetResolvedTypeFromTypeNode returns the type the checker has already
// associated with a type node, without resolving it. It peeks the cached node
// links, so it never instantiates or evaluates anything; an unresolved node
// returns nil.
func (c *Checker) GetResolvedTypeFromTypeNode(node *ast.Node) *Type {
	if node == nil {
		return nil
	}
	links := c.typeNodeLinks.TryGet(node)
	if links == nil {
		return nil
	}
	return links.resolvedType
}

// GetConditionalTypeBranchType returns the branch type of a conditional type
// only when the checker has already materialized the declared branch node. It
// is a cached-field peek, not a resolver: an unresolved branch returns nil, so
// callers that inspect represented components never force instantiation.
func (c *Checker) GetConditionalTypeBranchType(t *Type, trueBranch bool) *Type {
	return c.GetResolvedTypeFromTypeNode(c.GetConditionalTypeBranchNode(t, trueBranch))
}

// GetResolvedTypeArguments returns the type arguments already recorded on a
// reference type without resolving or instantiating them. A deferred reference
// whose arguments have not been materialized returns nil.
func (c *Checker) GetResolvedTypeArguments(t *Type) []*Type {
	if t == nil || t.flags&TypeFlagsObject == 0 || t.objectFlags&ObjectFlagsReference == 0 {
		return nil
	}
	return t.AsTypeReference().resolvedTypeArguments
}

// GetResolvedTypeOfSymbolIfMaterialized returns the type already computed for a
// value symbol, or nil when the checker has not materialized it. It never
// resolves the symbol's type, so callers can avoid forcing an inferred type
// whose instantiation is unbounded.
func (c *Checker) GetResolvedTypeOfSymbolIfMaterialized(symbol *ast.Symbol) *Type {
	if symbol == nil {
		return nil
	}
	links := c.valueSymbolLinks.TryGet(symbol)
	if links == nil {
		return nil
	}
	return links.resolvedType
}

// GetResolvedDeclaredTypeOfSymbolIfMaterialized returns the declared type
// already recorded for a symbol, or nil when it has not been computed. Only
// already materialized results are returned, so resolving a type alias whose
// declared type would instantiate a recursive alias can never be forced.
func (c *Checker) GetResolvedDeclaredTypeOfSymbolIfMaterialized(symbol *ast.Symbol) *Type {
	if symbol == nil {
		return nil
	}
	switch {
	case symbol.Flags&(ast.SymbolFlagsClass|ast.SymbolFlagsInterface|ast.SymbolFlagsAlias) != 0:
		if links := c.declaredTypeLinks.TryGet(symbol); links != nil {
			return links.declaredType
		}
	case symbol.Flags&ast.SymbolFlagsTypeAlias != 0:
		if links := c.typeAliasLinks.TryGet(symbol); links != nil {
			return links.declaredType
		}
	}
	return nil
}

// GetDeclaredIndexInfosOfSymbol returns the index infos declared by a symbol
// without resolving its member table or its key and value annotations. Unlike
// getIndexSymbol it walks the symbol's own declarations directly, so a
// late-bound computed property name is never evaluated just to inspect index
// signatures. A key or value type is returned only when the checker has
// already materialized the annotation; an unresolved annotation stays nil so
// callers can inspect the declaration instead of forcing a potentially
// unbounded instantiation.
func (c *Checker) GetDeclaredIndexInfosOfSymbol(symbol *ast.Symbol) []*IndexInfo {
	if symbol == nil {
		return nil
	}
	var infos []*IndexInfo
	for _, symbolDeclaration := range symbol.Declarations {
		if symbolDeclaration == nil {
			continue
		}
		var declarations []*ast.Node
		switch symbolDeclaration.Kind {
		case ast.KindClassDeclaration, ast.KindClassExpression, ast.KindInterfaceDeclaration,
			ast.KindEnumDeclaration, ast.KindTypeLiteral, ast.KindMappedType:
			declarations = symbolDeclaration.Members()
		}
		if ast.IsIndexSignatureDeclaration(symbolDeclaration) {
			declarations = append(declarations, symbolDeclaration)
		}
		for _, declaration := range declarations {
			if declaration == nil || !ast.IsIndexSignatureDeclaration(declaration) {
				continue
			}
			if ast.HasStaticModifier(declaration) {
				// Instance members only, matching getIndexSymbol.
				continue
			}
			parameters := declaration.Parameters()
			if len(parameters) != 1 {
				continue
			}
			keyNode := parameters[0].Type()
			if keyNode == nil {
				continue
			}
			keyType := c.GetResolvedTypeFromTypeNode(keyNode)
			valueType := c.GetResolvedTypeFromTypeNode(declaration.Type())
			infos = append(infos, c.newIndexInfo(keyType, valueType, ast.HasModifier(declaration, ast.ModifierFlagsReadonly), declaration, nil))
		}
	}
	return infos
}

// GetResolvedReturnTypeOfSignatureIfMaterialized returns the return type the
// checker has already computed for a signature, or nil when it has not been
// materialized. It is a cached-field peek, not a resolver, so an annotated but
// unresolved return type is never forced.
func (c *Checker) GetResolvedReturnTypeOfSignatureIfMaterialized(signature *Signature) *Type {
	if signature == nil {
		return nil
	}
	return signature.resolvedReturnType
}

// GetResolvedConstraintOfTypeParameterIfMaterialized returns the constraint the
// checker has already computed for a type parameter, or nil when it has not
// been materialized or when the checker recorded that there is no constraint.
// A nil result means the declared constraint annotation is still unresolved, so
// callers can inspect the declaration without forcing it.
func (c *Checker) GetResolvedConstraintOfTypeParameterIfMaterialized(t *Type) *Type {
	if t == nil || t.flags&TypeFlagsTypeParameter == 0 {
		return nil
	}
	constraint := t.AsTypeParameter().constraint
	if constraint == nil || constraint == c.noConstraintType || constraint == c.circularConstraintType {
		return nil
	}
	return constraint
}

// GetResolvedDefaultFromTypeParameterIfMaterialized returns the default type
// the checker has already computed for a type parameter, or nil when it has
// not been materialized or when the checker recorded that there is no default.
func (c *Checker) GetResolvedDefaultFromTypeParameterIfMaterialized(t *Type) *Type {
	if t == nil || t.flags&TypeFlagsTypeParameter == 0 {
		return nil
	}
	defaultType := t.AsTypeParameter().resolvedDefaultType
	if defaultType == nil || defaultType == c.noConstraintType || defaultType == c.circularConstraintType || defaultType == c.resolvingDefaultType {
		return nil
	}
	return defaultType
}

// GetResolvedBaseTypesOfTypeIfMaterialized returns the base types the checker
// has already resolved for a class or interface. The boolean reports whether
// inheritance has been materialized at all, so a cold declaration is never
// mistaken for one without heritage and callers can avoid forcing it.
func (c *Checker) GetResolvedBaseTypesOfTypeIfMaterialized(t *Type) ([]*Type, bool) {
	if t == nil || t.objectFlags&(ObjectFlagsClassOrInterface|ObjectFlagsTuple) == 0 {
		return nil, false
	}
	data := t.AsInterfaceType()
	if !data.baseTypesResolved {
		return nil, false
	}
	return data.resolvedBaseTypes, true
}

// GetResolvedMembersOfTypeIfMaterialized returns the member table the checker
// has already resolved for a structured type. The boolean reports whether the
// member surface was materialized at all: a deferred reference keeps false so
// callers inspect the declaration instead of forcing member resolution, which
// would instantiate index annotations and inherited members.
func (c *Checker) GetResolvedMembersOfTypeIfMaterialized(t *Type) (ast.SymbolTable, bool) {
	if t == nil || t.flags&TypeFlagsObject == 0 || t.objectFlags&ObjectFlagsMembersResolved == 0 {
		return nil, false
	}
	return t.AsStructuredType().members, true
}

// GetResolvedSignaturesOfTypeIfMaterialized returns the call or construct
// signatures the checker has already resolved for a structured type. The
// boolean reports whether the member surface was materialized at all, so a
// deferred type never forces signature resolution.
func (c *Checker) GetResolvedSignaturesOfTypeIfMaterialized(t *Type, kind SignatureKind) ([]*Signature, bool) {
	if t == nil || t.flags&TypeFlagsObject == 0 || t.objectFlags&ObjectFlagsMembersResolved == 0 {
		return nil, false
	}
	data := t.AsStructuredType()
	if kind == SignatureKindCall {
		return data.CallSignatures(), true
	}
	return data.ConstructSignatures(), true
}

// GetResolvedIndexInfosOfTypeIfMaterialized returns the index infos the
// checker has already computed for a structured type. The boolean reports
// whether the member surface was materialized at all, so a deferred type never
// forces an index annotation through this accessor.
func (c *Checker) GetResolvedIndexInfosOfTypeIfMaterialized(t *Type) ([]*IndexInfo, bool) {
	if t == nil || t.flags&TypeFlagsObject == 0 || t.objectFlags&ObjectFlagsMembersResolved == 0 {
		return nil, false
	}
	return t.AsStructuredType().indexInfos, true
}

// GetInstantiatedSymbolMapper returns the mapper already recorded on an
// instantiated symbol, or nil. It is a cached-field peek: it never resolves or
// instantiates the symbol's type, so a caller can map an uninstantiated binder
// forward without evaluating a recursive alias member.
func (c *Checker) GetInstantiatedSymbolMapper(symbol *ast.Symbol) *TypeMapper {
	if symbol == nil || symbol.CheckFlags&ast.CheckFlagsInstantiated == 0 {
		return nil
	}
	links := c.valueSymbolLinks.TryGet(symbol)
	if links == nil {
		return nil
	}
	return links.mapper
}

// IsSourceFileTypeChecked reports whether the checker has finished type
// checking a source file. It is a state peek that never checks the file: the
// stability analysis uses it to decide whether an inferred type may be resolved
// through the ordinary accessors, because forcing inference over a file that
// has not been checked could run body checks and report their diagnostics.
func (c *Checker) IsSourceFileTypeChecked(sourceFile *ast.SourceFile) bool {
	if sourceFile == nil {
		return false
	}
	links := c.sourceFileLinks.TryGet(sourceFile)
	return links != nil && links.typeChecked
}

// IsCheckingSourceFile reports whether the checker is currently inside its
// ordinary source-file check lifecycle. It is a state peek that never checks a
// file. The stability analysis uses it to allow native lazy inference only
// while the checker is actively checking, which is the lifecycle in which that
// inference's diagnostics are attributed to their declaring expressions; a
// speculative read outside any check keeps refusing inferred components.
func (c *Checker) IsCheckingSourceFile() bool {
	return c.ctx != nil
}
