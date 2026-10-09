package checker

import (
	"github.com/microsoft/typescript-go/shim/ast"
	"unsafe"
)

// GetNonPrimitiveType returns the checker's canonical non-primitive object type.
func GetNonPrimitiveType(c *Checker) *Type { return Checker_nonPrimitiveType(c) }

// ConstraintType returns only the already-resolved constraint of a mapped type.
func GetMappedTypeConstraintType(t *MappedType) *Type { return MappedType_constraintType(t) }

// NameType returns only the already-resolved remapped key type of a mapped type.
func GetMappedTypeNameType(t *MappedType) *Type { return MappedType_nameType(t) }

// TemplateType returns only the already-resolved mapped property value type.
func GetMappedTypeTemplateType(t *MappedType) *Type { return MappedType_templateType(t) }

// GetResolvedConditionalTypeBranch returns an already-resolved true or false
// branch of a conditional type, or nil when the branch has not been resolved
// yet. Unlike getTrueTypeFromConditionalType it never instantiates or evaluates
// the conditional, so callers that only inspect represented components can read
// a resolved branch without risking recursive instantiation.
func GetResolvedConditionalTypeBranch(c *Checker, t *Type, trueBranch bool) *Type {
	if t == nil || t.Flags()&TypeFlagsConditional == 0 {
		return nil
	}
	d := t.AsConditionalType()
	if trueBranch {
		return ConditionalType_resolvedTrueType(d)
	}
	return ConditionalType_resolvedFalseType(d)
}

// GetConditionalTypeBranchNode returns the declared true or false branch node of
// a conditional type without reading or evaluating the branch type.
func GetConditionalTypeBranchNode(c *Checker, t *Type, trueBranch bool) *ast.Node {
	if t == nil || t.Flags()&TypeFlagsConditional == 0 {
		return nil
	}
	d := t.AsConditionalType()
	root := ConditionalType_root(d)
	if root == nil {
		return nil
	}
	node := ConditionalRoot_node(root)
	if node == nil {
		return nil
	}
	if trueBranch {
		return node.TrueType
	}
	return node.FalseType
}

// GetResolvedTypeFromTypeNode returns the type the checker has already
// associated with a type node, without resolving it. It peeks the cached node
// links, so it never instantiates or evaluates anything; an unresolved node
// returns nil.
func GetResolvedTypeFromTypeNode(c *Checker, node *ast.Node) *Type {
	if node == nil {
		return nil
	}
	store := Checker_typeNodeLinks(c)
	links := store.TryGet(node)
	if links == nil {
		return nil
	}
	return TypeNodeLinks_resolvedType(links)
}

// GetConditionalTypeBranchType returns the branch type of a conditional type
// only when the checker has already materialized the declared branch node. It
// is a cached-field peek, not a resolver: an unresolved branch returns nil, so
// callers that inspect represented components never force instantiation.
func GetConditionalTypeBranchType(c *Checker, t *Type, trueBranch bool) *Type {
	return GetResolvedTypeFromTypeNode(c, GetConditionalTypeBranchNode(c, t, trueBranch))
}

// GetResolvedTypeArguments returns the type arguments already recorded on a
// reference type without resolving or instantiating them. A deferred reference
// whose arguments have not been materialized returns nil.
func GetResolvedTypeArguments(c *Checker, t *Type) []*Type {
	if t == nil || t.Flags()&TypeFlagsObject == 0 || t.ObjectFlags()&ObjectFlagsReference == 0 {
		return nil
	}
	return TypeReference_resolvedTypeArguments(t.AsTypeReference())
}

// GetResolvedTypeOfSymbolIfMaterialized returns the type already computed for a
// value symbol, or nil when the checker has not materialized it. It never
// resolves the symbol's type, so callers can avoid forcing an inferred type
// whose instantiation is unbounded.
func GetResolvedTypeOfSymbolIfMaterialized(c *Checker, symbol *ast.Symbol) *Type {
	if symbol == nil {
		return nil
	}
	links := materializedValueSymbolLinks(c, symbol)
	if links == nil {
		return nil
	}
	return ValueSymbolLinks_resolvedType(links)
}

// GetResolvedDeclaredTypeOfSymbolIfMaterialized returns the declared type
// already recorded for a symbol, or nil when it has not been computed. Only
// already materialized results are returned, so resolving a type alias whose
// declared type would instantiate a recursive alias can never be forced.
func GetResolvedDeclaredTypeOfSymbolIfMaterialized(c *Checker, symbol *ast.Symbol) *Type {
	if symbol == nil {
		return nil
	}
	switch {
	case symbol.Flags&(ast.SymbolFlagsClass|ast.SymbolFlagsInterface|ast.SymbolFlagsAlias) != 0:
		store := Checker_declaredTypeLinks(c)
		if links := store.TryGet(symbol); links != nil {
			return DeclaredTypeLinks_declaredType(links)
		}
	case symbol.Flags&ast.SymbolFlagsTypeAlias != 0:
		store := Checker_typeAliasLinks(c)
		if links := store.TryGet(symbol); links != nil {
			return TypeAliasLinks_declaredType(links)
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
func GetDeclaredIndexInfosOfSymbol(c *Checker, symbol *ast.Symbol) []*IndexInfo {
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
			keyType := GetResolvedTypeFromTypeNode(c, keyNode)
			valueType := GetResolvedTypeFromTypeNode(c, declaration.Type())
			infos = append(infos, Checker_newIndexInfo(c, keyType, valueType, ast.HasModifier(declaration, ast.ModifierFlagsReadonly), declaration, nil))
		}
	}
	return infos
}

// GetResolvedReturnTypeOfSignatureIfMaterialized returns the return type the
// checker has already computed for a signature, or nil when it has not been
// materialized. It is a cached-field peek, not a resolver, so an annotated but
// unresolved return type is never forced.
func GetResolvedReturnTypeOfSignatureIfMaterialized(c *Checker, signature *Signature) *Type {
	if signature == nil {
		return nil
	}
	return Signature_resolvedReturnType(signature)
}

// GetResolvedConstraintOfTypeParameterIfMaterialized returns the constraint the
// checker has already computed for a type parameter, or nil when it has not
// been materialized or when the checker recorded that there is no constraint.
// A nil result means the declared constraint annotation is still unresolved, so
// callers can inspect the declaration without forcing it.
func GetResolvedConstraintOfTypeParameterIfMaterialized(c *Checker, t *Type) *Type {
	if t == nil || t.Flags()&TypeFlagsTypeParameter == 0 {
		return nil
	}
	constraint := TypeParameter_constraint(t.AsTypeParameter())
	if constraint == nil || constraint == Checker_noConstraintType(c) || constraint == Checker_circularConstraintType(c) {
		return nil
	}
	return constraint
}

// GetResolvedDefaultFromTypeParameterIfMaterialized returns the default type
// the checker has already computed for a type parameter, or nil when it has
// not been materialized or when the checker recorded that there is no default.
func GetResolvedDefaultFromTypeParameterIfMaterialized(c *Checker, t *Type) *Type {
	if t == nil || t.Flags()&TypeFlagsTypeParameter == 0 {
		return nil
	}
	defaultType := TypeParameter_resolvedDefaultType(t.AsTypeParameter())
	if defaultType == nil || defaultType == Checker_noConstraintType(c) || defaultType == Checker_circularConstraintType(c) || defaultType == Checker_resolvingDefaultType(c) {
		return nil
	}
	return defaultType
}

// GetResolvedBaseTypesOfTypeIfMaterialized returns the base types the checker
// has already resolved for a class or interface. The boolean reports whether
// inheritance has been materialized at all, so a cold declaration is never
// mistaken for one without heritage and callers can avoid forcing it.
func GetResolvedBaseTypesOfTypeIfMaterialized(c *Checker, t *Type) ([]*Type, bool) {
	if t == nil || t.ObjectFlags()&(ObjectFlagsClassOrInterface|ObjectFlagsTuple) == 0 {
		return nil, false
	}
	data := t.AsInterfaceType()
	if !InterfaceType_baseTypesResolved(data) {
		return nil, false
	}
	return InterfaceType_resolvedBaseTypes(data), true
}

// GetResolvedMembersOfTypeIfMaterialized returns the member table the checker
// has already resolved for a structured type. The boolean reports whether the
// member surface was materialized at all: a deferred reference keeps false so
// callers inspect the declaration instead of forcing member resolution, which
// would instantiate index annotations and inherited members.
func GetResolvedMembersOfTypeIfMaterialized(c *Checker, t *Type) (ast.SymbolTable, bool) {
	if t == nil || t.Flags()&TypeFlagsObject == 0 || t.ObjectFlags()&ObjectFlagsMembersResolved == 0 {
		return nil, false
	}
	return StructuredType_members(t.AsStructuredType()), true
}

// GetResolvedSignaturesOfTypeIfMaterialized returns the call or construct
// signatures the checker has already resolved for a structured type. The
// boolean reports whether the member surface was materialized at all, so a
// deferred type never forces signature resolution.
func GetResolvedSignaturesOfTypeIfMaterialized(c *Checker, t *Type, kind SignatureKind) ([]*Signature, bool) {
	if t == nil || t.Flags()&TypeFlagsObject == 0 || t.ObjectFlags()&ObjectFlagsMembersResolved == 0 {
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
func GetResolvedIndexInfosOfTypeIfMaterialized(c *Checker, t *Type) ([]*IndexInfo, bool) {
	if t == nil || t.Flags()&TypeFlagsObject == 0 || t.ObjectFlags()&ObjectFlagsMembersResolved == 0 {
		return nil, false
	}
	return StructuredType_indexInfos(t.AsStructuredType()), true
}

// GetInstantiatedSymbolMapper returns the mapper already recorded on an
// instantiated symbol, or nil. It is a cached-field peek: it never resolves or
// instantiates the symbol's type, so a caller can map an uninstantiated binder
// forward without evaluating a recursive alias member.
func GetInstantiatedSymbolMapper(c *Checker, symbol *ast.Symbol) *TypeMapper {
	if symbol == nil || symbol.CheckFlags&ast.CheckFlagsInstantiated == 0 {
		return nil
	}
	links := materializedValueSymbolLinks(c, symbol)
	if links == nil {
		return nil
	}
	return ValueSymbolLinks_mapper(links)
}

// IsSourceFileTypeChecked reports whether the checker has finished type
// checking a source file. It is a state peek that never checks the file: the
// stability analysis uses it to decide whether an inferred type may be resolved
// through the ordinary accessors, because forcing inference over a file that
// has not been checked could run body checks and report their diagnostics.
func IsSourceFileTypeChecked(c *Checker, sourceFile *ast.SourceFile) bool {
	if sourceFile == nil {
		return false
	}
	store := Checker_sourceFileLinks(c)
	links := store.TryGet(sourceFile)
	return links != nil && SourceFileLinks_typeChecked(links)
}

// IsCheckingSourceFile reports whether the checker is currently inside its
// ordinary source-file check lifecycle. It is a state peek that never checks a
// file. The stability analysis uses it to allow native lazy inference only
// while the checker is actively checking, which is the lifecycle in which that
// inference's diagnostics are attributed to their declaring expressions; a
// speculative read outside any check keeps refusing inferred components.
func IsCheckingSourceFile(c *Checker) bool {
	return Checker_ctx(c) != nil
}

// IsAliasResolutionFailed reports only an already-cached alias resolution
// failure. It never resolves the alias or emits diagnostics.
func IsAliasResolutionFailed(c *Checker, symbol *ast.Symbol) bool {
	if symbol == nil {
		return false
	}
	store := Checker_aliasSymbolLinks(c)
	links := store.TryGet(symbol)
	return links != nil && AliasSymbolLinks_aliasTarget(links) == Checker_unknownSymbol(c)
}

// materializedValueSymbolLinks reads the legacy provider's symbol store
// through the generated checker layout, without allocating links.
func materializedValueSymbolLinks(c *Checker, symbol *ast.Symbol) *ValueSymbolLinks {
	store := (*extra_Checker)(unsafe.Pointer(c)).valueSymbolLinks
	return store.TryGet(symbol)
}
