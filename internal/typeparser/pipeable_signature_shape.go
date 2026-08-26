package typeparser

import (
	"maps"
	"reflect"
	"slices"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

const (
	pipeableShapeMaxDepth       = 64
	pipeableShapeMaxWork        = 4096
	pipeableShapeMaxUnionSearch = 128
)

type pipeableSignatureShapeCacheKey struct {
	dataFirst    *checker.Signature
	candidate    *checker.Signature
	subjectIndex int
}

type pipeableTypePair struct {
	left  *checker.Type
	right *checker.Type
}

func rawSignature(signature *checker.Signature) *checker.Signature {
	for signature != nil && signature.Target() != nil {
		signature = signature.Target()
	}
	return signature
}

func pipeableSignatureShapesMatch(c *checker.Checker, dataFirst *checker.Signature, candidate *checker.Signature, subjectIndex int) bool {
	dataFirst = rawSignature(dataFirst)
	candidate = rawSignature(candidate)
	if c == nil || dataFirst == nil || candidate == nil {
		return false
	}
	key := pipeableSignatureShapeCacheKey{dataFirst: dataFirst, candidate: candidate, subjectIndex: subjectIndex}
	if links, ok := c.EffectLinks.(*EffectLinks); ok && links != nil {
		return Cached(&links.PipeableSignatureShape, key, func() bool {
			return comparePipeableSignatureTypes(c, dataFirst, candidate, subjectIndex)
		})
	}
	return comparePipeableSignatureTypes(c, dataFirst, candidate, subjectIndex)
}

// signatureTypeMatcher zips existing, uninstantiated compiler type graphs. It
// never asks the checker to relate two signatures and never creates a mapper.
// Generic binders are paired by structural occurrence rather than declaration
// order, with maps in both directions to preserve binder identity.
type signatureTypeMatcher struct {
	checker      *checker.Checker
	leftToRight  map[*checker.Type]*checker.Type
	rightToLeft  map[*checker.Type]*checker.Type
	leftBinders  map[*checker.Type]struct{}
	rightBinders map[*checker.Type]struct{}
	active       map[pipeableTypePair]struct{}
	work         *int
	unionSearch  *int
}

func comparePipeableSignatureTypes(c *checker.Checker, dataFirst *checker.Signature, candidate *checker.Signature, subjectIndex int) bool {
	if c == nil || dataFirst == nil || candidate == nil {
		return false
	}
	leftParameters := dataFirst.Parameters()
	if subjectIndex < 0 || subjectIndex >= len(leftParameters) {
		return false
	}

	candidateReturn := c.GetReturnTypeOfSignature(candidate)
	if candidateReturn == nil {
		return false
	}
	returnedSignatures := c.GetSignaturesOfType(candidateReturn, checker.SignatureKindCall)
	if len(returnedSignatures) != 1 || returnedSignatures[0] == nil || returnedSignatures[0].Target() != nil {
		// A target here means that discovering the callable shape instantiated a
		// generic alias. Dropping its mapper would compare the wrong raw graph.
		return false
	}
	returned := returnedSignatures[0]
	if len(returned.Parameters()) != 1 || returned.HasRestParameter() {
		return false
	}

	work, unionSearch := 0, 0
	m := &signatureTypeMatcher{
		checker:      c,
		leftToRight:  make(map[*checker.Type]*checker.Type),
		rightToLeft:  make(map[*checker.Type]*checker.Type),
		leftBinders:  make(map[*checker.Type]struct{}),
		rightBinders: make(map[*checker.Type]struct{}),
		active:       make(map[pipeableTypePair]struct{}),
		work:         &work,
		unionSearch:  &unionSearch,
	}
	m.registerBinders(dataFirst.TypeParameters(), true)
	m.registerBinders(candidate.TypeParameters(), false)
	m.registerBinders(returned.TypeParameters(), false)
	if len(m.leftBinders) != len(m.rightBinders) {
		return false
	}

	rightParameters := candidate.Parameters()
	if len(rightParameters) != len(leftParameters)-1 {
		return false
	}
	rightIndex := 0
	for leftIndex, leftParameter := range leftParameters {
		if leftIndex == subjectIndex {
			continue
		}
		if !m.compareParameter(
			leftParameter,
			leftIndex == len(leftParameters)-1 && dataFirst.HasRestParameter(),
			rightParameters[rightIndex],
			rightIndex == len(rightParameters)-1 && candidate.HasRestParameter(),
			0,
		) {
			return false
		}
		rightIndex++
	}
	if !m.compareParameter(leftParameters[subjectIndex], false, returned.Parameters()[0], false, 0) {
		return false
	}
	if !m.compareType(c.GetReturnTypeOfSignature(dataFirst), c.GetReturnTypeOfSignature(returned), 0) {
		return false
	}
	return m.validateBinders()
}

func (m *signatureTypeMatcher) registerBinders(types []*checker.Type, left bool) {
	for _, t := range types {
		if t == nil || t.Flags()&checker.TypeFlagsTypeParameter == 0 {
			continue
		}
		if left {
			m.leftBinders[t] = struct{}{}
		} else {
			m.rightBinders[t] = struct{}{}
		}
	}
}

func (m *signatureTypeMatcher) compareParameter(left *ast.Symbol, leftRest bool, right *ast.Symbol, rightRest bool, depth int) bool {
	if left == nil || right == nil || leftRest != rightRest || symbolIsOptional(left) != symbolIsOptional(right) {
		return false
	}
	return m.compareType(m.checker.GetTypeOfSymbol(left), m.checker.GetTypeOfSymbol(right), depth+1)
}

func symbolIsOptional(symbol *ast.Symbol) bool {
	return symbol != nil && symbol.Flags&ast.SymbolFlagsOptional != 0
}

func (m *signatureTypeMatcher) compareType(left *checker.Type, right *checker.Type, depth int) bool {
	if left == right {
		return left != nil
	}
	if left == nil || right == nil || depth > pipeableShapeMaxDepth {
		return false
	}
	*m.work++
	if *m.work > pipeableShapeMaxWork {
		return false
	}

	if base := noInferBaseType(m.checker, left); base != nil {
		return m.compareType(base, right, depth+1)
	}
	if base := noInferBaseType(m.checker, right); base != nil {
		return m.compareType(left, base, depth+1)
	}

	pair := pipeableTypePair{left: left, right: right}
	if _, ok := m.active[pair]; ok {
		return true
	}
	m.active[pair] = struct{}{}
	defer delete(m.active, pair)

	leftAlias, rightAlias := left.Alias(), right.Alias()
	if leftAlias != nil || rightAlias != nil {
		return leftAlias != nil && rightAlias != nil &&
			sameSymbolReference(m.checker, leftAlias.Symbol(), rightAlias.Symbol()) &&
			m.compareTypeList(leftAlias.TypeArguments(), rightAlias.TypeArguments(), depth+1)
	}

	leftIsBinder := left.Flags()&checker.TypeFlagsTypeParameter != 0
	rightIsBinder := right.Flags()&checker.TypeFlagsTypeParameter != 0
	if leftIsBinder || rightIsBinder {
		return leftIsBinder && rightIsBinder && m.compareTypeParameter(left, right)
	}
	if left.Flags() != right.Flags() {
		return false
	}

	switch {
	case left.Flags()&checker.TypeFlagsUnion != 0:
		return m.compareUnion(left.Types(), right.Types(), depth+1)
	case left.Flags()&checker.TypeFlagsIntersection != 0:
		// Preserve compiler order for intersections; unlike unions, their order
		// can affect overloaded callable behavior.
		return m.compareTypeList(left.Types(), right.Types(), depth+1)
	case left.Flags()&checker.TypeFlagsConditional != 0:
		leftConditional, rightConditional := left.AsConditionalType(), right.AsConditionalType()
		return m.compareType(leftConditional.CheckType(), rightConditional.CheckType(), depth+1) &&
			m.compareType(leftConditional.ExtendsType(), rightConditional.ExtendsType(), depth+1) &&
			m.compareType(m.checker.GetTrueTypeOfConditionalType(left), m.checker.GetTrueTypeOfConditionalType(right), depth+1) &&
			m.compareType(m.checker.GetFalseTypeOfConditionalType(left), m.checker.GetFalseTypeOfConditionalType(right), depth+1)
	case left.Flags()&checker.TypeFlagsIndexedAccess != 0:
		leftIndexed, rightIndexed := left.AsIndexedAccessType(), right.AsIndexedAccessType()
		return m.compareType(leftIndexed.ObjectType(), rightIndexed.ObjectType(), depth+1) &&
			m.compareType(leftIndexed.IndexType(), rightIndexed.IndexType(), depth+1)
	case left.Flags()&checker.TypeFlagsIndex != 0:
		return m.compareType(left.AsIndexType().Target(), right.AsIndexType().Target(), depth+1)
	case left.Flags()&checker.TypeFlagsTemplateLiteral != 0:
		return slices.Equal(left.AsTemplateLiteralType().Texts(), right.AsTemplateLiteralType().Texts()) &&
			m.compareTypeList(left.AsTemplateLiteralType().Types(), right.AsTemplateLiteralType().Types(), depth+1)
	case left.Flags()&checker.TypeFlagsStringMapping != 0:
		return sameSymbolReference(m.checker, left.Symbol(), right.Symbol()) &&
			m.compareType(left.AsStringMappingType().Target(), right.AsStringMappingType().Target(), depth+1)
	case left.Flags()&checker.TypeFlagsSubstitution != 0:
		leftSubstitution, rightSubstitution := left.AsSubstitutionType(), right.AsSubstitutionType()
		return m.compareType(leftSubstitution.BaseType(), rightSubstitution.BaseType(), depth+1) &&
			m.compareType(leftSubstitution.SubstConstraint(), rightSubstitution.SubstConstraint(), depth+1)
	case left.Flags()&checker.TypeFlagsObject != 0:
		return m.compareObject(left, right, depth+1)
	case left.Flags()&checker.TypeFlagsLiteral != 0:
		return reflect.DeepEqual(left.AsLiteralType().Value(), right.AsLiteralType().Value()) &&
			sameOptionalSymbol(m.checker, left.Symbol(), right.Symbol())
	case left.Flags()&checker.TypeFlagsUniqueESSymbol != 0,
		left.Flags()&checker.TypeFlagsEnum != 0:
		return sameSymbolReference(m.checker, left.Symbol(), right.Symbol())
	case left.Flags()&checker.TypeFlagsSingleton != 0:
		return true
	default:
		return false
	}
}

func (m *signatureTypeMatcher) compareTypeParameter(left *checker.Type, right *checker.Type) bool {
	_, leftLocal := m.leftBinders[left]
	_, rightLocal := m.rightBinders[right]
	if leftLocal || rightLocal {
		if !leftLocal || !rightLocal {
			return false
		}
		if mapped, ok := m.leftToRight[left]; ok {
			return mapped == right
		}
		if mapped, ok := m.rightToLeft[right]; ok {
			return mapped == left
		}
		m.leftToRight[left] = right
		m.rightToLeft[right] = left
		return true
	}
	return left == right || sameSymbolReference(m.checker, left.Symbol(), right.Symbol())
}

func (m *signatureTypeMatcher) compareObject(left *checker.Type, right *checker.Type, depth int) bool {
	leftFlags, rightFlags := left.ObjectFlags(), right.ObjectFlags()
	leftMapped := leftFlags&checker.ObjectFlagsMapped != 0
	rightMapped := rightFlags&checker.ObjectFlagsMapped != 0
	if leftMapped || rightMapped {
		return leftMapped && rightMapped && m.compareMappedType(left, right, depth+1)
	}
	if leftFlags&checker.ObjectFlagsReverseMapped != 0 || rightFlags&checker.ObjectFlagsReverseMapped != 0 ||
		leftFlags&checker.ObjectFlagsEvolvingArray != 0 || rightFlags&checker.ObjectFlagsEvolvingArray != 0 ||
		leftFlags&checker.ObjectFlagsInstantiationExpressionType != 0 || rightFlags&checker.ObjectFlagsInstantiationExpressionType != 0 {
		return false
	}

	leftReference := leftFlags&checker.ObjectFlagsReference != 0
	rightReference := rightFlags&checker.ObjectFlagsReference != 0
	if leftReference || rightReference {
		if !leftReference || !rightReference {
			return false
		}
		leftTarget, rightTarget := left.Target(), right.Target()
		return (leftTarget == rightTarget || sameSymbolReference(m.checker, leftTarget.Symbol(), rightTarget.Symbol())) &&
			m.compareTypeList(m.checker.GetTypeArguments(left), m.checker.GetTypeArguments(right), depth+1)
	}

	leftAnonymous := leftFlags&checker.ObjectFlagsAnonymous != 0
	rightAnonymous := rightFlags&checker.ObjectFlagsAnonymous != 0
	if !leftAnonymous || !rightAnonymous {
		return !leftAnonymous && !rightAnonymous && sameSymbolReference(m.checker, left.Symbol(), right.Symbol())
	}
	return m.compareAnonymousObject(left, right, depth+1)
}

func (m *signatureTypeMatcher) compareMappedType(left *checker.Type, right *checker.Type, depth int) bool {
	leftBinder := checker.Checker_getTypeParameterFromMappedType(m.checker, left)
	rightBinder := checker.Checker_getTypeParameterFromMappedType(m.checker, right)
	if leftBinder == nil || rightBinder == nil || checker.GetMappedTypeModifiers(left) != checker.GetMappedTypeModifiers(right) {
		return false
	}
	m.registerBinders([]*checker.Type{leftBinder}, true)
	m.registerBinders([]*checker.Type{rightBinder}, false)
	return m.compareType(leftBinder, rightBinder, depth+1) &&
		m.compareType(
			checker.Checker_getConstraintTypeFromMappedType(m.checker, left),
			checker.Checker_getConstraintTypeFromMappedType(m.checker, right),
			depth+1,
		) &&
		m.compareOptionalTypeAt(
			checker.Checker_getNameTypeFromMappedType(m.checker, left),
			checker.Checker_getNameTypeFromMappedType(m.checker, right),
			depth+1,
		) &&
		m.compareType(
			checker.Checker_getTemplateTypeFromMappedType(m.checker, left),
			checker.Checker_getTemplateTypeFromMappedType(m.checker, right),
			depth+1,
		)
}

func (m *signatureTypeMatcher) compareAnonymousObject(left *checker.Type, right *checker.Type, depth int) bool {
	leftProperties := m.checker.GetPropertiesOfType(left)
	rightProperties := m.checker.GetPropertiesOfType(right)
	if len(leftProperties) != len(rightProperties) {
		return false
	}
	rightByName := make(map[string]*ast.Symbol, len(rightProperties))
	for _, property := range rightProperties {
		if property == nil || rightByName[property.Name] != nil {
			return false
		}
		rightByName[property.Name] = property
	}
	for _, leftProperty := range leftProperties {
		if leftProperty == nil {
			return false
		}
		rightProperty := rightByName[leftProperty.Name]
		if rightProperty == nil || symbolIsOptional(leftProperty) != symbolIsOptional(rightProperty) ||
			checker.Checker_isReadonlySymbol(m.checker, leftProperty) != checker.Checker_isReadonlySymbol(m.checker, rightProperty) ||
			!m.compareType(m.checker.GetTypeOfSymbol(leftProperty), m.checker.GetTypeOfSymbol(rightProperty), depth+1) {
			return false
		}
	}

	leftIndexes, rightIndexes := m.checker.GetIndexInfosOfType(left), m.checker.GetIndexInfosOfType(right)
	if len(leftIndexes) != len(rightIndexes) {
		return false
	}
	for i := range leftIndexes {
		if leftIndexes[i].IsReadonly() != rightIndexes[i].IsReadonly() ||
			!m.compareType(leftIndexes[i].KeyType(), rightIndexes[i].KeyType(), depth+1) ||
			!m.compareType(leftIndexes[i].ValueType(), rightIndexes[i].ValueType(), depth+1) {
			return false
		}
	}

	if len(m.checker.GetSignaturesOfType(left, checker.SignatureKindConstruct)) != 0 ||
		len(m.checker.GetSignaturesOfType(right, checker.SignatureKindConstruct)) != 0 {
		return false
	}
	leftCalls := m.checker.GetSignaturesOfType(left, checker.SignatureKindCall)
	rightCalls := m.checker.GetSignaturesOfType(right, checker.SignatureKindCall)
	if len(leftCalls) != len(rightCalls) {
		return false
	}
	for i := range leftCalls {
		if leftCalls[i].Target() != nil || rightCalls[i].Target() != nil || !m.compareSignature(leftCalls[i], rightCalls[i], depth+1) {
			return false
		}
	}
	return true
}

func (m *signatureTypeMatcher) compareSignature(left *checker.Signature, right *checker.Signature, depth int) bool {
	if left == nil || right == nil || left.HasRestParameter() != right.HasRestParameter() ||
		left.MinArgumentCount() != right.MinArgumentCount() ||
		(m.checker.GetTypePredicateOfSignature(left) != nil || m.checker.GetTypePredicateOfSignature(right) != nil) {
		return false
	}
	m.registerBinders(left.TypeParameters(), true)
	m.registerBinders(right.TypeParameters(), false)
	leftParameters, rightParameters := left.Parameters(), right.Parameters()
	if len(leftParameters) != len(rightParameters) {
		return false
	}
	if (left.ThisParameter() == nil) != (right.ThisParameter() == nil) {
		return false
	}
	if left.ThisParameter() != nil && !m.compareParameter(left.ThisParameter(), false, right.ThisParameter(), false, depth+1) {
		return false
	}
	for i := range leftParameters {
		if !m.compareParameter(
			leftParameters[i], i == len(leftParameters)-1 && left.HasRestParameter(),
			rightParameters[i], i == len(rightParameters)-1 && right.HasRestParameter(),
			depth+1,
		) {
			return false
		}
	}
	return m.compareType(m.checker.GetReturnTypeOfSignature(left), m.checker.GetReturnTypeOfSignature(right), depth+1)
}

func (m *signatureTypeMatcher) compareTypeList(left []*checker.Type, right []*checker.Type, depth int) bool {
	if len(left) != len(right) {
		return false
	}
	for i := range left {
		if !m.compareType(left[i], right[i], depth+1) {
			return false
		}
	}
	return true
}

func (m *signatureTypeMatcher) compareUnion(left []*checker.Type, right []*checker.Type, depth int) bool {
	if len(left) != len(right) {
		return false
	}
	return m.compareUnionAt(left, right, make([]bool, len(right)), 0, depth+1)
}

func (m *signatureTypeMatcher) compareUnionAt(left []*checker.Type, right []*checker.Type, used []bool, index int, depth int) bool {
	if index == len(left) {
		return true
	}
	for rightIndex := range right {
		if used[rightIndex] {
			continue
		}
		*m.unionSearch++
		if *m.unionSearch > pipeableShapeMaxUnionSearch {
			return false
		}
		branch := m.clone()
		if !branch.compareType(left[index], right[rightIndex], depth+1) {
			continue
		}
		branchUsed := append([]bool(nil), used...)
		branchUsed[rightIndex] = true
		if branch.compareUnionAt(left, right, branchUsed, index+1, depth+1) {
			m.commit(branch)
			return true
		}
	}
	return false
}

func (m *signatureTypeMatcher) clone() *signatureTypeMatcher {
	clone := *m
	clone.leftToRight = maps.Clone(m.leftToRight)
	clone.rightToLeft = maps.Clone(m.rightToLeft)
	clone.leftBinders = maps.Clone(m.leftBinders)
	clone.rightBinders = maps.Clone(m.rightBinders)
	clone.active = maps.Clone(m.active)
	return &clone
}

func (m *signatureTypeMatcher) commit(branch *signatureTypeMatcher) {
	m.leftToRight = branch.leftToRight
	m.rightToLeft = branch.rightToLeft
	m.leftBinders = branch.leftBinders
	m.rightBinders = branch.rightBinders
}

func (m *signatureTypeMatcher) validateBinders() bool {
	if len(m.leftBinders) != len(m.rightBinders) || len(m.leftToRight) != len(m.leftBinders) {
		return false
	}
	for left, right := range m.leftToRight {
		if checker.Checker_getTypeParameterModifiers(m.checker, left) != checker.Checker_getTypeParameterModifiers(m.checker, right) {
			return false
		}
		leftConstraint, rightConstraint := m.checker.GetConstraintOfTypeParameter(left), m.checker.GetConstraintOfTypeParameter(right)
		if !m.compareOptionalType(leftConstraint, rightConstraint) {
			return false
		}
		leftDefault, rightDefault := m.checker.GetDefaultFromTypeParameter(left), m.checker.GetDefaultFromTypeParameter(right)
		if !m.compareOptionalType(leftDefault, rightDefault) {
			return false
		}
	}
	return true
}

func (m *signatureTypeMatcher) compareOptionalType(left *checker.Type, right *checker.Type) bool {
	return m.compareOptionalTypeAt(left, right, 0)
}

func (m *signatureTypeMatcher) compareOptionalTypeAt(left *checker.Type, right *checker.Type, depth int) bool {
	if left == nil || right == nil {
		return left == right
	}
	return m.compareType(left, right, depth+1)
}

func sameOptionalSymbol(c *checker.Checker, left *ast.Symbol, right *ast.Symbol) bool {
	if left == nil || right == nil {
		return left == right
	}
	return sameSymbolReference(c, left, right)
}

func noInferBaseType(c *checker.Checker, t *checker.Type) *checker.Type {
	if c == nil || t == nil {
		return nil
	}
	if checker.Checker_isNoInferType(c, t) {
		return t.AsSubstitutionType().BaseType()
	}
	alias := t.Alias()
	if alias == nil || len(alias.TypeArguments()) != 1 || alias.Symbol() == nil {
		return nil
	}
	for _, declaration := range alias.Symbol().Declarations {
		if isNoInferAliasDeclaration(c, declaration) {
			return alias.TypeArguments()[0]
		}
	}
	return nil
}

// Effect supported NoInfer before it became a TypeScript intrinsic with the
// standard tuple/indexed-access encoding. Recognize only those two exact
// declarations so an unrelated alias named NoInfer is not made transparent.
func isNoInferAliasDeclaration(c *checker.Checker, declaration *ast.Node) bool {
	if declaration == nil || declaration.Kind != ast.KindTypeAliasDeclaration || len(declaration.TypeParameters()) != 1 {
		return false
	}
	body := declaration.Type()
	if body == nil {
		return false
	}
	if body.Kind == ast.KindIntrinsicKeyword {
		return true
	}
	if body.Kind != ast.KindIndexedAccessType {
		return false
	}
	indexed := body.AsIndexedAccessTypeNode()
	if indexed.ObjectType == nil || indexed.ObjectType.Kind != ast.KindTupleType ||
		len(indexed.ObjectType.AsTupleTypeNode().Elements.Nodes) != 1 ||
		indexed.IndexType == nil || indexed.IndexType.Kind != ast.KindConditionalType {
		return false
	}
	binder := declaration.TypeParameters()[0].Symbol()
	if binder == nil {
		binder = c.GetSymbolAtLocation(declaration.TypeParameters()[0].Name())
	}
	conditional := indexed.IndexType.AsConditionalTypeNode()
	return binder != nil &&
		isTypeParameterTypeNode(c, indexed.ObjectType.AsTupleTypeNode().Elements.Nodes[0], binder) &&
		isTypeParameterTypeNode(c, conditional.CheckType, binder) &&
		conditional.ExtendsType != nil && conditional.ExtendsType.Kind == ast.KindAnyKeyword &&
		isNumericLiteralTypeNode(conditional.TrueType, "0") &&
		conditional.FalseType != nil && conditional.FalseType.Kind == ast.KindNeverKeyword
}

func isTypeParameterTypeNode(c *checker.Checker, node *ast.Node, binder *ast.Symbol) bool {
	if node == nil || node.Kind != ast.KindTypeReference {
		return false
	}
	reference := node.AsTypeReferenceNode()
	return reference.TypeArguments == nil && sameSymbolReference(c, c.GetSymbolAtLocation(reference.TypeName), binder)
}

func isNumericLiteralTypeNode(node *ast.Node, value string) bool {
	return node != nil && node.Kind == ast.KindLiteralType &&
		node.AsLiteralTypeNode().Literal != nil && node.AsLiteralTypeNode().Literal.Kind == ast.KindNumericLiteral &&
		node.AsLiteralTypeNode().Literal.Text() == value
}
