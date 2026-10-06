package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// apiStabilitySafety is the verdict of the pre-flight recursion guard that runs
// before a potentially recursive lazy checker read. The guard inspects the
// represented compiler type graph and the raw declaration structure without
// performing the guarded resolution itself. Safe authorizes the ordinary lazy
// read, Recursive and Unknown never do.
//
// The guard is a pure safety analysis: it decides only whether a read may
// expand a recursive alias without a lazy memo boundary. It never contributes
// stability findings and it never suppresses compiler diagnostics.
type apiStabilitySafety int

const (
	apiStabilitySafetySafe apiStabilitySafety = iota
	apiStabilitySafetyRecursive
	apiStabilitySafetyUnknown
)

const (
	// apiStabilitySafetyMaxWork bounds one guard run. Exhausting it yields
	// Unknown, which blocks the read; it is never memoized as safe.
	apiStabilitySafetyMaxWork = 16_384
	// apiStabilitySafetyMaxDepth bounds nested alias applications on one guard
	// path. Exceeding it yields Unknown.
	apiStabilitySafetyMaxDepth = 64
)

// apiStabilitySafetyOperation identifies the guarded read family a completed
// verdict belongs to, so a Safe result is only reused for the same operation.
type apiStabilitySafetyOperation uint8

const (
	apiStabilitySafetyOperationMemberTable apiStabilitySafetyOperation = iota
	apiStabilitySafetyOperationBaseTypes
	apiStabilitySafetyOperationSymbolType
	apiStabilitySafetyOperationSignatureMembers
	apiStabilitySafetyOperationSignatureReturn
	apiStabilitySafetyOperationDeclaredType
	apiStabilitySafetyOperationAnnotation
)

// apiStabilitySafetyVerdictKey is the concrete identity of one guard request:
// the read operation, the represented compiler object or raw node it resolves,
// and the interned carrier of the substitution it is read under. The carrier
// is the identity of the whole substitution chain, so two requests that would
// resolve different represented components never share a verdict. Relation
// operand and projection scans remain internal to a request, so they cannot
// collide with a completed top-level verdict. No serialized context, string or
// raw declaration target is part of a key.
type apiStabilitySafetyVerdictKey struct {
	operation apiStabilitySafetyOperation
	t         *checker.Type
	signature *checker.Signature
	symbol    *ast.Symbol
	node      *ast.Node
	carrier   *apiStabilityCarrier
}

// apiStabilitySafetyArg is one concrete argument of a represented generic
// application. Either the checker already represented the argument type, or the
// raw argument node is kept together with the environment it was written in so
// a type parameter argument can still be followed through an outer binding.
type apiStabilitySafetyArg struct {
	t        *checker.Type
	node     *ast.Node
	subst    apiStabilitySubstitution
	bindings *apiStabilitySafetyBinding
}

func (arg apiStabilitySafetyArg) empty() bool {
	return arg.t == nil && arg.node == nil
}

func safetyArgsEqual(left, right []apiStabilitySafetyArg) bool {
	if len(left) != len(right) {
		return false
	}
	for index := range left {
		if left[index].t != nil || right[index].t != nil {
			if left[index].t != right[index].t {
				return false
			}
			continue
		}
		if left[index].node != right[index].node {
			return false
		}
	}
	return true
}

// apiStabilitySafetyBinding is one generic-application frame: the declared type
// parameters of a class, interface or alias bound to the arguments of one
// application. Frames chain so a nested application keeps the outer bindings.
type apiStabilitySafetyBinding struct {
	parent *apiStabilitySafetyBinding
	params []*checker.Type
	args   []apiStabilitySafetyArg
}

// apiStabilitySafetyApplication is one alias application on the current guard
// path. The concrete argument list is compared by identity so a repeated
// application of the same alias with the same represented arguments is
// recognized as an evaluated cycle.
type apiStabilitySafetyApplication struct {
	symbol *ast.Symbol
	args   []apiStabilitySafetyArg
}

// apiStabilitySafetyScan is the mutable state of one guard run. The verdict is
// computed by traversing raw annotations, represented types and declaration
// structure; a work and depth bound makes the traversal total and fails closed.
// All nested scans share one analysis-level work counter so a chain of nested
// argument verifications cannot run away.
type apiStabilitySafetyScan struct {
	a *apiStabilityAnalysis

	stack []apiStabilitySafetyApplication

	// relationOperands marks a nested scan that verifies a conditional operand
	// before the compiler relation is asked to compare it. The relation compares
	// the full projected member surface of both operands (properties, call and
	// construct returns, index infos) and instantiates generic members, so the
	// operand scan evaluates return annotations and refuses any component the
	// relation could instantiate (an unbound type parameter, a deferral, an
	// inferred return).
	relationOperands bool

	activeDeclarations map[*ast.Symbol]bool
	materializing      map[*checker.Type]bool
	materializingNodes map[*ast.Node]bool
}

func (s *apiStabilitySafetyScan) exceeded() bool {
	s.a.safetyWork++
	return s.a.safetyWork > apiStabilitySafetyMaxWork
}

// combineSafety merges two verdicts of one read: any recursion makes the whole
// read recursive, and any unknown makes it unknown.
func combineSafety(left, right apiStabilitySafety) apiStabilitySafety {
	if left == apiStabilitySafetyRecursive || right == apiStabilitySafetyRecursive {
		return apiStabilitySafetyRecursive
	}
	if left == apiStabilitySafetyUnknown || right == apiStabilitySafetyUnknown {
		return apiStabilitySafetyUnknown
	}
	return apiStabilitySafetySafe
}

// symbolTypeResolutionIsSafe reports whether resolving an unmaterialized
// symbol's type through the ordinary lazy accessor is bounded. An annotation or
// a function-like declaration is read through the recursion guard; an inferred
// declaration (an initializer, a parameter default or an accessor body) is read
// through the checker's ordinary lazy inference. Inference is the compiler's own
// lazy read: it attributes any diagnostics it produces to the declaring file's
// own expressions, so a later check of that file observes exactly the same
// diagnostics and types.
func (a *apiStabilityAnalysis) symbolTypeResolutionIsSafe(symbol *ast.Symbol) bool {
	if symbol == nil || len(symbol.Declarations) == 0 {
		return false
	}
	subst := apiStabilitySubstitution{}
	if mapper := checker.GetInstantiatedSymbolMapper(a.tp.checker, symbol); mapper != nil {
		subst = a.extendMapperSubstitution(apiStabilitySubstitution{}, nil, mapper)
	}
	key := apiStabilitySafetyVerdictKey{operation: apiStabilitySafetyOperationSymbolType, symbol: symbol, carrier: subst.carrier()}
	if a.safetySafeVerdict(key) {
		return true
	}
	if a.safetyBudgetExhausted() {
		return false
	}
	s := a.newSafetyScan()
	for _, declaration := range symbol.Declarations {
		if declaration == nil {
			continue
		}
		if functionLike := declaration.FunctionLikeData(); functionLike != nil {
			// Resolving a function-like value creates its signature: parameter
			// types and type parameter constraints are resolved eagerly, the
			// return type stays lazy and is gated by its own read. An accessor
			// that must infer its type from its body checks the body exactly
			// like function and initializer inference does: its diagnostics
			// belong to the declaring file's own expressions, and it is only
			// read through the native source-check lifecycle.
			if declaration.Kind == ast.KindGetAccessor || declaration.Kind == ast.KindSetAccessor {
				if !a.inferredComponentResolutionIsSafe(declaration) {
					return false
				}
			}
			if verdict := s.functionLikeResolution(declaration, subst, nil); verdict != apiStabilitySafetySafe {
				return false
			}
			continue
		}
		annotation := declarationAnnotationNodeOf(declaration)
		if annotation == nil {
			// An inferred declaration is read through the ordinary lazy
			// accessor. The checker attributes the diagnostics inference
			// produces to the declaring file's expressions, so the checked
			// program keeps them exactly. Inference is only read while the
			// checker is inside its source-file check lifecycle; outside it a
			// speculative inference is refused.
			if !a.inferredComponentResolutionIsSafe(declaration) {
				return false
			}
			continue
		}
		if verdict := s.typeNode(annotation, subst, nil, false); verdict != apiStabilitySafetySafe {
			return false
		}
	}
	a.recordSafetySafeVerdict(key)
	return true
}

// signatureMemberResolutionIsSafe reports whether materializing the raw
// signatures of a signature member symbol is bounded. It only inspects
// parameter annotations, which signature creation resolves eagerly.
func (a *apiStabilityAnalysis) signatureMemberResolutionIsSafe(member *ast.Symbol, subst apiStabilitySubstitution) bool {
	if member == nil {
		return false
	}
	key := apiStabilitySafetyVerdictKey{operation: apiStabilitySafetyOperationSignatureMembers, symbol: member, carrier: subst.carrier()}
	if a.safetySafeVerdict(key) {
		return true
	}
	if a.safetyBudgetExhausted() {
		return false
	}
	s := a.newSafetyScan()
	for _, declaration := range member.Declarations {
		if declaration == nil || declaration.FunctionLikeData() == nil {
			continue
		}
		if verdict := s.functionLikeResolution(declaration, subst, nil); verdict != apiStabilitySafetySafe {
			return false
		}
	}
	a.recordSafetySafeVerdict(key)
	return true
}

// annotationResolutionIsSafe reports whether resolving a raw annotation node
// through the ordinary lazy type accessor is bounded. The substitution context
// is the concrete enclosing application whose mapper will be applied when the
// annotation is instantiated.
func (a *apiStabilityAnalysis) annotationResolutionIsSafe(node *ast.Node, subst apiStabilitySubstitution) bool {
	if node == nil {
		return false
	}
	key := apiStabilitySafetyVerdictKey{operation: apiStabilitySafetyOperationAnnotation, node: node, carrier: subst.carrier()}
	if a.safetySafeVerdict(key) {
		return true
	}
	if a.safetyBudgetExhausted() {
		return false
	}
	s := a.newSafetyScan()
	if s.typeNode(node, subst, nil, false) != apiStabilitySafetySafe {
		return false
	}
	a.recordSafetySafeVerdict(key)
	return true
}

// declaredTypeResolutionIsSafe reports whether resolving a symbol's declared
// type through the ordinary lazy accessor is bounded. Only a type alias
// actually resolves its right-hand side; a class or interface declared type is
// created without resolving members.
func (a *apiStabilityAnalysis) declaredTypeResolutionIsSafe(symbol *ast.Symbol) bool {
	if symbol == nil {
		return false
	}
	if symbol.Flags&ast.SymbolFlagsAlias != 0 {
		if hasAliasDeclaration(symbol) {
			target := ApiStabilityImmediateAliasedSymbol(a.tp.checker, symbol)
			if target != nil && target != symbol {
				return a.declaredTypeResolutionIsSafe(target)
			}
		}
		return true
	}
	if symbol.Flags&ast.SymbolFlagsTypeAlias == 0 {
		return true
	}
	key := apiStabilitySafetyVerdictKey{operation: apiStabilitySafetyOperationDeclaredType, symbol: symbol}
	if a.safetySafeVerdict(key) {
		return true
	}
	if a.safetyBudgetExhausted() {
		return false
	}
	declaration := apiStabilityTypeAliasDeclaration(symbol)
	if declaration == nil {
		return false
	}
	s := a.newSafetyScan()
	if s.typeNode(declaration.Type(), apiStabilitySubstitution{}, nil, false) != apiStabilitySafetySafe {
		return false
	}
	a.recordSafetySafeVerdict(key)
	return true
}

// signatureReturnResolutionIsSafe reports whether materializing an
// unmaterialized signature return through the ordinary lazy accessor is
// bounded. An inferred return has no annotation: it is read through the
// checker's ordinary lazy inference, which attributes the diagnostics it
// produces to the declaring file's own expressions, so a later check of that
// file observes exactly the same diagnostics and types. An accessor's inferred
// return checks the accessor body exactly like an inferred component, so it is
// read through the same native source-check lifecycle.
func (a *apiStabilityAnalysis) signatureReturnResolutionIsSafe(signature *checker.Signature, subst apiStabilitySubstitution) bool {
	if signature == nil {
		return false
	}
	node := signatureReturnTypeNode(signature)
	if node == nil {
		declaration := signature.Declaration()
		if declaration == nil {
			return false
		}
		return a.inferredComponentResolutionIsSafe(declaration)
	}
	key := apiStabilitySafetyVerdictKey{operation: apiStabilitySafetyOperationSignatureReturn, signature: signature, carrier: subst.carrier()}
	if a.safetySafeVerdict(key) {
		return true
	}
	if a.safetyBudgetExhausted() {
		return false
	}
	s := a.newSafetyScan()
	if s.typeNode(node, subst, nil, false) != apiStabilitySafetySafe {
		return false
	}
	a.recordSafetySafeVerdict(key)
	return true
}

// inferredComponentResolutionIsSafe reports whether the native lazy inference
// of an unmaterialized component declared in a source file may be read. The
// ordinary lazy accessors attribute the diagnostics inference produces to the
// declaring file's own expressions, exactly as that file's ordinary check
// would, but that attribution is only established while the checker is inside
// its source-file check lifecycle (the after-check callback). A speculative
// read outside that lifecycle keeps refusing inferred components.
func (a *apiStabilityAnalysis) inferredComponentResolutionIsSafe(declaration *ast.Node) bool {
	if declaration == nil {
		return false
	}
	if checker.IsSourceFileTypeChecked(a.tp.checker, ast.GetSourceFileOfNode(declaration)) {
		return true
	}
	return checker.IsCheckingSourceFile(a.tp.checker)
}

// baseTypesResolutionIsSafe reports whether materializing an unmaterialized
// heritage surface is bounded. Resolving base types evaluates the heritage type
// argument annotations; the base declarations' own member surfaces are gated by
// their own reads.
func (a *apiStabilityAnalysis) baseTypesResolutionIsSafe(declaration *checker.Type, subst apiStabilitySubstitution) bool {
	if declaration == nil {
		return false
	}
	symbol := declaration.Symbol()
	if symbol == nil {
		return false
	}
	key := apiStabilitySafetyVerdictKey{operation: apiStabilitySafetyOperationBaseTypes, t: declaration, carrier: subst.carrier()}
	if a.safetySafeVerdict(key) {
		return true
	}
	if a.safetyBudgetExhausted() {
		return false
	}
	s := a.newSafetyScan()
	if s.heritage(symbol, subst, nil, false) != apiStabilitySafetySafe {
		return false
	}
	a.recordSafetySafeVerdict(key)
	return true
}

// memberTableResolutionIsSafe reports whether resolving a structured type's
// member table (properties, signatures or index infos) through the ordinary
// lazy accessors is bounded. Member resolution eagerly instantiates declared
// index signature annotations, creates the declared call and construct
// signatures, and recursively resolves inherited base member tables.
func (a *apiStabilityAnalysis) memberTableResolutionIsSafe(t *checker.Type, subst apiStabilitySubstitution) bool {
	if t == nil {
		return false
	}
	if t.Flags()&checker.TypeFlagsObject == 0 {
		return true
	}
	key := apiStabilitySafetyVerdictKey{operation: apiStabilitySafetyOperationMemberTable, t: t, carrier: subst.carrier()}
	if a.safetySafeVerdict(key) {
		return true
	}
	if a.safetyBudgetExhausted() {
		return false
	}
	s := a.newSafetyScan()
	if s.memberTable(t, subst, nil) != apiStabilitySafetySafe {
		return false
	}
	a.recordSafetySafeVerdict(key)
	return true
}

// safetyBudgetExhausted reports whether this analysis has already consumed its
// shared guard budget. A guard request after exhaustion is refused immediately
// without allocating scan state; the traversal still collects represented
// dependencies and declaration metadata.
func (a *apiStabilityAnalysis) safetyBudgetExhausted() bool {
	return a.safetyWork > apiStabilitySafetyMaxWork
}

// safetySafeVerdict reports whether an identical guard request already
// completed safely in this analysis. Only completed Safe verdicts are reused:
// Unknown, recursion and exhaustion stay retryable by later scans and later
// analyses.
func (a *apiStabilityAnalysis) safetySafeVerdict(key apiStabilitySafetyVerdictKey) bool {
	if a.safetySafeMemo == nil {
		return false
	}
	_, ok := a.safetySafeMemo[key]
	return ok
}

// recordSafetySafeVerdict records one completed Safe guard verdict.
func (a *apiStabilityAnalysis) recordSafetySafeVerdict(key apiStabilitySafetyVerdictKey) {
	if a.safetySafeMemo == nil {
		a.safetySafeMemo = make(map[apiStabilitySafetyVerdictKey]struct{})
	}
	a.safetySafeMemo[key] = struct{}{}
}

// symbolAtTypeNameNode returns the symbol named by a raw type-name, heritage,
// qualifier or declaration-name node, caching successful lookups by AST
// identity for this analysis. A failed lookup is never cached. Only the symbol
// lookup is cached: no resolved type, binding-dependent member identity or
// context string is ever stored here.
func (a *apiStabilityAnalysis) symbolAtTypeNameNode(node *ast.Node) *ast.Symbol {
	if node == nil {
		return nil
	}
	if symbol, ok := a.typeNameSymbols[node]; ok {
		return symbol
	}
	symbol := a.tp.checker.GetSymbolAtLocation(node)
	if symbol != nil {
		if a.typeNameSymbols == nil {
			a.typeNameSymbols = make(map[*ast.Node]*ast.Symbol)
		}
		a.typeNameSymbols[node] = symbol
	}
	return symbol
}

func (a *apiStabilityAnalysis) newSafetyScan() *apiStabilitySafetyScan {
	return &apiStabilitySafetyScan{a: a}
}

// typeNode scans one raw type annotation node. project marks a position whose
// members are evaluated because the surrounding construct projects into them
// (an indexed access or a keyof target), so nested property annotations are
// evaluated as well.
func (s *apiStabilitySafetyScan) typeNode(node *ast.Node, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	if node == nil {
		return apiStabilitySafetySafe
	}
	if s.exceeded() {
		return apiStabilitySafetyUnknown
	}
	switch node.Kind {
	case ast.KindAnyKeyword, ast.KindUnknownKeyword, ast.KindNeverKeyword, ast.KindVoidKeyword,
		ast.KindUndefinedKeyword, ast.KindNullKeyword, ast.KindStringKeyword, ast.KindNumberKeyword,
		ast.KindBigIntKeyword, ast.KindBooleanKeyword, ast.KindSymbolKeyword, ast.KindObjectKeyword,
		ast.KindThisType, ast.KindIntrinsicKeyword, ast.KindLiteralType:
		return apiStabilitySafetySafe
	case ast.KindParenthesizedType:
		return s.typeNode(node.AsParenthesizedTypeNode().Type, subst, bindings, project)
	case ast.KindArrayType:
		return s.typeNode(node.AsArrayTypeNode().ElementType, subst, bindings, project)
	case ast.KindOptionalType:
		return s.typeNode(node.AsOptionalTypeNode().Type, subst, bindings, project)
	case ast.KindRestType:
		return s.typeNode(node.AsRestTypeNode().Type, subst, bindings, project)
	case ast.KindTypeOperator:
		operator := node.AsTypeOperatorNode()
		if operator.Operator == ast.KindKeyOfKeyword {
			// `keyof T` resolves the keys of T's member surface.
			return s.typeNode(operator.Type, subst, bindings, true)
		}
		return s.typeNode(operator.Type, subst, bindings, project)
	case ast.KindNamedTupleMember:
		return s.typeNode(node.AsNamedTupleMember().Type, subst, bindings, project)
	case ast.KindTupleType:
		verdict := apiStabilitySafetySafe
		if elements := node.AsTupleTypeNode().Elements; elements != nil {
			for _, element := range elements.Nodes {
				verdict = combineSafety(verdict, s.typeNode(element, subst, bindings, project))
			}
		}
		return verdict
	case ast.KindUnionType:
		verdict := apiStabilitySafetySafe
		if list := node.AsUnionTypeNode().Types; list != nil {
			for _, member := range list.Nodes {
				verdict = combineSafety(verdict, s.typeNode(member, subst, bindings, project))
			}
		}
		return verdict
	case ast.KindIntersectionType:
		verdict := apiStabilitySafetySafe
		if list := node.AsIntersectionTypeNode().Types; list != nil {
			for _, member := range list.Nodes {
				verdict = combineSafety(verdict, s.typeNode(member, subst, bindings, project))
			}
		}
		return verdict
	case ast.KindTemplateLiteralType:
		verdict := apiStabilitySafetySafe
		if template := node.AsTemplateLiteralTypeNode(); template != nil && template.TemplateSpans != nil {
			for _, span := range template.TemplateSpans.Nodes {
				if span == nil || span.Kind != ast.KindTemplateLiteralTypeSpan {
					continue
				}
				verdict = combineSafety(verdict, s.typeNode(span.AsTemplateLiteralTypeSpan().Type, subst, bindings, project))
			}
		}
		return verdict
	case ast.KindFunctionType, ast.KindConstructorType:
		return s.functionLikeResolution(node, subst, bindings)
	case ast.KindTypeLiteral:
		return s.typeLiteral(node, subst, bindings, project)
	case ast.KindMappedType:
		return s.mappedTypeNode(node, subst, bindings, project)
	case ast.KindConditionalType:
		return s.conditionalNode(node, subst, bindings, project)
	case ast.KindIndexedAccessType:
		indexed := node.AsIndexedAccessTypeNode()
		verdict := s.typeNode(indexed.ObjectType, subst, bindings, true)
		return combineSafety(verdict, s.typeNode(indexed.IndexType, subst, bindings, project))
	case ast.KindTypeReference:
		return s.typeReference(node, subst, bindings, project)
	case ast.KindExpressionWithTypeArguments:
		expression := node.AsExpressionWithTypeArguments()
		verdict := s.typeArguments(expression.TypeArguments, subst, bindings)
		if !project {
			return verdict
		}
		symbol := s.a.symbolAtTypeNameNode(expression.Expression)
		return combineSafety(verdict, s.declarationMembers(symbol, expression.TypeArguments, subst, bindings, true))
	case ast.KindTypeQuery:
		return s.typeQuery(node, subst, bindings, project)
	case ast.KindImportType:
		importType := node.AsImportTypeNode()
		verdict := s.typeArguments(importType.TypeArguments, subst, bindings)
		if importType.Qualifier == nil {
			return combineSafety(verdict, apiStabilitySafetyUnknown)
		}
		symbol := s.a.symbolAtTypeNameNode(importType.Qualifier)
		if symbol == nil {
			return combineSafety(verdict, apiStabilitySafetyUnknown)
		}
		if !project {
			return verdict
		}
		return combineSafety(verdict, s.declarationMembers(symbol, importType.TypeArguments, subst, bindings, true))
	case ast.KindInferType:
		return apiStabilitySafetySafe
	default:
		return apiStabilitySafetyUnknown
	}
}

// typeArguments scans the eagerly evaluated type arguments of a reference.
func (s *apiStabilitySafetyScan) typeArguments(arguments *ast.NodeList, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding) apiStabilitySafety {
	verdict := apiStabilitySafetySafe
	if arguments == nil {
		return verdict
	}
	for _, argument := range arguments.Nodes {
		verdict = combineSafety(verdict, s.typeNode(argument, subst, bindings, false))
	}
	return verdict
}

// typeReference scans an application or declaration reference. A reference to a
// bound type parameter follows its concrete binding; a type alias application
// follows the alias declaration with a new binding frame and detects an
// evaluated cycle; a class or interface reference evaluates its type arguments
// and, when projected, its declaration member surface.
func (s *apiStabilitySafetyScan) typeReference(node *ast.Node, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	reference := node.AsTypeReferenceNode()
	if reference == nil {
		return apiStabilitySafetyUnknown
	}
	symbol := s.a.symbolAtTypeNameNode(reference.TypeName)
	if symbol == nil {
		return apiStabilitySafetyUnknown
	}
	switch {
	case symbol.Flags&ast.SymbolFlagsTypeParameter != 0:
		return s.typeParameterReference(symbol, subst, bindings, project)
	case symbol.Flags&ast.SymbolFlagsAlias != 0:
		if hasAliasDeclaration(symbol) {
			target := ApiStabilityImmediateAliasedSymbol(s.a.tp.checker, symbol)
			if target != nil && target != symbol {
				return s.referenceToSymbol(target, reference.TypeArguments, subst, bindings, project)
			}
		}
		return apiStabilitySafetyUnknown
	case symbol.Flags&ast.SymbolFlagsTypeAlias != 0:
		return s.aliasApplication(symbol, reference.TypeArguments, subst, bindings, project)
	case symbol.Flags&(ast.SymbolFlagsClass|ast.SymbolFlagsInterface|ast.SymbolFlagsEnum) != 0:
		verdict := s.typeArguments(reference.TypeArguments, subst, bindings)
		verdict = combineSafety(verdict, s.omittedArgumentDefaults(symbol, reference.TypeArguments, subst, bindings))
		if !project {
			return verdict
		}
		return combineSafety(verdict, s.declarationMembers(symbol, reference.TypeArguments, subst, bindings, true))
	default:
		return apiStabilitySafetyUnknown
	}
}

// referenceToSymbol resolves an import alias reference to its target symbol.
func (s *apiStabilitySafetyScan) referenceToSymbol(symbol *ast.Symbol, arguments *ast.NodeList, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	switch {
	case symbol.Flags&ast.SymbolFlagsTypeAlias != 0:
		return s.aliasApplication(symbol, arguments, subst, bindings, project)
	case symbol.Flags&ast.SymbolFlagsTypeParameter != 0:
		return s.typeParameterReference(symbol, subst, bindings, project)
	case symbol.Flags&(ast.SymbolFlagsClass|ast.SymbolFlagsInterface|ast.SymbolFlagsEnum) != 0:
		verdict := s.typeArguments(arguments, subst, bindings)
		verdict = combineSafety(verdict, s.omittedArgumentDefaults(symbol, arguments, subst, bindings))
		if !project {
			return verdict
		}
		return combineSafety(verdict, s.declarationMembers(symbol, arguments, subst, bindings, true))
	default:
		return apiStabilitySafetyUnknown
	}
}

// omittedArgumentDefaults scans the declared default annotations the checker
// evaluates when a generic application omits arguments. Later defaults are
// scanned with the earlier parameters bound, so a default that references an
// earlier parameter still follows its concrete argument. A required parameter
// the application omits is an error the guard refuses to evaluate.
func (s *apiStabilitySafetyScan) omittedArgumentDefaults(symbol *ast.Symbol, arguments *ast.NodeList, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding) apiStabilitySafety {
	defaults := safetyTypeParameterDefaults(symbol)
	params := s.a.tp.checker.GetLocalTypeParametersOfClassOrInterfaceOrTypeAlias(symbol)
	if len(defaults) == 0 || len(params) == 0 {
		return apiStabilitySafetySafe
	}
	provided := 0
	if arguments != nil {
		provided = len(arguments.Nodes)
	}
	if provided >= len(params) {
		return apiStabilitySafetySafe
	}
	args := make([]apiStabilitySafetyArg, len(params))
	for index := 0; index < provided && index < len(params); index++ {
		if argument := arguments.Nodes[index]; argument != nil {
			args[index] = apiStabilitySafetyArg{node: argument, subst: subst, bindings: bindings}
		}
	}
	frame := &apiStabilitySafetyBinding{parent: bindings, params: params, args: args}
	verdict := apiStabilitySafetySafe
	for index := provided; index < len(params); index++ {
		if index >= len(defaults) || defaults[index] == nil {
			return apiStabilitySafetyUnknown
		}
		args[index] = apiStabilitySafetyArg{node: defaults[index]}
		verdict = combineSafety(verdict, s.typeNode(defaults[index], subst, frame, false))
	}
	return verdict
}

// typeParameterReference follows a type parameter reference through the active
// substitution and binding frames. An unbound parameter keeps the annotation
// deferred, so the guarded read evaluates nothing for that reference and it is
// safe. A relation-operand scan refuses an unbound parameter instead: the
// relation may instantiate it against the other operand.
func (s *apiStabilitySafetyScan) typeParameterReference(symbol *ast.Symbol, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	s.a.noteMaterializingSymbolRead(apiStabilityMaterializationReadDeclaredType, symbol)
	parameter := s.a.tp.checker.GetDeclaredTypeOfSymbol(symbol)
	if parameter == nil {
		return apiStabilitySafetyUnknown
	}
	if argument, ok := safetyBindingLookup(bindings, parameter); ok {
		if argument.empty() {
			return apiStabilitySafetyUnknown
		}
		if argument.t != nil {
			return s.typeValue(argument.t, argument.subst, argument.bindings, project)
		}
		return s.typeNode(argument.node, argument.subst, argument.bindings, project)
	}
	mapped, parent, replaced := s.a.substitute(subst, parameter)
	if replaced {
		if mapped == nil {
			// The parameter was replaced by an argument the checker has not
			// represented; the applied value is unknown.
			return apiStabilitySafetyUnknown
		}
		if mapped == parameter {
			if s.relationOperands {
				return apiStabilitySafetyUnknown
			}
			return apiStabilitySafetySafe
		}
		return s.typeValue(mapped, parent, bindings, project)
	}
	if s.relationOperands {
		return apiStabilitySafetyUnknown
	}
	return apiStabilitySafetySafe
}

// safetyBindingLookup finds the binding of a declared type parameter in a
// binding chain.
func safetyBindingLookup(bindings *apiStabilitySafetyBinding, parameter *checker.Type) (apiStabilitySafetyArg, bool) {
	if parameter == nil {
		return apiStabilitySafetyArg{}, false
	}
	symbol := parameter.Symbol()
	for frame := bindings; frame != nil; frame = frame.parent {
		for index, candidate := range frame.params {
			if index >= len(frame.args) {
				continue
			}
			if candidate == parameter || symbol != nil && candidate.Symbol() == symbol {
				return frame.args[index], true
			}
		}
	}
	return apiStabilitySafetyArg{}, false
}

// aliasApplication follows one application of a type alias. The alias
// declaration is scanned with a binding frame for its declared parameters. A
// repeated application of the same alias with the same concrete arguments on
// the eager path is an evaluated cycle; a cycle crossed only through lazy
// positions is never scanned, so reaching it here means the cycle is
// evaluated.
func (s *apiStabilitySafetyScan) aliasApplication(symbol *ast.Symbol, arguments *ast.NodeList, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	declaration := apiStabilityTypeAliasDeclaration(symbol)
	if declaration == nil {
		return apiStabilitySafetyUnknown
	}
	params := s.a.tp.checker.GetLocalTypeParametersOfClassOrInterfaceOrTypeAlias(symbol)
	args := make([]apiStabilitySafetyArg, len(params))
	verdict := apiStabilitySafetySafe
	if arguments != nil {
		for index, argument := range arguments.Nodes {
			if argument == nil {
				continue
			}
			// Every explicit argument is evaluated eagerly when the checker
			// creates the application, even when the alias right-hand side
			// ignores it. Scan each provided argument in the environment it was
			// written in.
			verdict = combineSafety(verdict, s.typeNode(argument, subst, bindings, false))
			if index >= len(params) {
				continue
			}
			args[index] = apiStabilitySafetyArg{node: argument, subst: subst, bindings: bindings}
		}
	}

	// Omitted arguments are filled from declared defaults, which the checker
	// evaluates when it creates the application.
	var defaults []*ast.Node
	if len(params) != 0 {
		defaultNodes := safetyTypeParameterDefaults(symbol)
		for index := range params {
			if !args[index].empty() {
				continue
			}
			if index < len(defaultNodes) && defaultNodes[index] != nil {
				args[index] = apiStabilitySafetyArg{node: defaultNodes[index]}
				defaults = append(defaults, defaultNodes[index])
				continue
			}
			// A required argument the application omits is an error the guard
			// refuses to evaluate.
			return apiStabilitySafetyUnknown
		}
	}
	for _, application := range s.stack {
		if application.symbol == symbol && safetyArgsEqual(application.args, args) {
			return apiStabilitySafetyRecursive
		}
	}
	if len(s.stack) >= apiStabilitySafetyMaxDepth {
		return apiStabilitySafetyUnknown
	}
	frame := &apiStabilitySafetyBinding{parent: bindings, params: params, args: args}
	s.stack = append(s.stack, apiStabilitySafetyApplication{symbol: symbol, args: args})
	defer func() { s.stack = s.stack[:len(s.stack)-1] }()
	for _, defaultNode := range defaults {
		verdict = combineSafety(verdict, s.typeNode(defaultNode, subst, frame, project))
	}
	return combineSafety(verdict, s.typeNode(declaration.Type(), subst, frame, project))
}

// typeValue scans one represented type. The represented graph is the primary
// carrier: it is followed through its concrete structure, and any type
// parameter is resolved through the active substitution or binding frames.
func (s *apiStabilitySafetyScan) typeValue(t *checker.Type, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	if t == nil {
		return apiStabilitySafetySafe
	}
	if s.exceeded() {
		return apiStabilitySafetyUnknown
	}
	t = nonDistributedParameter(s.a.tp.checker, t)
	flags := t.Flags()
	switch {
	case flags&checker.TypeFlagsTypeParameter != 0:
		if parameter, replaced := s.mappedParameterValue(t, subst, bindings); replaced {
			if parameter == nil {
				return apiStabilitySafetyUnknown
			}
			if parameter == t {
				return apiStabilitySafetySafe
			}
			return s.typeValue(parameter, subst, bindings, project)
		}
		return s.constraintSafety(t, subst, bindings, project)
	case flags&checker.TypeFlagsUnionOrIntersection != 0:
		verdict := apiStabilitySafetySafe
		for _, member := range t.Types() {
			verdict = combineSafety(verdict, s.typeValue(member, subst, bindings, project))
		}
		return verdict
	case flags&checker.TypeFlagsConditional != 0:
		return s.conditionalValue(t, subst, bindings, project)
	case flags&checker.TypeFlagsIndexedAccess != 0:
		indexed := t.AsIndexedAccessType()
		if indexed == nil {
			return apiStabilitySafetyUnknown
		}
		verdict := s.typeValue(indexed.ObjectType(), subst, bindings, true)
		return combineSafety(verdict, s.typeValue(indexed.IndexType(), subst, bindings, project))
	case flags&checker.TypeFlagsIndex != 0:
		index := t.AsIndexType()
		if index == nil {
			return apiStabilitySafetyUnknown
		}
		return s.typeValue(index.Target(), subst, bindings, true)
	case flags&checker.TypeFlagsTemplateLiteral != 0:
		verdict := apiStabilitySafetySafe
		if template := t.AsTemplateLiteralType(); template != nil {
			for _, part := range template.Types() {
				verdict = combineSafety(verdict, s.typeValue(part, subst, bindings, project))
			}
		}
		return verdict
	case flags&checker.TypeFlagsStringMapping != 0:
		if mapping := t.AsStringMappingType(); mapping != nil {
			return s.typeValue(mapping.Target(), subst, bindings, project)
		}
		return apiStabilitySafetySafe
	case flags&checker.TypeFlagsSubstitution != 0:
		if substitution := t.AsSubstitutionType(); substitution != nil {
			verdict := s.typeValue(substitution.BaseType(), subst, bindings, project)
			return combineSafety(verdict, s.typeValue(substitution.SubstConstraint(), subst, bindings, project))
		}
		return apiStabilitySafetySafe
	case flags&checker.TypeFlagsObject != 0:
		return s.objectValue(t, subst, bindings, project)
	default:
		return apiStabilitySafetySafe
	}
}

// mappedParameterValue resolves a type parameter through the binding chain or
// the active substitution frames. The boolean reports whether the parameter is
// replaced.
func (s *apiStabilitySafetyScan) mappedParameterValue(t *checker.Type, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding) (*checker.Type, bool) {
	if argument, ok := safetyBindingLookup(bindings, t); ok {
		return s.materializeArgument(argument), true
	}
	mapped, _, replaced := s.a.substitute(subst, t)
	if replaced {
		return mapped, true
	}
	return nil, false
}

// materializeArgument returns the represented type of a captured application
// argument without performing the guarded resolution: a represented argument is
// resolved through its own environment, a primitive or literal argument is read
// from the checker intrinsics, and a type parameter argument is followed
// through its outer binding. Anything else stays nil.
func (s *apiStabilitySafetyScan) materializeArgument(argument apiStabilitySafetyArg) *checker.Type {
	if argument.t != nil {
		if argument.t.Flags()&checker.TypeFlagsTypeParameter == 0 {
			return argument.t
		}
		if s.materializing[argument.t] {
			return nil
		}
		if s.materializing == nil {
			s.materializing = make(map[*checker.Type]bool)
		}
		s.materializing[argument.t] = true
		mapped, replaced := s.mappedParameterValue(argument.t, argument.subst, argument.bindings)
		delete(s.materializing, argument.t)
		if replaced {
			return mapped
		}
		return argument.t
	}
	if argument.node == nil {
		return nil
	}
	if represented := checker.GetResolvedTypeFromTypeNode(s.a.tp.checker, argument.node); represented != nil {
		if mapped, replaced := s.mappedParameterValue(represented, argument.subst, argument.bindings); replaced {
			return mapped
		}
		return represented
	}
	if intrinsic := apiStabilityIntrinsicTypeOfNode(s.a.tp.checker, argument.node); intrinsic != nil {
		return intrinsic
	}
	if argument.node.Kind == ast.KindTypeReference {
		reference := argument.node.AsTypeReferenceNode()
		symbol := s.a.symbolAtTypeNameNode(reference.TypeName)
		if symbol != nil && symbol.Flags&ast.SymbolFlagsTypeParameter != 0 {
			s.a.noteMaterializingSymbolRead(apiStabilityMaterializationReadDeclaredType, symbol)
			parameter := s.a.tp.checker.GetDeclaredTypeOfSymbol(symbol)
			if parameter != nil {
				if s.materializing[parameter] {
					return nil
				}
				if s.materializing == nil {
					s.materializing = make(map[*checker.Type]bool)
				}
				s.materializing[parameter] = true
				mapped, replaced := s.mappedParameterValue(parameter, argument.subst, argument.bindings)
				delete(s.materializing, parameter)
				if replaced {
					return mapped
				}
			}
		}
	}
	// A concrete argument annotation (a reference to a declaration, an
	// application, a literal) is only resolved when the guard establishes that
	// its own resolution is bounded. The nested verification shares this
	// analysis's work counter and never recurses into the same argument.
	if s.materializingNodes[argument.node] {
		return nil
	}
	if s.materializingNodes == nil {
		s.materializingNodes = make(map[*ast.Node]bool)
	}
	s.materializingNodes[argument.node] = true
	nested := s.a.newSafetyScan()
	nested.relationOperands = s.relationOperands
	safe := nested.typeNode(argument.node, argument.subst, argument.bindings, false) == apiStabilitySafetySafe
	delete(s.materializingNodes, argument.node)
	if safe {
		s.a.noteMaterializingNodeRead(argument.node)
		return s.a.tp.checker.GetTypeFromTypeNode(argument.node)
	}
	return nil
}

// apiStabilityIntrinsicTypeOfNode returns the checker intrinsic for a primitive
// keyword annotation. The mapping is a direct field read; no annotation is
// resolved.
func apiStabilityIntrinsicTypeOfNode(c *checker.Checker, node *ast.Node) *checker.Type {
	if node == nil {
		return nil
	}
	switch node.Kind {
	case ast.KindAnyKeyword:
		return c.GetAnyType()
	case ast.KindUnknownKeyword:
		return c.GetUnknownType()
	case ast.KindNeverKeyword:
		return c.GetNeverType()
	case ast.KindVoidKeyword:
		return c.GetVoidType()
	case ast.KindUndefinedKeyword:
		return c.GetUndefinedType()
	case ast.KindNullKeyword:
		return c.GetNullType()
	case ast.KindStringKeyword:
		return c.GetStringType()
	case ast.KindNumberKeyword:
		return c.GetNumberType()
	case ast.KindBigIntKeyword:
		return c.GetBigIntType()
	case ast.KindBooleanKeyword:
		return c.GetBooleanType()
	case ast.KindSymbolKeyword:
		return c.GetESSymbolType()
	case ast.KindObjectKeyword:
		return checker.GetNonPrimitiveType(c)
	}
	return nil
}

// constraintSafety follows the constraint of an unbound type parameter when the
// guarded read may resolve it (an apparent-type request on a type parameter). A
// relation-operand scan refuses an unbound parameter instead: the relation may
// instantiate it against the other operand.
func (s *apiStabilitySafetyScan) constraintSafety(t *checker.Type, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	if s.relationOperands {
		return apiStabilitySafetyUnknown
	}
	annotation := typeParameterAnnotationNode(t, true)
	if annotation == nil {
		return apiStabilitySafetySafe
	}
	return s.typeNode(annotation, subst, bindings, project)
}

// objectValue scans a represented object type.
func (s *apiStabilitySafetyScan) objectValue(t *checker.Type, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	flags := t.ObjectFlags()
	if flags&(checker.ObjectFlagsReverseMapped|checker.ObjectFlagsEvolvingArray) != 0 {
		return apiStabilitySafetySafe
	}
	if flags&checker.ObjectFlagsMapped != 0 {
		return s.mappedValue(t, subst, bindings, project)
	}
	if flags&checker.ObjectFlagsReference != 0 && t.Target() != nil && t.Target() != t {
		target := t.Target()
		arguments := checker.GetResolvedTypeArguments(s.a.tp.checker, t)
		verdict := apiStabilitySafetySafe
		for _, argument := range arguments {
			verdict = combineSafety(verdict, s.typeValue(argument, subst, bindings, project))
		}
		if !project {
			return verdict
		}
		if s.relationOperands {
			// Bind the reference's own represented arguments so a member
			// annotation that mentions the target's type parameters resolves to
			// the concrete argument the relation will compare.
			return combineSafety(verdict, s.declarationMembersOfRepresentedArguments(target, arguments, subst, bindings, true))
		}
		return combineSafety(verdict, s.declarationMembers(target.Symbol(), nil, subst, bindings, true))
	}
	symbol := t.Symbol()
	if symbol == nil {
		return apiStabilitySafetySafe
	}
	return s.declarationStructure(symbol, subst, bindings, project)
}

// mappedValue scans a represented mapped type through its represented mapped
// components.
func (s *apiStabilitySafetyScan) mappedValue(t *checker.Type, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	mapped := t.AsMappedType()
	if mapped == nil {
		return apiStabilitySafetySafe
	}
	verdict := s.typeValue(checker.GetMappedTypeConstraintType(mapped), subst, bindings, project)
	verdict = combineSafety(verdict, s.typeValue(checker.GetMappedTypeNameType(mapped), subst, bindings, project))
	return combineSafety(verdict, s.typeValue(checker.GetMappedTypeTemplateType(mapped), subst, bindings, project))
}

// conditionalValue scans a represented conditional type. A deferred
// conditional whose operands still mention an unbound parameter is not
// evaluated by the guarded read. A concrete conditional must be resolved: the
// guard selects the branch with the compiler's own relation instead of
// guessing, and refuses to evaluate a conditional whose outcome the relation
// cannot establish without inference.
func (s *apiStabilitySafetyScan) conditionalValue(t *checker.Type, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	conditional := t.AsConditionalType()
	if conditional == nil {
		return apiStabilitySafetyUnknown
	}
	check := conditional.CheckType()
	extends := conditional.ExtendsType()
	checkSafe := s.typeValue(check, subst, bindings, project)
	extendsSafe := s.typeValue(extends, subst, bindings, project)
	if checkSafe != apiStabilitySafetySafe || extendsSafe != apiStabilitySafetySafe {
		return combineSafety(checkSafe, extendsSafe)
	}
	if s.containsUnboundParameter(check, subst, bindings, nil) || s.containsUnboundParameter(extends, subst, bindings, nil) {
		return apiStabilitySafetySafe
	}
	selection := s.selectConditionalBranch(check, extends, subst, bindings)
	if selection == branchUnknown {
		return apiStabilitySafetyUnknown
	}
	verdict := apiStabilitySafetySafe
	if selection&branchTrue != 0 {
		verdict = combineSafety(verdict, s.conditionalBranchValue(t, true, subst, bindings, project))
	}
	if selection&branchFalse != 0 {
		verdict = combineSafety(verdict, s.conditionalBranchValue(t, false, subst, bindings, project))
	}
	return verdict
}

// conditionalBranchValue scans one represented branch of a conditional. The
// materialized branch is preferred; otherwise the declared branch annotation is
// scanned with the conditional's own mapper bindings.
func (s *apiStabilitySafetyScan) conditionalBranchValue(t *checker.Type, trueBranch bool, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	if represented := checker.GetResolvedConditionalTypeBranch(s.a.tp.checker, t, trueBranch); represented != nil {
		return s.typeValue(represented, subst, bindings, project)
	}
	node := checker.GetConditionalTypeBranchNode(s.a.tp.checker, t, trueBranch)
	if node == nil {
		return apiStabilitySafetyUnknown
	}
	return s.typeNode(node, subst, bindings, project)
}

// conditionalNode scans a raw conditional annotation. Operands that mention an
// unbound parameter keep the conditional deferred and safe: constructing a
// deferred conditional never evaluates its branches, even when an infer binder
// appears. A concrete conditional is resolved through the compiler relation; an
// infer binder in its extends type has an inferred outcome the relation cannot
// establish without instantiating the application, so the guard refuses it.
func (s *apiStabilitySafetyScan) conditionalNode(node *ast.Node, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	conditional := node.AsConditionalTypeNode()
	if conditional == nil {
		return apiStabilitySafetyUnknown
	}
	checkSafe := s.typeNode(conditional.CheckType, subst, bindings, project)
	extendsSafe := s.typeNode(conditional.ExtendsType, subst, bindings, project)
	if checkSafe != apiStabilitySafetySafe || extendsSafe != apiStabilitySafetySafe {
		return combineSafety(checkSafe, extendsSafe)
	}
	if s.typeNodeMentionsUnboundParameter(conditional.CheckType, subst, bindings, make(map[*ast.Node]bool)) ||
		s.typeNodeMentionsUnboundParameter(conditional.ExtendsType, subst, bindings, make(map[*ast.Node]bool)) {
		return apiStabilitySafetySafe
	}
	if s.typeNodeMentionsInfer(conditional.ExtendsType, make(map[*ast.Node]bool)) {
		return apiStabilitySafetyUnknown
	}
	check := s.operandType(conditional.CheckType, subst, bindings)
	extends := s.operandType(conditional.ExtendsType, subst, bindings)
	selection := s.selectConditionalBranch(check, extends, subst, bindings)
	if selection == branchUnknown {
		return apiStabilitySafetyUnknown
	}
	verdict := apiStabilitySafetySafe
	if selection&branchTrue != 0 {
		verdict = combineSafety(verdict, s.typeNode(conditional.TrueType, subst, bindings, project))
	}
	if selection&branchFalse != 0 {
		verdict = combineSafety(verdict, s.typeNode(conditional.FalseType, subst, bindings, project))
	}
	return verdict
}

// branchSelection is a bit set of conditional branches the compiler relation
// evaluates.
type branchSelection int

const (
	branchUnknown branchSelection = 1 << iota
	branchTrue
	branchFalse
)

// selectConditionalBranch asks the compiler relation which branch a concrete
// conditional evaluates. The check type distributes over unions, so a mixed
// distribution evaluates both branches. The relation is only asked when both
// operands are fully represented and free of type parameters the active
// environment replaces, and when a relation-operand pre-flight establishes that
// resolving their full projected surfaces is bounded. The guard never
// instantiates a branch to answer the question and never evaluates an
// unresolved compound substitution.
func (s *apiStabilitySafetyScan) selectConditionalBranch(check, extends *checker.Type, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding) branchSelection {
	if check == nil || extends == nil {
		return branchUnknown
	}
	if s.exceeded() {
		return branchUnknown
	}
	c := s.a.tp.checker
	if c.GetAnyType() == check {
		// `any extends X ? A : B` evaluates to `A | B`.
		return branchTrue | branchFalse
	}
	if check.Flags()&checker.TypeFlagsNever != 0 {
		// `never` is assignable to every target, and a distributive check over
		// `never` evaluates no branch at all. Scanning only the true branch is
		// sound for both outcomes and never authorizes the false branch.
		return branchTrue
	}
	if check == extends {
		// Identical represented types are assignable without evaluating any
		// member surface.
		return branchTrue
	}
	if extends == c.GetAnyType() || extends == c.GetUnknownType() {
		// Every type is assignable to `any` and to `unknown`.
		return branchTrue
	}
	if s.containsReplacedParameter(check, subst, bindings, nil) ||
		s.containsReplacedParameter(extends, subst, bindings, nil) {
		// An operand still mentions a type parameter the active environment
		// replaces. The checker would instantiate the compound operand before
		// relating it; the guard never authorizes evaluating an unresolved
		// compound substitution.
		return branchUnknown
	}
	// The relation compares the full projected member surfaces of both operands:
	// property types, call and construct signature returns and index infos are
	// all resolved while relating. Unless every one of those components is
	// itself established as bounded by a relation-operand scan, the relation
	// must not be asked, because it would expand them eagerly.
	operands := s.a.newSafetyScan()
	operands.relationOperands = true
	if operands.typeValue(check, subst, bindings, true) != apiStabilitySafetySafe ||
		operands.typeValue(extends, subst, bindings, true) != apiStabilitySafetySafe {
		return branchUnknown
	}
	checkMembers := []*checker.Type{check}
	if check.Flags()&checker.TypeFlagsUnion != 0 {
		checkMembers = append(checkMembers[:0:0], check.Types()...)
	}
	selection := branchSelection(0)
	for _, member := range checkMembers {
		if checker.Checker_isTypeAssignableTo(c, member, extends) {
			selection |= branchTrue
		} else {
			selection |= branchFalse
		}
	}
	return selection
}

// operandType returns the represented type of a concrete conditional operand,
// following only a top-level parameter reference through the active
// environment. A compound operand that still mentions a substituted parameter
// keeps that parameter and is refused by the relation-operand pre-flight; the
// guard never instantiates a compound substitution itself.
func (s *apiStabilitySafetyScan) operandType(node *ast.Node, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding) *checker.Type {
	if node == nil {
		return nil
	}
	represented := checker.GetResolvedTypeFromTypeNode(s.a.tp.checker, node)
	if represented == nil {
		s.a.noteMaterializingNodeRead(node)
		represented = s.a.tp.checker.GetTypeFromTypeNode(node)
	}
	if represented == nil {
		return nil
	}
	if mapped, replaced := s.mappedParameterValue(represented, subst, bindings); replaced {
		return mapped
	}
	return represented
}

// containsUnboundParameter reports whether a represented type still mentions a
// type parameter that no active frame replaces.
func (s *apiStabilitySafetyScan) containsUnboundParameter(t *checker.Type, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, active map[*checker.Type]bool) bool {
	if t == nil {
		return false
	}
	if active == nil {
		active = make(map[*checker.Type]bool)
	}
	if active[t] {
		return false
	}
	active[t] = true
	defer delete(active, t)
	t = nonDistributedParameter(s.a.tp.checker, t)
	flags := t.Flags()
	switch {
	case flags&checker.TypeFlagsTypeParameter != 0:
		if parameter, replaced := s.mappedParameterValue(t, subst, bindings); replaced {
			if parameter == nil {
				return true
			}
			if parameter == t {
				return false
			}
			return s.containsUnboundParameter(parameter, subst, bindings, active)
		}
		return true
	case flags&checker.TypeFlagsUnionOrIntersection != 0:
		for _, member := range t.Types() {
			if s.containsUnboundParameter(member, subst, bindings, active) {
				return true
			}
		}
	case flags&checker.TypeFlagsObject != 0 && t.ObjectFlags()&checker.ObjectFlagsReference != 0:
		for _, argument := range checker.GetResolvedTypeArguments(s.a.tp.checker, t) {
			if s.containsUnboundParameter(argument, subst, bindings, active) {
				return true
			}
		}
	case flags&checker.TypeFlagsConditional != 0:
		if conditional := t.AsConditionalType(); conditional != nil {
			return s.containsUnboundParameter(conditional.CheckType(), subst, bindings, active) ||
				s.containsUnboundParameter(conditional.ExtendsType(), subst, bindings, active)
		}
	case flags&checker.TypeFlagsIndexedAccess != 0:
		if indexed := t.AsIndexedAccessType(); indexed != nil {
			return s.containsUnboundParameter(indexed.ObjectType(), subst, bindings, active) ||
				s.containsUnboundParameter(indexed.IndexType(), subst, bindings, active)
		}
	case flags&checker.TypeFlagsIndex != 0:
		if index := t.AsIndexType(); index != nil {
			return s.containsUnboundParameter(index.Target(), subst, bindings, active)
		}
	case flags&checker.TypeFlagsTemplateLiteral != 0:
		if template := t.AsTemplateLiteralType(); template != nil {
			for _, part := range template.Types() {
				if s.containsUnboundParameter(part, subst, bindings, active) {
					return true
				}
			}
		}
	}
	return false
}

// containsReplacedParameter reports whether a represented type still mentions a
// type parameter the active environment replaces. The compiler would
// instantiate such a compound operand before evaluating a relation over it, so
// the guard treats it as unknown instead of guessing a substitution. Reference
// arguments are followed directly; an anonymous compound the checker has not
// instantiated is inspected through its raw declarations, because its
// unsubstituted representation does not show the parameter its resolved members
// would carry.
func (s *apiStabilitySafetyScan) containsReplacedParameter(t *checker.Type, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, active map[*checker.Type]bool) bool {
	if t == nil {
		return false
	}
	if active == nil {
		active = make(map[*checker.Type]bool)
	}
	if active[t] {
		return false
	}
	active[t] = true
	defer delete(active, t)
	t = nonDistributedParameter(s.a.tp.checker, t)
	flags := t.Flags()
	switch {
	case flags&checker.TypeFlagsTypeParameter != 0:
		_, replaced := s.mappedParameterValue(t, subst, bindings)
		return replaced
	case flags&checker.TypeFlagsUnionOrIntersection != 0:
		for _, member := range t.Types() {
			if s.containsReplacedParameter(member, subst, bindings, active) {
				return true
			}
		}
	case flags&checker.TypeFlagsObject != 0:
		if t.ObjectFlags()&checker.ObjectFlagsReference != 0 {
			for _, argument := range checker.GetResolvedTypeArguments(s.a.tp.checker, t) {
				if s.containsReplacedParameter(argument, subst, bindings, active) {
					return true
				}
			}
			return false
		}
		if t.ObjectFlags()&checker.ObjectFlagsInstantiated != 0 {
			// The represented instantiation substitutes its members through
			// its own mapper; the relation-operand scan refuses any raw member
			// parameter it cannot bind, so no additional raw detection applies.
			return false
		}
		// An anonymous compound the checker has not instantiated still
		// contains its raw member annotations. A replaced parameter inside one
		// of them is instantiated before the relation compares the operand,
		// which the represented structure does not show, so the operand is
		// refused.
		return s.rawDeclarationsMentionReplacedParameter(t.Symbol(), subst, bindings)
	case flags&checker.TypeFlagsConditional != 0:
		if conditional := t.AsConditionalType(); conditional != nil {
			return s.containsReplacedParameter(conditional.CheckType(), subst, bindings, active) ||
				s.containsReplacedParameter(conditional.ExtendsType(), subst, bindings, active)
		}
	case flags&checker.TypeFlagsIndexedAccess != 0:
		if indexed := t.AsIndexedAccessType(); indexed != nil {
			return s.containsReplacedParameter(indexed.ObjectType(), subst, bindings, active) ||
				s.containsReplacedParameter(indexed.IndexType(), subst, bindings, active)
		}
	case flags&checker.TypeFlagsIndex != 0:
		if index := t.AsIndexType(); index != nil {
			return s.containsReplacedParameter(index.Target(), subst, bindings, active)
		}
	case flags&checker.TypeFlagsTemplateLiteral != 0:
		if template := t.AsTemplateLiteralType(); template != nil {
			for _, part := range template.Types() {
				if s.containsReplacedParameter(part, subst, bindings, active) {
					return true
				}
			}
		}
	case flags&checker.TypeFlagsSubstitution != 0:
		if substitution := t.AsSubstitutionType(); substitution != nil {
			return s.containsReplacedParameter(substitution.BaseType(), subst, bindings, active) ||
				s.containsReplacedParameter(substitution.SubstConstraint(), subst, bindings, active)
		}
	}
	return false
}

// typeNodeMentionsUnboundParameter reports whether a raw annotation mentions a
// type parameter no active frame replaces. An infer binder is bound by its own
// conditional, and a callable's own type parameters are bound by the callable,
// so neither makes the surrounding conditional deferred. A conditional whose
// operands still mention an unbound parameter is deferred and never evaluated
// by an ordinary read.
func (s *apiStabilitySafetyScan) typeNodeMentionsUnboundParameter(node *ast.Node, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, visiting map[*ast.Node]bool) bool {
	return s.typeNodeMentionsUnboundParameterScoped(node, subst, bindings, visiting, nil)
}

// typeNodeMentionsUnboundParameterScoped carries the type parameters the
// enclosing callables bind so a reference to one of them is not mistaken for an
// unbound parameter of the surrounding conditional.
func (s *apiStabilitySafetyScan) typeNodeMentionsUnboundParameterScoped(node *ast.Node, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, visiting map[*ast.Node]bool, scope map[*checker.Type]bool) bool {
	if node == nil || visiting[node] {
		return false
	}
	visiting[node] = true
	defer delete(visiting, node)
	switch node.Kind {
	case ast.KindInferType:
		return false
	case ast.KindTypeReference:
		reference := node.AsTypeReferenceNode()
		symbol := s.a.symbolAtTypeNameNode(reference.TypeName)
		if symbol != nil && symbol.Flags&ast.SymbolFlagsTypeParameter != 0 {
			s.a.noteMaterializingSymbolRead(apiStabilityMaterializationReadDeclaredType, symbol)
			parameter := s.a.tp.checker.GetDeclaredTypeOfSymbol(symbol)
			if parameter != nil && !scope[parameter] {
				if _, replaced := s.mappedParameterValue(parameter, subst, bindings); !replaced {
					return true
				}
			}
		}
		if reference.TypeArguments != nil {
			for _, argument := range reference.TypeArguments.Nodes {
				if s.typeNodeMentionsUnboundParameterScoped(argument, subst, bindings, visiting, scope) {
					return true
				}
			}
		}
	case ast.KindParenthesizedType:
		return s.typeNodeMentionsUnboundParameterScoped(node.AsParenthesizedTypeNode().Type, subst, bindings, visiting, scope)
	case ast.KindArrayType:
		return s.typeNodeMentionsUnboundParameterScoped(node.AsArrayTypeNode().ElementType, subst, bindings, visiting, scope)
	case ast.KindOptionalType:
		return s.typeNodeMentionsUnboundParameterScoped(node.AsOptionalTypeNode().Type, subst, bindings, visiting, scope)
	case ast.KindRestType:
		return s.typeNodeMentionsUnboundParameterScoped(node.AsRestTypeNode().Type, subst, bindings, visiting, scope)
	case ast.KindTypeOperator:
		return s.typeNodeMentionsUnboundParameterScoped(node.AsTypeOperatorNode().Type, subst, bindings, visiting, scope)
	case ast.KindNamedTupleMember:
		return s.typeNodeMentionsUnboundParameterScoped(node.AsNamedTupleMember().Type, subst, bindings, visiting, scope)
	case ast.KindTupleType:
		if elements := node.AsTupleTypeNode().Elements; elements != nil {
			for _, element := range elements.Nodes {
				if s.typeNodeMentionsUnboundParameterScoped(element, subst, bindings, visiting, scope) {
					return true
				}
			}
		}
	case ast.KindUnionType:
		if list := node.AsUnionTypeNode().Types; list != nil {
			for _, member := range list.Nodes {
				if s.typeNodeMentionsUnboundParameterScoped(member, subst, bindings, visiting, scope) {
					return true
				}
			}
		}
	case ast.KindIntersectionType:
		if list := node.AsIntersectionTypeNode().Types; list != nil {
			for _, member := range list.Nodes {
				if s.typeNodeMentionsUnboundParameterScoped(member, subst, bindings, visiting, scope) {
					return true
				}
			}
		}
	case ast.KindTemplateLiteralType:
		if template := node.AsTemplateLiteralTypeNode(); template != nil && template.TemplateSpans != nil {
			for _, span := range template.TemplateSpans.Nodes {
				if span == nil || span.Kind != ast.KindTemplateLiteralTypeSpan {
					continue
				}
				if s.typeNodeMentionsUnboundParameterScoped(span.AsTemplateLiteralTypeSpan().Type, subst, bindings, visiting, scope) {
					return true
				}
			}
		}
	case ast.KindIndexedAccessType:
		indexed := node.AsIndexedAccessTypeNode()
		return s.typeNodeMentionsUnboundParameterScoped(indexed.ObjectType, subst, bindings, visiting, scope) ||
			s.typeNodeMentionsUnboundParameterScoped(indexed.IndexType, subst, bindings, visiting, scope)
	case ast.KindConditionalType:
		conditional := node.AsConditionalTypeNode()
		return s.typeNodeMentionsUnboundParameterScoped(conditional.CheckType, subst, bindings, visiting, scope) ||
			s.typeNodeMentionsUnboundParameterScoped(conditional.ExtendsType, subst, bindings, visiting, scope)
	case ast.KindTypeLiteral:
		for _, member := range node.Members() {
			if member == nil {
				continue
			}
			if functionLike := member.FunctionLikeData(); functionLike != nil {
				if s.functionLikeMentionsUnboundParameter(functionLike, subst, bindings, visiting, scope) {
					return true
				}
				continue
			}
			if s.typeNodeMentionsUnboundParameterScoped(member, subst, bindings, visiting, scope) {
				return true
			}
		}
	case ast.KindPropertySignature:
		return s.typeNodeMentionsUnboundParameterScoped(node.AsPropertySignatureDeclaration().Type, subst, bindings, visiting, scope)
	case ast.KindPropertyDeclaration:
		return s.typeNodeMentionsUnboundParameterScoped(node.AsPropertyDeclaration().Type, subst, bindings, visiting, scope)
	case ast.KindIndexSignature:
		declaration := node.AsIndexSignatureDeclaration()
		if declaration == nil {
			return false
		}
		if declaration.Parameters != nil && len(declaration.Parameters.Nodes) == 1 && declaration.Parameters.Nodes[0].Kind == ast.KindParameter {
			if s.typeNodeMentionsUnboundParameterScoped(declaration.Parameters.Nodes[0].AsParameterDeclaration().Type, subst, bindings, visiting, scope) {
				return true
			}
		}
		return s.typeNodeMentionsUnboundParameterScoped(declaration.Type, subst, bindings, visiting, scope)
	case ast.KindMappedType:
		mapped := node.AsMappedTypeNode()
		if mapped == nil {
			return false
		}
		if s.typeNodeMentionsUnboundParameterScoped(mapped.NameType, subst, bindings, visiting, scope) ||
			s.typeNodeMentionsUnboundParameterScoped(mapped.Type, subst, bindings, visiting, scope) {
			return true
		}
		if mapped.TypeParameter != nil {
			if declaration := mapped.TypeParameter.AsTypeParameterDeclaration(); declaration != nil {
				return s.typeNodeMentionsUnboundParameterScoped(declaration.Constraint, subst, bindings, visiting, scope)
			}
		}
	case ast.KindFunctionType, ast.KindConstructorType:
		if functionLike := node.FunctionLikeData(); functionLike != nil {
			return s.functionLikeMentionsUnboundParameter(functionLike, subst, bindings, visiting, scope)
		}
	}
	return false
}

// functionLikeMentionsUnboundParameter walks the constraints, parameter and
// return annotations of a function-like declaration looking for an unbound
// parameter. The declaration's own type parameters are bound by the callable
// itself and are excluded through the scope, so a generic operand is not
// mistaken for a deferred one.
func (s *apiStabilitySafetyScan) functionLikeMentionsUnboundParameter(functionLike *ast.FunctionLikeBase, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, visiting map[*ast.Node]bool, scope map[*checker.Type]bool) bool {
	if functionLike == nil {
		return false
	}
	own := scope
	if functionLike.TypeParameters != nil {
		own = make(map[*checker.Type]bool, len(scope)+len(functionLike.TypeParameters.Nodes))
		for parameter := range scope {
			own[parameter] = true
		}
		for _, typeParameter := range functionLike.TypeParameters.Nodes {
			if typeParameter == nil || typeParameter.Kind != ast.KindTypeParameter {
				continue
			}
			if name := ast.GetNameOfDeclaration(typeParameter); name != nil {
				if symbol := s.a.symbolAtTypeNameNode(name); symbol != nil {
					// The reference symbol of a type parameter can be a
					// distinct instance; the declared type is the shared
					// identity, so it is the scope key.
					s.a.noteMaterializingSymbolRead(apiStabilityMaterializationReadDeclaredType, symbol)
					if declared := s.a.tp.checker.GetDeclaredTypeOfSymbol(symbol); declared != nil {
						own[declared] = true
					}
				}
			}
		}
		for _, typeParameter := range functionLike.TypeParameters.Nodes {
			if typeParameter == nil || typeParameter.Kind != ast.KindTypeParameter {
				continue
			}
			declaration := typeParameter.AsTypeParameterDeclaration()
			if declaration == nil {
				continue
			}
			// A constraint is evaluated in the callable's own scope, so an
			// earlier own type parameter referenced by a later constraint is
			// bound as well.
			if s.typeNodeMentionsUnboundParameterScoped(declaration.Constraint, subst, bindings, visiting, own) {
				return true
			}
		}
	}
	for _, parameter := range functionLikeParameterNodes(functionLike) {
		if parameter != nil && parameter.Kind == ast.KindParameter {
			if s.typeNodeMentionsUnboundParameterScoped(parameter.AsParameterDeclaration().Type, subst, bindings, visiting, own) {
				return true
			}
		}
	}
	return functionLike.Type != nil && s.typeNodeMentionsUnboundParameterScoped(functionLike.Type, subst, bindings, visiting, own)
}

// typeNodeMentionsInfer reports whether an annotation mentions an infer binder.
func (s *apiStabilitySafetyScan) typeNodeMentionsInfer(node *ast.Node, visiting map[*ast.Node]bool) bool {
	if node == nil || visiting[node] {
		return false
	}
	visiting[node] = true
	defer delete(visiting, node)
	switch node.Kind {
	case ast.KindInferType:
		return true
	case ast.KindParenthesizedType:
		return s.typeNodeMentionsInfer(node.AsParenthesizedTypeNode().Type, visiting)
	case ast.KindArrayType:
		return s.typeNodeMentionsInfer(node.AsArrayTypeNode().ElementType, visiting)
	case ast.KindOptionalType:
		return s.typeNodeMentionsInfer(node.AsOptionalTypeNode().Type, visiting)
	case ast.KindRestType:
		return s.typeNodeMentionsInfer(node.AsRestTypeNode().Type, visiting)
	case ast.KindTypeOperator:
		return s.typeNodeMentionsInfer(node.AsTypeOperatorNode().Type, visiting)
	case ast.KindNamedTupleMember:
		return s.typeNodeMentionsInfer(node.AsNamedTupleMember().Type, visiting)
	case ast.KindTupleType:
		if elements := node.AsTupleTypeNode().Elements; elements != nil {
			for _, element := range elements.Nodes {
				if s.typeNodeMentionsInfer(element, visiting) {
					return true
				}
			}
		}
	case ast.KindUnionType:
		if list := node.AsUnionTypeNode().Types; list != nil {
			for _, member := range list.Nodes {
				if s.typeNodeMentionsInfer(member, visiting) {
					return true
				}
			}
		}
	case ast.KindIntersectionType:
		if list := node.AsIntersectionTypeNode().Types; list != nil {
			for _, member := range list.Nodes {
				if s.typeNodeMentionsInfer(member, visiting) {
					return true
				}
			}
		}
	case ast.KindTemplateLiteralType:
		if template := node.AsTemplateLiteralTypeNode(); template != nil && template.TemplateSpans != nil {
			for _, span := range template.TemplateSpans.Nodes {
				if span != nil && span.Kind == ast.KindTemplateLiteralTypeSpan {
					if s.typeNodeMentionsInfer(span.AsTemplateLiteralTypeSpan().Type, visiting) {
						return true
					}
				}
			}
		}
	case ast.KindIndexedAccessType:
		indexed := node.AsIndexedAccessTypeNode()
		return s.typeNodeMentionsInfer(indexed.ObjectType, visiting) || s.typeNodeMentionsInfer(indexed.IndexType, visiting)
	case ast.KindConditionalType:
		conditional := node.AsConditionalTypeNode()
		return s.typeNodeMentionsInfer(conditional.CheckType, visiting) ||
			s.typeNodeMentionsInfer(conditional.ExtendsType, visiting) ||
			s.typeNodeMentionsInfer(conditional.TrueType, visiting) ||
			s.typeNodeMentionsInfer(conditional.FalseType, visiting)
	case ast.KindTypeReference:
		reference := node.AsTypeReferenceNode()
		if reference.TypeArguments != nil {
			for _, argument := range reference.TypeArguments.Nodes {
				if s.typeNodeMentionsInfer(argument, visiting) {
					return true
				}
			}
		}
	case ast.KindFunctionType, ast.KindConstructorType:
		if functionLike := node.FunctionLikeData(); functionLike != nil {
			for _, parameter := range functionLikeParameterNodes(functionLike) {
				if parameter != nil && parameter.Kind == ast.KindParameter {
					if s.typeNodeMentionsInfer(parameter.AsParameterDeclaration().Type, visiting) {
						return true
					}
				}
			}
			if functionLike.Type != nil && s.typeNodeMentionsInfer(functionLike.Type, visiting) {
				return true
			}
		}
	}
	return false
}

// rawDeclarationsMentionReplacedParameter reports whether the raw declarations
// of a symbol contain an annotation that mentions a type parameter the active
// environment replaces. It is a non-evaluating declaration-structure walk used
// to refuse a relation operand whose represented compound does not show the
// substitution its resolved members would carry. A compound without a
// declaration cannot be established as safe and is refused.
func (s *apiStabilitySafetyScan) rawDeclarationsMentionReplacedParameter(symbol *ast.Symbol, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding) bool {
	if symbol == nil || len(symbol.Declarations) == 0 {
		return true
	}
	visiting := make(map[*ast.Node]bool)
	for _, declaration := range symbol.Declarations {
		if declaration == nil {
			continue
		}
		if s.declarationMentionsReplacedParameter(declaration, subst, bindings, visiting) {
			return true
		}
	}
	return false
}

// declarationMentionsReplacedParameter walks the public annotations of one raw
// declaration: function-like parameters and returns, type literal and heritage
// member annotations, index signatures and mapped components. It resolves
// nothing.
func (s *apiStabilitySafetyScan) declarationMentionsReplacedParameter(declaration *ast.Node, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, visiting map[*ast.Node]bool) bool {
	if declaration == nil {
		return false
	}
	switch declaration.Kind {
	case ast.KindInterfaceDeclaration, ast.KindClassDeclaration, ast.KindClassExpression, ast.KindEnumDeclaration:
		for _, member := range declaration.Members() {
			if member != nil && s.typeNodeMentionsReplacedParameter(member, subst, bindings, visiting) {
				return true
			}
		}
		return false
	case ast.KindTypeAliasDeclaration, ast.KindJSTypeAliasDeclaration:
		return s.typeNodeMentionsReplacedParameter(declaration.Type(), subst, bindings, visiting)
	case ast.KindVariableDeclaration, ast.KindPropertyDeclaration, ast.KindPropertySignature, ast.KindParameter:
		return s.typeNodeMentionsReplacedParameter(declarationAnnotationNodeOf(declaration), subst, bindings, visiting)
	default:
		return s.typeNodeMentionsReplacedParameter(declaration, subst, bindings, visiting)
	}
}

// functionLikeMentionsReplacedParameter walks the parameter, type parameter
// constraint and return annotations of a function-like declaration.
func (s *apiStabilitySafetyScan) functionLikeMentionsReplacedParameter(functionLike *ast.FunctionLikeBase, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, visiting map[*ast.Node]bool) bool {
	if functionLike == nil {
		return false
	}
	for _, parameter := range functionLikeParameterNodes(functionLike) {
		if parameter == nil || parameter.Kind != ast.KindParameter {
			continue
		}
		if s.typeNodeMentionsReplacedParameter(parameter.AsParameterDeclaration().Type, subst, bindings, visiting) {
			return true
		}
	}
	if functionLike.TypeParameters != nil {
		for _, typeParameter := range functionLike.TypeParameters.Nodes {
			if typeParameter == nil || typeParameter.Kind != ast.KindTypeParameter {
				continue
			}
			if declaration := typeParameter.AsTypeParameterDeclaration(); declaration != nil {
				if s.typeNodeMentionsReplacedParameter(declaration.Constraint, subst, bindings, visiting) {
					return true
				}
			}
		}
	}
	return s.typeNodeMentionsReplacedParameter(functionLike.Type, subst, bindings, visiting)
}

// typeNodeMentionsReplacedParameter reports whether a raw annotation mentions a
// type parameter the active environment replaces anywhere inside its structure.
// It is a non-evaluating syntax gate: a conditional operand that still contains
// such a parameter is instantiated by the checker before a relation compares
// it, so the guard refuses to ask the relation instead of guessing the
// substituted value. An `infer` binder is scoped by its own conditional and is
// not an environment parameter.
func (s *apiStabilitySafetyScan) typeNodeMentionsReplacedParameter(node *ast.Node, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, visiting map[*ast.Node]bool) bool {
	if node == nil || visiting[node] {
		return false
	}
	if functionLike := node.FunctionLikeData(); functionLike != nil {
		return s.functionLikeMentionsReplacedParameter(functionLike, subst, bindings, visiting)
	}
	visiting[node] = true
	defer delete(visiting, node)
	switch node.Kind {
	case ast.KindInferType:
		return false
	case ast.KindTypeReference:
		reference := node.AsTypeReferenceNode()
		if symbol := s.a.symbolAtTypeNameNode(reference.TypeName); symbol != nil && symbol.Flags&ast.SymbolFlagsTypeParameter != 0 {
			s.a.noteMaterializingSymbolRead(apiStabilityMaterializationReadDeclaredType, symbol)
			if parameter := s.a.tp.checker.GetDeclaredTypeOfSymbol(symbol); parameter != nil {
				if _, replaced := s.mappedParameterValue(parameter, subst, bindings); replaced {
					return true
				}
			}
		}
		if reference.TypeArguments != nil {
			for _, argument := range reference.TypeArguments.Nodes {
				if s.typeNodeMentionsReplacedParameter(argument, subst, bindings, visiting) {
					return true
				}
			}
		}
	case ast.KindParenthesizedType:
		return s.typeNodeMentionsReplacedParameter(node.AsParenthesizedTypeNode().Type, subst, bindings, visiting)
	case ast.KindArrayType:
		return s.typeNodeMentionsReplacedParameter(node.AsArrayTypeNode().ElementType, subst, bindings, visiting)
	case ast.KindOptionalType:
		return s.typeNodeMentionsReplacedParameter(node.AsOptionalTypeNode().Type, subst, bindings, visiting)
	case ast.KindRestType:
		return s.typeNodeMentionsReplacedParameter(node.AsRestTypeNode().Type, subst, bindings, visiting)
	case ast.KindTypeOperator:
		return s.typeNodeMentionsReplacedParameter(node.AsTypeOperatorNode().Type, subst, bindings, visiting)
	case ast.KindNamedTupleMember:
		return s.typeNodeMentionsReplacedParameter(node.AsNamedTupleMember().Type, subst, bindings, visiting)
	case ast.KindTupleType:
		if elements := node.AsTupleTypeNode().Elements; elements != nil {
			for _, element := range elements.Nodes {
				if s.typeNodeMentionsReplacedParameter(element, subst, bindings, visiting) {
					return true
				}
			}
		}
	case ast.KindUnionType:
		if list := node.AsUnionTypeNode().Types; list != nil {
			for _, member := range list.Nodes {
				if s.typeNodeMentionsReplacedParameter(member, subst, bindings, visiting) {
					return true
				}
			}
		}
	case ast.KindIntersectionType:
		if list := node.AsIntersectionTypeNode().Types; list != nil {
			for _, member := range list.Nodes {
				if s.typeNodeMentionsReplacedParameter(member, subst, bindings, visiting) {
					return true
				}
			}
		}
	case ast.KindTemplateLiteralType:
		if template := node.AsTemplateLiteralTypeNode(); template != nil && template.TemplateSpans != nil {
			for _, span := range template.TemplateSpans.Nodes {
				if span == nil || span.Kind != ast.KindTemplateLiteralTypeSpan {
					continue
				}
				if s.typeNodeMentionsReplacedParameter(span.AsTemplateLiteralTypeSpan().Type, subst, bindings, visiting) {
					return true
				}
			}
		}
	case ast.KindIndexedAccessType:
		indexed := node.AsIndexedAccessTypeNode()
		return s.typeNodeMentionsReplacedParameter(indexed.ObjectType, subst, bindings, visiting) ||
			s.typeNodeMentionsReplacedParameter(indexed.IndexType, subst, bindings, visiting)
	case ast.KindConditionalType:
		conditional := node.AsConditionalTypeNode()
		return s.typeNodeMentionsReplacedParameter(conditional.CheckType, subst, bindings, visiting) ||
			s.typeNodeMentionsReplacedParameter(conditional.ExtendsType, subst, bindings, visiting) ||
			s.typeNodeMentionsReplacedParameter(conditional.TrueType, subst, bindings, visiting) ||
			s.typeNodeMentionsReplacedParameter(conditional.FalseType, subst, bindings, visiting)
	case ast.KindTypeLiteral:
		for _, member := range node.Members() {
			if member != nil && s.typeNodeMentionsReplacedParameter(member, subst, bindings, visiting) {
				return true
			}
		}
	case ast.KindMappedType:
		mapped := node.AsMappedTypeNode()
		if mapped == nil {
			return false
		}
		if s.typeNodeMentionsReplacedParameter(mapped.NameType, subst, bindings, visiting) ||
			s.typeNodeMentionsReplacedParameter(mapped.Type, subst, bindings, visiting) {
			return true
		}
		if mapped.TypeParameter != nil {
			if declaration := mapped.TypeParameter.AsTypeParameterDeclaration(); declaration != nil {
				return s.typeNodeMentionsReplacedParameter(declaration.Constraint, subst, bindings, visiting)
			}
		}
	case ast.KindPropertySignature:
		return s.typeNodeMentionsReplacedParameter(node.AsPropertySignatureDeclaration().Type, subst, bindings, visiting)
	case ast.KindPropertyDeclaration:
		return s.typeNodeMentionsReplacedParameter(node.AsPropertyDeclaration().Type, subst, bindings, visiting)
	case ast.KindParameter:
		return s.typeNodeMentionsReplacedParameter(node.AsParameterDeclaration().Type, subst, bindings, visiting)
	case ast.KindIndexSignature:
		declaration := node.AsIndexSignatureDeclaration()
		if declaration == nil {
			return false
		}
		if declaration.Parameters != nil && len(declaration.Parameters.Nodes) == 1 && declaration.Parameters.Nodes[0].Kind == ast.KindParameter {
			if s.typeNodeMentionsReplacedParameter(declaration.Parameters.Nodes[0].AsParameterDeclaration().Type, subst, bindings, visiting) {
				return true
			}
		}
		return s.typeNodeMentionsReplacedParameter(declaration.Type, subst, bindings, visiting)
	}
	return false
}

// typeLiteral scans a type literal annotation. Index signatures are evaluated
// eagerly by member resolution; property and method annotations are evaluated
// only when the literal is projected into.
func (s *apiStabilitySafetyScan) typeLiteral(node *ast.Node, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	verdict := apiStabilitySafetySafe
	for _, member := range node.Members() {
		if member == nil {
			continue
		}
		verdict = combineSafety(verdict, s.memberAnnotation(member, subst, bindings, project))
	}
	return verdict
}

// mappedTypeNode scans a mapped type annotation through its eagerly evaluated
// constraint, name and template annotations.
func (s *apiStabilitySafetyScan) mappedTypeNode(node *ast.Node, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	mapped := node.AsMappedTypeNode()
	if mapped == nil {
		return apiStabilitySafetyUnknown
	}
	verdict := s.typeNode(mapped.NameType, subst, bindings, project)
	verdict = combineSafety(verdict, s.typeNode(mapped.Type, subst, bindings, project))
	if mapped.TypeParameter != nil {
		if declaration := mapped.TypeParameter.AsTypeParameterDeclaration(); declaration != nil {
			verdict = combineSafety(verdict, s.typeNode(declaration.Constraint, subst, bindings, project))
		}
	}
	return verdict
}

// memberAnnotation scans one class, interface or type literal member. Index
// signatures and declared call and construct signatures are always evaluated by
// member resolution; a named property or method is evaluated only when the
// member surface is projected into.
func (s *apiStabilitySafetyScan) memberAnnotation(member *ast.Node, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	switch member.Kind {
	case ast.KindIndexSignature:
		declaration := member.AsIndexSignatureDeclaration()
		verdict := apiStabilitySafetySafe
		if declaration.Parameters != nil && len(declaration.Parameters.Nodes) == 1 && declaration.Parameters.Nodes[0].Kind == ast.KindParameter {
			verdict = s.typeNode(declaration.Parameters.Nodes[0].AsParameterDeclaration().Type, subst, bindings, true)
		}
		return combineSafety(verdict, s.typeNode(declaration.Type, subst, bindings, true))
	case ast.KindCallSignature, ast.KindConstructSignature, ast.KindConstructor:
		if s.memberHasLateBoundName(member) {
			return apiStabilitySafetyUnknown
		}
		return s.functionLikeResolution(member, subst, bindings)
	case ast.KindPropertySignature, ast.KindPropertyDeclaration:
		if !project {
			return apiStabilitySafetySafe
		}
		if s.memberHasLateBoundName(member) {
			return apiStabilitySafetyUnknown
		}
		var annotation *ast.Node
		if member.Kind == ast.KindPropertySignature {
			annotation = member.AsPropertySignatureDeclaration().Type
		} else {
			annotation = member.AsPropertyDeclaration().Type
		}
		if annotation == nil && s.relationOperands {
			// The relation compares the member's resolved type. A property
			// without an annotation is only resolvable by inference (an
			// initializer or an implicit `any`), which the guard refuses to
			// force unless the checker already materialized the component.
			if symbol := s.memberSymbolOfDeclaration(member); symbol != nil {
				if materialized := checker.GetResolvedTypeOfSymbolIfMaterialized(s.a.tp.checker, symbol); materialized != nil {
					return s.typeValue(materialized, subst, bindings, true)
				}
			}
			return apiStabilitySafetyUnknown
		}
		return s.typeNode(annotation, subst, bindings, true)
	case ast.KindMethodSignature, ast.KindMethodDeclaration, ast.KindFunctionDeclaration:
		if s.memberHasLateBoundName(member) {
			return apiStabilitySafetyUnknown
		}
		if !project {
			return apiStabilitySafetySafe
		}
		return s.functionLikeResolution(member, subst, bindings)
	case ast.KindGetAccessor, ast.KindSetAccessor:
		if s.memberHasLateBoundName(member) {
			return apiStabilitySafetyUnknown
		}
		if !project {
			return apiStabilitySafetySafe
		}
		return s.functionLikeResolution(member, subst, bindings)
	default:
		if s.memberHasLateBoundName(member) {
			return apiStabilitySafetyUnknown
		}
		return apiStabilitySafetySafe
	}
}

// functionLikeResolution scans the parameter and type parameter constraint
// annotations a function-like declaration resolves eagerly. The return
// annotation is lazy and gated by its own read; a relation-operand scan reads
// it too, and projects parameter and return member surfaces, because the
// relation compares them structurally and would instantiate them. A relation
// operand whose inferred return the relation would have to infer from a body is
// refused.
func (s *apiStabilitySafetyScan) functionLikeResolution(node *ast.Node, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding) apiStabilitySafety {
	functionLike := node.FunctionLikeData()
	if functionLike == nil {
		return apiStabilitySafetySafe
	}
	// The relation resolves the full projected member surfaces of parameters
	// and returns while it compares them, so a relation-operand scan projects
	// into both positions exactly as the relation would.
	project := s.relationOperands
	verdict := apiStabilitySafetySafe
	for _, parameter := range functionLikeParameterNodes(functionLike) {
		if parameter == nil || parameter.Kind != ast.KindParameter {
			continue
		}
		verdict = combineSafety(verdict, s.typeNode(parameter.AsParameterDeclaration().Type, subst, bindings, project))
	}
	if functionLike.TypeParameters != nil {
		for _, typeParameter := range functionLike.TypeParameters.Nodes {
			if typeParameter == nil || typeParameter.Kind != ast.KindTypeParameter {
				continue
			}
			declaration := typeParameter.AsTypeParameterDeclaration()
			if declaration == nil {
				continue
			}
			if s.relationOperands {
				// The relation instantiates a generic signature in the context
				// of the other operand and resolves the declaration's
				// constraints and defaults while inferring. A constraint or
				// default whose raw structure mentions a replaced parameter, or
				// whose projected surface would expand a recursive alias, is
				// refused rather than evaluated.
				visiting := make(map[*ast.Node]bool)
				if s.typeNodeMentionsReplacedParameter(declaration.Constraint, subst, bindings, visiting) ||
					s.typeNodeMentionsReplacedParameter(declaration.DefaultType, subst, bindings, visiting) {
					return apiStabilitySafetyUnknown
				}
			}
			verdict = combineSafety(verdict, s.typeNode(declaration.Constraint, subst, bindings, project))
			if s.relationOperands {
				verdict = combineSafety(verdict, s.typeNode(declaration.DefaultType, subst, bindings, project))
			}
		}
	}
	if s.relationOperands {
		if functionLike.Type == nil && !ast.NodeIsMissing(node.Body()) {
			// The relation would infer the return type from the body, which the
			// guard refuses to force.
			return apiStabilitySafetyUnknown
		}
		verdict = combineSafety(verdict, s.typeNode(functionLike.Type, subst, bindings, project))
	}
	return verdict
}

// memberSymbolOfDeclaration returns the checker symbol a member declaration
// names, or nil when the declaration does not name one. It only looks up the
// declared name; no member type is resolved.
func (s *apiStabilitySafetyScan) memberSymbolOfDeclaration(member *ast.Node) *ast.Symbol {
	if member == nil {
		return nil
	}
	name := ast.GetNameOfDeclaration(member)
	if name == nil || name.Kind == ast.KindComputedPropertyName {
		return nil
	}
	return s.a.symbolAtTypeNameNode(name)
}

// memberHasLateBoundName reports whether a member's declared name must be
// evaluated to resolve the member table. A computed name whose expression is an
// entity name is late-bindable and is refused rather than checked, except for
// the well-known `Symbol.*` names whose evaluation is a pure library lookup.
func (s *apiStabilitySafetyScan) memberHasLateBoundName(member *ast.Node) bool {
	name := ast.GetNameOfDeclaration(member)
	if name == nil || name.Kind != ast.KindComputedPropertyName {
		return false
	}
	expression := name.Expression()
	if expression == nil || !ast.IsEntityNameExpression(expression) {
		return false
	}
	return !apiStabilityWellKnownSymbolName(expression)
}

// apiStabilityWellKnownSymbolName reports whether an entity-name expression is
// a `Symbol.<well-known>` reference that resolves to a library unique symbol
// without checking program code.
func apiStabilityWellKnownSymbolName(expression *ast.Node) bool {
	if expression == nil || expression.Kind != ast.KindPropertyAccessExpression {
		return false
	}
	access := expression.AsPropertyAccessExpression()
	if access == nil || access.Expression == nil || access.Name() == nil {
		return false
	}
	if access.Expression.Kind != ast.KindIdentifier || access.Expression.Text() != "Symbol" {
		return false
	}
	switch access.Name().Text() {
	case "iterator", "asyncIterator", "hasInstance", "isConcatSpreadable", "match", "matchAll",
		"replace", "search", "species", "split", "toPrimitive", "toStringTag", "unscopables":
		return true
	}
	return false
}

// apiStabilityWellKnownSymbolDisplayName renders a well-known `Symbol.<name>`
// computed-name expression for diagnostics, or "" when the expression is not a
// well-known symbol reference. It reads the expression's identifiers only and
// never evaluates it.
func apiStabilityWellKnownSymbolDisplayName(expression *ast.Node) string {
	if !apiStabilityWellKnownSymbolName(expression) {
		return ""
	}
	access := expression.AsPropertyAccessExpression()
	if access == nil || access.Name() == nil {
		return ""
	}
	return "[Symbol." + access.Name().Text() + "]"
}

// symbolHasLateBoundMembers reports whether any declaration member of a symbol
// still carries the binder's unresolved computed-name placeholder. Such a
// member is absent from the early member table and only appears once the
// checker late-binds its name, so a surface read from the early table is
// incomplete. A computed member the binder could name with a literal (or a
// literal type) is already in the early table and does not set the flag.
func (a *apiStabilityAnalysis) symbolHasLateBoundMembers(symbol *ast.Symbol) bool {
	if symbol == nil {
		return false
	}
	for _, declaration := range symbol.Declarations {
		if declaration == nil {
			continue
		}
		switch declaration.Kind {
		case ast.KindInterfaceDeclaration, ast.KindClassDeclaration, ast.KindClassExpression,
			ast.KindTypeLiteral, ast.KindEnumDeclaration:
		default:
			continue
		}
		for _, member := range declaration.Members() {
			if member == nil {
				continue
			}
			memberSymbol := member.Symbol()
			if memberSymbol != nil && memberSymbol.Name == ast.InternalSymbolNameComputed {
				return true
			}
		}
	}
	return false
}

// declarationMembers scans the member resolution of a declaration symbol: index
// signature annotations are evaluated eagerly, heritage is resolved
// recursively, and a projected read evaluates the member annotations too.
func (s *apiStabilitySafetyScan) declarationMembers(symbol *ast.Symbol, arguments *ast.NodeList, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	if symbol == nil {
		return apiStabilitySafetyUnknown
	}
	params := s.a.tp.checker.GetLocalTypeParametersOfClassOrInterfaceOrTypeAlias(symbol)
	frame := bindings
	if len(params) != 0 {
		args := make([]apiStabilitySafetyArg, len(params))
		if arguments != nil {
			for index, argument := range arguments.Nodes {
				if index >= len(params) || argument == nil {
					continue
				}
				args[index] = apiStabilitySafetyArg{node: argument, subst: subst, bindings: bindings}
			}
		}
		frame = &apiStabilitySafetyBinding{parent: bindings, params: params, args: args}
	}
	return s.declarationStructure(symbol, subst, frame, project)
}

// declarationMembersOfRepresentedArguments scans a reference target's
// declaration surface with the reference's represented arguments bound as
// concrete values. A nil argument stays an empty binding, so a parameter the
// checker has not represented cannot fall back to its constraint.
func (s *apiStabilitySafetyScan) declarationMembersOfRepresentedArguments(target *checker.Type, arguments []*checker.Type, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	if target == nil {
		return apiStabilitySafetyUnknown
	}
	symbol := target.Symbol()
	if symbol == nil {
		return apiStabilitySafetyUnknown
	}
	params := s.a.tp.checker.GetLocalTypeParametersOfClassOrInterfaceOrTypeAlias(symbol)
	frame := bindings
	if len(params) != 0 {
		args := make([]apiStabilitySafetyArg, len(params))
		for index := range params {
			if index < len(arguments) && arguments[index] != nil {
				args[index] = apiStabilitySafetyArg{t: arguments[index]}
			}
		}
		frame = &apiStabilitySafetyBinding{parent: bindings, params: params, args: args}
	}
	return s.declarationStructure(symbol, subst, frame, project)
}

// declarationStructure scans a declaration symbol's own member and heritage
// structure.
func (s *apiStabilitySafetyScan) declarationStructure(symbol *ast.Symbol, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	if symbol == nil {
		return apiStabilitySafetyUnknown
	}
	if s.relationOperands && !s.a.shouldInspectSymbol(symbol) {
		// A default-library declaration is fixed and never references a program
		// alias; the relation may compare it freely. Only program symbols
		// reached through its represented arguments can carry a recursive
		// alias.
		return apiStabilitySafetySafe
	}
	if s.activeDeclarations[symbol] {
		return apiStabilitySafetyUnknown
	}
	if s.exceeded() {
		return apiStabilitySafetyUnknown
	}
	if s.activeDeclarations == nil {
		s.activeDeclarations = make(map[*ast.Symbol]bool)
	}
	s.activeDeclarations[symbol] = true
	defer delete(s.activeDeclarations, symbol)

	verdict := apiStabilitySafetySafe
	for _, declaration := range symbol.Declarations {
		if declaration == nil {
			continue
		}
		switch declaration.Kind {
		case ast.KindInterfaceDeclaration, ast.KindClassDeclaration, ast.KindClassExpression,
			ast.KindTypeLiteral, ast.KindEnumDeclaration:
			for _, member := range declaration.Members() {
				verdict = combineSafety(verdict, s.memberAnnotation(member, subst, bindings, project))
			}
			verdict = combineSafety(verdict, s.heritage(symbol, subst, bindings, project))
		case ast.KindMappedType:
			verdict = combineSafety(verdict, s.mappedTypeNode(declaration, subst, bindings, project))
		default:
			if declaration.FunctionLikeData() != nil && s.relationOperands {
				verdict = combineSafety(verdict, s.functionLikeResolution(declaration, subst, bindings))
				continue
			}
			annotation := declarationAnnotationNodeOf(declaration)
			verdict = combineSafety(verdict, s.typeNode(annotation, subst, bindings, project))
		}
	}
	return verdict
}

// heritage scans the heritage clause type arguments a base-type resolution
// evaluates, and recursively inspects the base declarations' own member
// structure because member resolution walks the whole inheritance chain. The
// projection flag is propagated: when a projected read evaluates the derived
// member surface, inherited member annotations are evaluated with it.
func (s *apiStabilitySafetyScan) heritage(symbol *ast.Symbol, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	if symbol == nil {
		return apiStabilitySafetySafe
	}
	verdict := apiStabilitySafetySafe
	for _, declaration := range symbol.Declarations {
		if declaration == nil {
			continue
		}
		var clauses *ast.HeritageClauseList
		switch declaration.Kind {
		case ast.KindClassDeclaration, ast.KindClassExpression:
			if classLike := declaration.ClassLikeData(); classLike != nil {
				clauses = classLike.HeritageClauses
			}
		case ast.KindInterfaceDeclaration:
			clauses = declaration.AsInterfaceDeclaration().HeritageClauses
		}
		if clauses == nil {
			continue
		}
		for _, clauseNode := range clauses.Nodes {
			if clauseNode == nil {
				continue
			}
			clause := clauseNode.AsHeritageClause()
			if clause == nil || clause.Types == nil {
				continue
			}
			for _, typeNode := range clause.Types.Nodes {
				if typeNode == nil {
					continue
				}
				// An interface heritage entry is written as a type reference,
				// a class heritage entry as an expression with type arguments.
				var baseSymbol *ast.Symbol
				var arguments *ast.NodeList
				switch typeNode.Kind {
				case ast.KindTypeReference:
					reference := typeNode.AsTypeReferenceNode()
					baseSymbol = s.a.symbolAtTypeNameNode(reference.TypeName)
					arguments = reference.TypeArguments
				case ast.KindExpressionWithTypeArguments:
					expression := typeNode.AsExpressionWithTypeArguments()
					baseSymbol = s.a.symbolAtTypeNameNode(expression.Expression)
					arguments = expression.TypeArguments
				default:
					continue
				}
				if baseSymbol == nil {
					verdict = combineSafety(verdict, apiStabilitySafetyUnknown)
					continue
				}
				if arguments != nil {
					verdict = combineSafety(verdict, s.typeArguments(arguments, subst, bindings))
				}
				baseParams := s.a.tp.checker.GetLocalTypeParametersOfClassOrInterfaceOrTypeAlias(baseSymbol)
				frame := bindings
				if len(baseParams) != 0 {
					args := make([]apiStabilitySafetyArg, len(baseParams))
					if arguments != nil {
						for index, argument := range arguments.Nodes {
							if index >= len(baseParams) || argument == nil {
								continue
							}
							args[index] = apiStabilitySafetyArg{node: argument, subst: subst, bindings: bindings}
						}
					}
					frame = &apiStabilitySafetyBinding{parent: bindings, params: baseParams, args: args}
				}
				verdict = combineSafety(verdict, s.declarationStructure(baseSymbol, subst, frame, project))
			}
		}
	}
	return verdict
}

// memberTable scans a represented type that is about to have its member table
// resolved.
func (s *apiStabilitySafetyScan) memberTable(t *checker.Type, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding) apiStabilitySafety {
	if t == nil {
		return apiStabilitySafetySafe
	}
	if s.exceeded() {
		return apiStabilitySafetyUnknown
	}
	t = nonDistributedParameter(s.a.tp.checker, t)
	flags := t.Flags()
	switch {
	case flags&checker.TypeFlagsTypeParameter != 0:
		return s.constraintSafety(t, subst, bindings, true)
	case flags&checker.TypeFlagsUnionOrIntersection != 0:
		verdict := apiStabilitySafetySafe
		for _, member := range t.Types() {
			verdict = combineSafety(verdict, s.memberTable(member, subst, bindings))
		}
		return verdict
	case flags&checker.TypeFlagsObject != 0:
		if t.ObjectFlags()&checker.ObjectFlagsMapped != 0 {
			return s.mappedValue(t, subst, bindings, false)
		}
		if t.ObjectFlags()&checker.ObjectFlagsReference != 0 && t.Target() != nil && t.Target() != t {
			verdict := apiStabilitySafetySafe
			for _, argument := range checker.GetResolvedTypeArguments(s.a.tp.checker, t) {
				verdict = combineSafety(verdict, s.typeValue(argument, subst, bindings, false))
			}
			return combineSafety(verdict, s.declarationStructure(t.Target().Symbol(), subst, bindings, false))
		}
		if symbol := t.Symbol(); symbol != nil {
			return s.declarationStructure(symbol, subst, bindings, false)
		}
		return apiStabilitySafetySafe
	default:
		return apiStabilitySafetySafe
	}
}

// typeQuery scans a `typeof` annotation through the value symbol it names.
func (s *apiStabilitySafetyScan) typeQuery(node *ast.Node, subst apiStabilitySubstitution, bindings *apiStabilitySafetyBinding, project bool) apiStabilitySafety {
	query := node.AsTypeQueryNode()
	if query == nil || query.ExprName == nil {
		return apiStabilitySafetyUnknown
	}
	symbol := s.a.symbolAtTypeNameNode(query.ExprName)
	if symbol == nil {
		return apiStabilitySafetyUnknown
	}
	if materialized := checker.GetResolvedTypeOfSymbolIfMaterialized(s.a.tp.checker, symbol); materialized != nil {
		return s.typeValue(materialized, subst, bindings, project)
	}
	for _, declaration := range symbol.Declarations {
		if declaration == nil {
			continue
		}
		if functionLike := declaration.FunctionLikeData(); functionLike != nil {
			verdict := s.functionLikeResolution(declaration, subst, bindings)
			if verdict != apiStabilitySafetySafe {
				return verdict
			}
			continue
		}
		annotation := declarationAnnotationNodeOf(declaration)
		if annotation == nil {
			if s.relationOperands {
				// The relation would resolve the inferred value's type.
				return apiStabilitySafetyUnknown
			}
			continue
		}
		if verdict := s.typeNode(annotation, subst, bindings, project); verdict != apiStabilitySafetySafe {
			return verdict
		}
	}
	return apiStabilitySafetySafe
}

// nonDistributedParameter normalizes a distributed conditional's cloned check
// parameter back to its declared binder.
func nonDistributedParameter(c *checker.Checker, t *checker.Type) *checker.Type {
	if t == nil || t.Flags()&checker.TypeFlagsTypeParameter == 0 {
		return t
	}
	if normalized := checker.GetNonDistributedTypeParameter(c, t); normalized != nil && normalized != t {
		return normalized
	}
	return t
}

// safetyTypeParameterDefaults returns the declared default annotations of a
// symbol's type parameters, aligned with the symbol's local type parameter
// order.
func safetyTypeParameterDefaults(symbol *ast.Symbol) []*ast.Node {
	if symbol == nil {
		return nil
	}
	for _, declaration := range symbol.Declarations {
		if declaration == nil {
			continue
		}
		var typeParameters *ast.NodeList
		switch declaration.Kind {
		case ast.KindTypeAliasDeclaration, ast.KindJSTypeAliasDeclaration:
			typeParameters = declaration.AsTypeAliasDeclaration().TypeParameters
		case ast.KindInterfaceDeclaration:
			typeParameters = declaration.AsInterfaceDeclaration().TypeParameters
		case ast.KindClassDeclaration, ast.KindClassExpression:
			typeParameters = declaration.ClassLikeData().TypeParameters
		}
		if typeParameters == nil {
			continue
		}
		defaults := make([]*ast.Node, 0, len(typeParameters.Nodes))
		for _, typeParameter := range typeParameters.Nodes {
			if typeParameter == nil || typeParameter.Kind != ast.KindTypeParameter {
				defaults = append(defaults, nil)
				continue
			}
			if typeParameterDeclaration := typeParameter.AsTypeParameterDeclaration(); typeParameterDeclaration != nil {
				defaults = append(defaults, typeParameterDeclaration.DefaultType)
			} else {
				defaults = append(defaults, nil)
			}
		}
		return defaults
	}
	return nil
}

// functionLikeParameterNodes returns the declared parameters of a function-like
// node, or nil when it declares none.
func functionLikeParameterNodes(functionLike *ast.FunctionLikeBase) []*ast.Node {
	if functionLike == nil || functionLike.Parameters == nil {
		return nil
	}
	return functionLike.Parameters.Nodes
}

// declarationAnnotationNodeOf returns the type annotation node a declaration
// resolves, or nil for an inferred declaration.
func declarationAnnotationNodeOf(declaration *ast.Node) *ast.Node {
	if declaration == nil {
		return nil
	}
	switch declaration.Kind {
	case ast.KindVariableDeclaration:
		return declaration.AsVariableDeclaration().Type
	case ast.KindPropertyDeclaration:
		return declaration.AsPropertyDeclaration().Type
	case ast.KindPropertySignature:
		return declaration.AsPropertySignatureDeclaration().Type
	case ast.KindParameter:
		return declaration.AsParameterDeclaration().Type
	case ast.KindTypeAliasDeclaration, ast.KindJSTypeAliasDeclaration:
		return declaration.Type()
	case ast.KindGetAccessor:
		if functionLike := declaration.FunctionLikeData(); functionLike != nil {
			return functionLike.Type
		}
	case ast.KindSetAccessor:
		if functionLike := declaration.FunctionLikeData(); functionLike != nil {
			for _, parameter := range functionLikeParameterNodes(functionLike) {
				if parameter != nil && parameter.Kind == ast.KindParameter {
					return parameter.AsParameterDeclaration().Type
				}
			}
		}
	}
	return nil
}
