package typeparser

import (
	"maps"

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
			return comparePipeableSignatureSyntax(c, dataFirst, candidate, subjectIndex)
		})
	}
	return comparePipeableSignatureSyntax(c, dataFirst, candidate, subjectIndex)
}

// signatureSyntaxMatcher compares declaration syntax instead of asking the
// checker to relate or instantiate signatures. Type parameter declarations are
// paired by structural occurrence, so declaration order and binder names do not
// affect the result.
type signatureSyntaxMatcher struct {
	checker      *checker.Checker
	leftToRight  map[*ast.Symbol]*ast.Symbol
	rightToLeft  map[*ast.Symbol]*ast.Symbol
	leftBinders  map[*ast.Symbol]*ast.Node
	rightBinders map[*ast.Symbol]*ast.Node
	work         *int
	unionSearch  *int
}

func comparePipeableSignatureSyntax(c *checker.Checker, dataFirst *checker.Signature, candidate *checker.Signature, subjectIndex int) bool {
	left := dataFirst.Declaration()
	right := candidate.Declaration()
	if left == nil || right == nil || left.Type() == nil || right.Type() == nil {
		return false
	}
	leftParameters := left.Parameters()
	if subjectIndex < 0 || subjectIndex >= len(leftParameters) {
		return false
	}
	rightReturn := unwrapParenthesizedType(right.Type())
	if rightReturn == nil || rightReturn.Kind != ast.KindFunctionType {
		return false
	}
	innerParameters := rightReturn.Parameters()
	if len(innerParameters) != 1 || parameterIsRest(innerParameters[0]) {
		return false
	}

	work, unionSearch := 0, 0
	m := &signatureSyntaxMatcher{
		checker:      c,
		leftToRight:  make(map[*ast.Symbol]*ast.Symbol),
		rightToLeft:  make(map[*ast.Symbol]*ast.Symbol),
		leftBinders:  make(map[*ast.Symbol]*ast.Node),
		rightBinders: make(map[*ast.Symbol]*ast.Node),
		work:         &work,
		unionSearch:  &unionSearch,
	}
	if !m.registerBinders(left.TypeParameters(), right.TypeParameters()) {
		return false
	}
	if !m.registerOneSideBinders(rightReturn.TypeParameters(), false) {
		return false
	}
	if len(m.leftBinders) != len(m.rightBinders) {
		return false
	}

	rightParameters := right.Parameters()
	if len(rightParameters) != len(leftParameters)-1 {
		return false
	}
	rightIndex := 0
	for leftIndex, parameter := range leftParameters {
		if leftIndex == subjectIndex {
			continue
		}
		if !m.compareParameter(parameter, rightParameters[rightIndex], 0) {
			return false
		}
		rightIndex++
	}
	if !m.compareParameter(leftParameters[subjectIndex], innerParameters[0], 0) {
		return false
	}
	if !m.compareType(left.Type(), rightReturn.Type(), 0) {
		return false
	}
	return m.validateBinders()
}

func (m *signatureSyntaxMatcher) registerBinders(left []*ast.Node, right []*ast.Node) bool {
	return m.registerOneSideBinders(left, true) && m.registerOneSideBinders(right, false)
}

func (m *signatureSyntaxMatcher) registerOneSideBinders(nodes []*ast.Node, left bool) bool {
	for _, node := range nodes {
		if node == nil || node.Kind != ast.KindTypeParameter {
			return false
		}
		symbol := node.Symbol()
		if symbol == nil {
			symbol = m.checker.GetSymbolAtLocation(node.Name())
		}
		if symbol == nil {
			return false
		}
		if left {
			m.leftBinders[symbol] = node
		} else {
			m.rightBinders[symbol] = node
		}
	}
	return true
}

func (m *signatureSyntaxMatcher) compareParameter(left *ast.Node, right *ast.Node, depth int) bool {
	if left == nil || right == nil || left.Kind != ast.KindParameter || right.Kind != ast.KindParameter {
		return false
	}
	if parameterIsOptional(left) != parameterIsOptional(right) || parameterIsRest(left) != parameterIsRest(right) {
		return false
	}
	return m.compareType(left.Type(), right.Type(), depth+1)
}

func parameterIsOptional(parameter *ast.Node) bool {
	if parameter == nil || parameter.Kind != ast.KindParameter {
		return false
	}
	p := parameter.AsParameterDeclaration()
	return p.QuestionToken != nil || p.Initializer != nil
}

func parameterIsRest(parameter *ast.Node) bool {
	return parameter != nil && parameter.Kind == ast.KindParameter && parameter.AsParameterDeclaration().DotDotDotToken != nil
}

func unwrapParenthesizedType(node *ast.Node) *ast.Node {
	for node != nil && node.Kind == ast.KindParenthesizedType {
		node = node.Type()
	}
	return node
}

func (m *signatureSyntaxMatcher) compareType(left *ast.Node, right *ast.Node, depth int) bool {
	if left == nil || right == nil || depth > pipeableShapeMaxDepth {
		return false
	}
	*m.work++
	if *m.work > pipeableShapeMaxWork {
		return false
	}
	left = unwrapParenthesizedType(left)
	right = unwrapParenthesizedType(right)
	if left == nil || right == nil {
		return false
	}
	if m.isNoInferReference(left) {
		return m.compareType(left.AsTypeReferenceNode().TypeArguments.Nodes[0], right, depth+1)
	}
	if m.isNoInferReference(right) {
		return m.compareType(left, right.AsTypeReferenceNode().TypeArguments.Nodes[0], depth+1)
	}
	if left.Kind != right.Kind {
		return false
	}

	switch left.Kind {
	case ast.KindAnyKeyword, ast.KindBigIntKeyword, ast.KindBooleanKeyword, ast.KindIntrinsicKeyword,
		ast.KindNeverKeyword, ast.KindNumberKeyword, ast.KindObjectKeyword, ast.KindStringKeyword,
		ast.KindSymbolKeyword, ast.KindUndefinedKeyword, ast.KindUnknownKeyword, ast.KindVoidKeyword,
		ast.KindThisType:
		return true
	case ast.KindTypeReference:
		return m.compareTypeReference(left, right, depth+1)
	case ast.KindUnionType:
		return m.compareUnion(left.AsUnionTypeNode().Types.Nodes, right.AsUnionTypeNode().Types.Nodes, depth+1)
	case ast.KindIntersectionType:
		return m.compareTypeLists(left.AsIntersectionTypeNode().Types.Nodes, right.AsIntersectionTypeNode().Types.Nodes, depth+1)
	case ast.KindConditionalType:
		leftConditional, rightConditional := left.AsConditionalTypeNode(), right.AsConditionalTypeNode()
		return m.compareType(leftConditional.CheckType, rightConditional.CheckType, depth+1) &&
			m.compareType(leftConditional.ExtendsType, rightConditional.ExtendsType, depth+1) &&
			m.compareType(leftConditional.TrueType, rightConditional.TrueType, depth+1) &&
			m.compareType(leftConditional.FalseType, rightConditional.FalseType, depth+1)
	case ast.KindMappedType:
		return m.compareMappedType(left, right, depth+1)
	case ast.KindIndexedAccessType:
		leftIndexed, rightIndexed := left.AsIndexedAccessTypeNode(), right.AsIndexedAccessTypeNode()
		return m.compareType(leftIndexed.ObjectType, rightIndexed.ObjectType, depth+1) &&
			m.compareType(leftIndexed.IndexType, rightIndexed.IndexType, depth+1)
	case ast.KindFunctionType:
		return m.compareFunctionLike(left, right, depth+1)
	case ast.KindTypeLiteral:
		return m.compareTypeLiteral(left, right, depth+1)
	case ast.KindArrayType:
		return m.compareType(left.AsArrayTypeNode().ElementType, right.AsArrayTypeNode().ElementType, depth+1)
	case ast.KindTupleType:
		return m.compareTypeLists(left.AsTupleTypeNode().Elements.Nodes, right.AsTupleTypeNode().Elements.Nodes, depth+1)
	case ast.KindOptionalType, ast.KindRestType:
		return m.compareType(left.Type(), right.Type(), depth+1)
	case ast.KindNamedTupleMember:
		return left.QuestionToken() != nil == (right.QuestionToken() != nil) &&
			(left.AsNamedTupleMember().DotDotDotToken != nil) == (right.AsNamedTupleMember().DotDotDotToken != nil) &&
			m.compareType(left.Type(), right.Type(), depth+1)
	case ast.KindTypeOperator:
		return left.AsTypeOperatorNode().Operator == right.AsTypeOperatorNode().Operator &&
			m.compareType(left.Type(), right.Type(), depth+1)
	case ast.KindLiteralType:
		return sameLiteralType(left.AsLiteralTypeNode().Literal, right.AsLiteralTypeNode().Literal)
	default:
		return false
	}
}

func (m *signatureSyntaxMatcher) compareMappedType(left *ast.Node, right *ast.Node, depth int) bool {
	leftMapped, rightMapped := left.AsMappedTypeNode(), right.AsMappedTypeNode()
	if !sameOptionalTokenKind(leftMapped.ReadonlyToken, rightMapped.ReadonlyToken) ||
		!sameOptionalTokenKind(leftMapped.QuestionToken, rightMapped.QuestionToken) ||
		!m.registerBinders([]*ast.Node{leftMapped.TypeParameter}, []*ast.Node{rightMapped.TypeParameter}) {
		return false
	}
	if !m.compareOptionalType(leftMapped.NameType, rightMapped.NameType, depth+1) ||
		!m.compareOptionalType(leftMapped.Type, rightMapped.Type, depth+1) {
		return false
	}
	leftMembers, rightMembers := typeListNodes(leftMapped.Members), typeListNodes(rightMapped.Members)
	return m.compareTypeLists(leftMembers, rightMembers, depth+1)
}

func (m *signatureSyntaxMatcher) compareOptionalType(left *ast.Node, right *ast.Node, depth int) bool {
	if left == nil || right == nil {
		return left == right
	}
	return m.compareType(left, right, depth+1)
}

func sameOptionalTokenKind(left *ast.Node, right *ast.Node) bool {
	if left == nil || right == nil {
		return left == right
	}
	return left.Kind == right.Kind
}

func typeListNodes(list *ast.NodeList) []*ast.Node {
	if list == nil {
		return nil
	}
	return list.Nodes
}

func (m *signatureSyntaxMatcher) compareTypeReference(left *ast.Node, right *ast.Node, depth int) bool {
	leftRef, rightRef := left.AsTypeReferenceNode(), right.AsTypeReferenceNode()
	leftSymbol := m.checker.GetSymbolAtLocation(leftRef.TypeName)
	rightSymbol := m.checker.GetSymbolAtLocation(rightRef.TypeName)
	if leftBinder, ok := m.leftBinders[leftSymbol]; ok && leftBinder != nil {
		if _, ok := m.rightBinders[rightSymbol]; !ok || !m.bind(leftSymbol, rightSymbol) {
			return false
		}
	} else if _, rightIsBinder := m.rightBinders[rightSymbol]; rightIsBinder || !sameSymbolReference(m.checker, leftSymbol, rightSymbol) {
		return false
	}
	return m.compareTypeLists(typeArgumentNodes(leftRef.TypeArguments), typeArgumentNodes(rightRef.TypeArguments), depth+1)
}

func typeArgumentNodes(list *ast.TypeList) []*ast.Node {
	if list == nil {
		return nil
	}
	return list.Nodes
}

func (m *signatureSyntaxMatcher) bind(left *ast.Symbol, right *ast.Symbol) bool {
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

func (m *signatureSyntaxMatcher) compareTypeLists(left []*ast.Node, right []*ast.Node, depth int) bool {
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

func (m *signatureSyntaxMatcher) compareFunctionLike(left *ast.Node, right *ast.Node, depth int) bool {
	if len(left.TypeParameters()) != len(right.TypeParameters()) || !m.registerBinders(left.TypeParameters(), right.TypeParameters()) {
		return false
	}
	leftParameters, rightParameters := left.Parameters(), right.Parameters()
	if len(leftParameters) != len(rightParameters) {
		return false
	}
	for i := range leftParameters {
		if !m.compareParameter(leftParameters[i], rightParameters[i], depth+1) {
			return false
		}
	}
	return m.compareType(left.Type(), right.Type(), depth+1)
}

func (m *signatureSyntaxMatcher) compareTypeLiteral(left *ast.Node, right *ast.Node, depth int) bool {
	leftMembers, rightMembers := left.Members(), right.Members()
	if len(leftMembers) != len(rightMembers) {
		return false
	}
	rightByName := make(map[string]*ast.Node, len(rightMembers))
	for _, member := range rightMembers {
		name, ok := typeMemberKey(member)
		if !ok || rightByName[name] != nil {
			return false
		}
		rightByName[name] = member
	}
	for _, leftMember := range leftMembers {
		name, ok := typeMemberKey(leftMember)
		if !ok {
			return false
		}
		rightMember := rightByName[name]
		if rightMember == nil || !m.compareTypeMember(leftMember, rightMember, depth+1) {
			return false
		}
	}
	return true
}

func typeMemberKey(member *ast.Node) (string, bool) {
	if member == nil {
		return "", false
	}
	if member.Kind == ast.KindCallSignature {
		return "\xfe.call", true
	}
	name := member.Name()
	if name == nil {
		return "", false
	}
	switch name.Kind {
	case ast.KindIdentifier, ast.KindPrivateIdentifier, ast.KindStringLiteral, ast.KindNumericLiteral:
		return name.Text(), true
	default:
		return "", false
	}
}

func (m *signatureSyntaxMatcher) compareTypeMember(left *ast.Node, right *ast.Node, depth int) bool {
	if left.Kind != right.Kind || left.QuestionToken() != nil != (right.QuestionToken() != nil) {
		return false
	}
	const relevantModifiers = ast.ModifierFlagsReadonly
	if left.ModifierFlags()&relevantModifiers != right.ModifierFlags()&relevantModifiers {
		return false
	}
	switch left.Kind {
	case ast.KindPropertySignature:
		return m.compareType(left.Type(), right.Type(), depth+1)
	case ast.KindMethodSignature, ast.KindCallSignature:
		return m.compareFunctionLike(left, right, depth+1)
	default:
		return false
	}
}

func (m *signatureSyntaxMatcher) compareUnion(left []*ast.Node, right []*ast.Node, depth int) bool {
	if len(left) != len(right) {
		return false
	}
	return m.compareUnionAt(left, right, make([]bool, len(right)), 0, depth+1)
}

func (m *signatureSyntaxMatcher) compareUnionAt(left []*ast.Node, right []*ast.Node, used []bool, index int, depth int) bool {
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

func (m *signatureSyntaxMatcher) clone() *signatureSyntaxMatcher {
	clone := *m
	clone.leftToRight = cloneSymbolMap(m.leftToRight)
	clone.rightToLeft = cloneSymbolMap(m.rightToLeft)
	clone.leftBinders = cloneNodeMap(m.leftBinders)
	clone.rightBinders = cloneNodeMap(m.rightBinders)
	return &clone
}

func (m *signatureSyntaxMatcher) commit(branch *signatureSyntaxMatcher) {
	m.leftToRight = branch.leftToRight
	m.rightToLeft = branch.rightToLeft
	m.leftBinders = branch.leftBinders
	m.rightBinders = branch.rightBinders
}

func cloneSymbolMap(source map[*ast.Symbol]*ast.Symbol) map[*ast.Symbol]*ast.Symbol {
	result := make(map[*ast.Symbol]*ast.Symbol, len(source))
	maps.Copy(result, source)
	return result
}

func cloneNodeMap(source map[*ast.Symbol]*ast.Node) map[*ast.Symbol]*ast.Node {
	result := make(map[*ast.Symbol]*ast.Node, len(source))
	maps.Copy(result, source)
	return result
}

func (m *signatureSyntaxMatcher) validateBinders() bool {
	if len(m.leftBinders) != len(m.rightBinders) || len(m.leftToRight) != len(m.leftBinders) {
		return false
	}
	for leftSymbol, leftDeclaration := range m.leftBinders {
		rightSymbol := m.leftToRight[leftSymbol]
		rightDeclaration := m.rightBinders[rightSymbol]
		if rightDeclaration == nil || !m.compareTypeParameterDeclaration(leftDeclaration, rightDeclaration) {
			return false
		}
	}
	return true
}

func (m *signatureSyntaxMatcher) compareTypeParameterDeclaration(left *ast.Node, right *ast.Node) bool {
	leftParameter, rightParameter := left.AsTypeParameterDeclaration(), right.AsTypeParameterDeclaration()
	const relevantModifiers = ast.ModifierFlagsConst | ast.ModifierFlagsIn | ast.ModifierFlagsOut
	if left.ModifierFlags()&relevantModifiers != right.ModifierFlags()&relevantModifiers {
		return false
	}
	if (leftParameter.Constraint == nil) != (rightParameter.Constraint == nil) ||
		(leftParameter.DefaultType == nil) != (rightParameter.DefaultType == nil) {
		return false
	}
	if leftParameter.Constraint != nil && !m.compareType(leftParameter.Constraint, rightParameter.Constraint, 0) {
		return false
	}
	return leftParameter.DefaultType == nil || m.compareType(leftParameter.DefaultType, rightParameter.DefaultType, 0)
}

func (m *signatureSyntaxMatcher) isNoInferReference(node *ast.Node) bool {
	if node == nil || node.Kind != ast.KindTypeReference {
		return false
	}
	reference := node.AsTypeReferenceNode()
	if reference.TypeName == nil || reference.TypeArguments == nil || len(reference.TypeArguments.Nodes) != 1 {
		return false
	}
	var hasNoInferName bool
	switch reference.TypeName.Kind {
	case ast.KindIdentifier:
		hasNoInferName = reference.TypeName.Text() == "NoInfer"
	case ast.KindQualifiedName:
		hasNoInferName = reference.TypeName.AsQualifiedName().Right.Text() == "NoInfer"
	default:
		return false
	}
	if !hasNoInferName {
		return false
	}
	symbol := m.checker.GetSymbolAtLocation(reference.TypeName)
	if symbol == nil {
		return false
	}
	if symbol.Flags&ast.SymbolFlagsAlias != 0 {
		symbol = checker.SkipAlias(symbol, m.checker)
	}
	if symbol == nil {
		return false
	}
	for _, declaration := range symbol.Declarations {
		if isNoInferDeclaration(m.checker, declaration) {
			return true
		}
	}
	return false
}

func isNoInferDeclaration(c *checker.Checker, declaration *ast.Node) bool {
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
		isTypeParameterReference(c, indexed.ObjectType.AsTupleTypeNode().Elements.Nodes[0], binder) &&
		isTypeParameterReference(c, conditional.CheckType, binder) &&
		conditional.ExtendsType != nil && conditional.ExtendsType.Kind == ast.KindAnyKeyword &&
		isNumericLiteralType(conditional.TrueType, "0") &&
		conditional.FalseType != nil && conditional.FalseType.Kind == ast.KindNeverKeyword
}

func isTypeParameterReference(c *checker.Checker, node *ast.Node, binder *ast.Symbol) bool {
	if node == nil || node.Kind != ast.KindTypeReference {
		return false
	}
	reference := node.AsTypeReferenceNode()
	return reference.TypeArguments == nil && sameSymbolReference(c, c.GetSymbolAtLocation(reference.TypeName), binder)
}

func isNumericLiteralType(node *ast.Node, value string) bool {
	return node != nil && node.Kind == ast.KindLiteralType &&
		node.AsLiteralTypeNode().Literal != nil && node.AsLiteralTypeNode().Literal.Kind == ast.KindNumericLiteral &&
		node.AsLiteralTypeNode().Literal.Text() == value
}

func sameLiteralType(left *ast.Node, right *ast.Node) bool {
	if left == nil || right == nil || left.Kind != right.Kind {
		return false
	}
	switch left.Kind {
	case ast.KindStringLiteral, ast.KindNumericLiteral, ast.KindBigIntLiteral, ast.KindNoSubstitutionTemplateLiteral:
		return left.Text() == right.Text()
	case ast.KindTrueKeyword, ast.KindFalseKeyword, ast.KindNullKeyword:
		return true
	default:
		return false
	}
}
