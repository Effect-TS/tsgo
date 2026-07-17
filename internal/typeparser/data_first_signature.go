package typeparser

import (
	"strings"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
)

type ParsedDataFirstOrLastCall struct {
	Node         *ast.CallExpression
	Callee       *ast.Node
	Subject      *ast.Node
	Args         []*ast.Node
	SubjectIndex int
}

type PipeableSignatureWitness struct {
	ArgumentTypes []*checker.Type
	SubjectType   *checker.Type
}

func (tp *TypeParser) DataFirstOrLastCall(node *ast.Node) *ParsedDataFirstOrLastCall {
	if tp == nil || tp.checker == nil || node == nil || node.Kind != ast.KindCallExpression {
		return nil
	}
	call := node.AsCallExpression()
	if call == nil || call.Expression == nil || call.Arguments == nil || len(call.Arguments.Nodes) < 2 {
		return nil
	}

	for _, arg := range call.Arguments.Nodes {
		if arg == nil || arg.Kind == ast.KindSpreadElement {
			return nil
		}
	}

	c := tp.checker
	resolved := c.GetResolvedSignature(node)
	if resolved == nil || resolved.Declaration() == nil {
		return nil
	}
	if len(resolved.Parameters()) != len(call.Arguments.Nodes) {
		return nil
	}

	resolvedSymbol := checker.Checker_getSymbolOfDeclaration(c, resolved.Declaration())
	if resolvedSymbol == nil {
		return nil
	}
	calleeType := tp.GetTypeAtLocation(call.Expression)
	if calleeType == nil {
		return nil
	}
	candidates := c.GetSignaturesOfType(calleeType, checker.SignatureKindCall)

	subjectIndexes := []int{0}
	if len(call.Arguments.Nodes) == 2 {
		last := len(call.Arguments.Nodes) - 1
		preferFirst := false
		if params := resolved.Parameters(); len(params) > 0 {
			preferFirst = isLikelySelfParameter(params[0])
		}
		if preferFirst {
			subjectIndexes = []int{0, last}
		} else {
			subjectIndexes = []int{last, 0}
		}
	}

	for _, subjectIndex := range subjectIndexes {
		subjectType := tp.GetTypeAtLocation(call.Arguments.Nodes[subjectIndex])
		if subjectType == nil {
			continue
		}
		args := omitArgAt(call.Arguments.Nodes, subjectIndex)
		argumentTypes := make([]*checker.Type, 0, len(args))
		for _, arg := range args {
			argumentTypes = append(argumentTypes, tp.GetTypeAtLocation(arg))
		}
		witness := &PipeableSignatureWitness{ArgumentTypes: argumentTypes, SubjectType: subjectType}

		for _, candidate := range candidates {
			if candidate == nil || candidate.Declaration() == nil {
				continue
			}
			candidateSymbol := checker.Checker_getSymbolOfDeclaration(c, candidate.Declaration())
			if candidateSymbol == nil || checker.Checker_getSymbolIfSameReference(c, resolvedSymbol, candidateSymbol) == nil {
				continue
			}
			if !MatchesPipeableSignature(c, resolved, candidate, subjectIndex, witness) {
				continue
			}

			return &ParsedDataFirstOrLastCall{
				Node:         call,
				Callee:       call.Expression,
				Subject:      call.Arguments.Nodes[subjectIndex],
				Args:         args,
				SubjectIndex: subjectIndex,
			}
		}
	}

	return nil
}

// MatchesPipeableSignature reports whether candidate is the pipeable form of
// dataFirst with the parameter at subjectIndex moved into a returned unary function.
// When witness is nil, parameter types from dataFirst are used for comparison.
func MatchesPipeableSignature(c *checker.Checker, dataFirst *checker.Signature, candidate *checker.Signature, subjectIndex int, witness *PipeableSignatureWitness) bool {
	if c == nil || dataFirst == nil || candidate == nil {
		return false
	}
	params := dataFirst.Parameters()
	if subjectIndex < 0 || subjectIndex >= len(params) {
		return false
	}

	derived := DerivePipeableSignatureFromDataFirst(c, dataFirst, subjectIndex)
	if derived == nil {
		return false
	}

	var argumentTypes []*checker.Type
	var subjectType *checker.Type
	if witness != nil {
		argumentTypes = witness.ArgumentTypes
		subjectType = witness.SubjectType
	} else {
		argumentTypes = make([]*checker.Type, 0, len(params)-1)
		for i, param := range params {
			if i == subjectIndex {
				subjectType = c.GetTypeOfSymbol(param)
				continue
			}
			argumentTypes = append(argumentTypes, c.GetTypeOfSymbol(param))
		}
	}

	return argumentTypesMatchParameters(c, argumentTypes, candidate) && hasMatchingUnaryReturn(c, candidate, derived, subjectType)
}

func argumentTypesMatchParameters(c *checker.Checker, args []*checker.Type, sig *checker.Signature) bool {
	if c == nil || sig == nil || len(args) < sig.MinArgumentCount() {
		return false
	}
	params := sig.Parameters()
	if len(params) == 0 {
		return len(args) == 0
	}
	if len(args) > len(params) && !sig.HasRestParameter() {
		return false
	}
	for i, argType := range args {
		paramIndex := i
		if paramIndex >= len(params) {
			paramIndex = len(params) - 1
		}
		paramType := c.GetTypeOfSymbol(params[paramIndex])
		if !typeAcceptsArgument(c, argType, paramType) && (len(sig.TypeParameters()) == 0 || !hasCompatibleCallShape(c, argType, paramType)) {
			return false
		}
	}
	return true
}

func hasMatchingUnaryReturn(c *checker.Checker, candidate *checker.Signature, derived *checker.Signature, subjectType *checker.Type) bool {
	if c == nil || candidate == nil || derived == nil || subjectType == nil {
		return false
	}
	candidateReturn := c.GetReturnTypeOfSignature(candidate)
	derivedReturn := c.GetReturnTypeOfSignature(derived)
	if candidateReturn == nil || derivedReturn == nil {
		return false
	}
	derivedSigs := c.GetSignaturesOfType(derivedReturn, checker.SignatureKindCall)
	for _, candidateSig := range c.GetSignaturesOfType(candidateReturn, checker.SignatureKindCall) {
		if candidateSig == nil || len(candidateSig.Parameters()) != 1 {
			continue
		}
		for _, derivedSig := range derivedSigs {
			if derivedSig == nil || len(derivedSig.Parameters()) != 1 {
				continue
			}
			candidateParamType := c.GetTypeOfSymbol(candidateSig.Parameters()[0])
			if !typeAcceptsArgument(c, subjectType, candidateParamType) && ((len(candidate.TypeParameters()) == 0 && len(candidateSig.TypeParameters()) == 0) || !hasCompatibleCallShape(c, subjectType, candidateParamType)) {
				continue
			}
			if !sameShallowTypeOrigin(c, c.GetReturnTypeOfSignature(candidateSig), c.GetReturnTypeOfSignature(derivedSig)) {
				continue
			}
			return true
		}
	}
	return false
}

func hasCompatibleCallShape(c *checker.Checker, left *checker.Type, right *checker.Type) bool {
	if c == nil || left == nil || right == nil {
		return false
	}
	leftCallable := len(c.GetSignaturesOfType(left, checker.SignatureKindCall)) > 0
	rightCallable := len(c.GetSignaturesOfType(right, checker.SignatureKindCall)) > 0
	return leftCallable == rightCallable
}

func typeAcceptsArgument(c *checker.Checker, argument *checker.Type, parameter *checker.Type) bool {
	if c == nil || argument == nil || parameter == nil {
		return false
	}
	if parameter.Flags()&checker.TypeFlagsTypeParameter != 0 {
		constraint := c.GetConstraintOfTypeParameter(parameter)
		return constraint == nil || typeAcceptsArgument(c, argument, constraint)
	}
	return sameShallowTypeOrigin(c, argument, parameter) || checker.Checker_isTypeAssignableTo(c, argument, parameter)
}

func sameShallowTypeOrigin(c *checker.Checker, left *checker.Type, right *checker.Type) bool {
	if left == right {
		return true
	}
	if c == nil || left == nil || right == nil {
		return false
	}

	if left.ObjectFlags()&checker.ObjectFlagsReference != 0 || right.ObjectFlags()&checker.ObjectFlagsReference != 0 {
		if left.ObjectFlags()&checker.ObjectFlagsReference == 0 || right.ObjectFlags()&checker.ObjectFlagsReference == 0 {
			return false
		}
		leftTarget, rightTarget := left.Target(), right.Target()
		return leftTarget == rightTarget || sameSymbolReference(c, leftTarget.Symbol(), rightTarget.Symbol())
	}

	leftAlias, rightAlias := left.Alias(), right.Alias()
	if leftAlias != nil || rightAlias != nil {
		return leftAlias != nil && rightAlias != nil && sameSymbolReference(c, leftAlias.Symbol(), rightAlias.Symbol())
	}

	if left.Symbol() != nil || right.Symbol() != nil {
		return sameSymbolReference(c, left.Symbol(), right.Symbol())
	}

	return left.Flags() == right.Flags() && left.Flags()&checker.TypeFlagsSingleton != 0
}

func sameSymbolReference(c *checker.Checker, left *ast.Symbol, right *ast.Symbol) bool {
	return left != nil && right != nil && checker.Checker_getSymbolIfSameReference(c, left, right) != nil
}

// DerivePipeableSignatureFromDataFirst removes the subject parameter from a
// data-first signature and moves it to a unary function in the return type.
func DerivePipeableSignatureFromDataFirst(c *checker.Checker, sig *checker.Signature, subjectIndex int) *checker.Signature {
	if c == nil || sig == nil {
		return nil
	}
	params := sig.Parameters()
	if subjectIndex < 0 || subjectIndex >= len(params) {
		return nil
	}
	subject := params[subjectIndex]
	if subject == nil {
		return nil
	}

	outerParams := make([]*ast.Symbol, 0, len(params)-1)
	for i, param := range params {
		if i == subjectIndex {
			continue
		}
		outerParams = append(outerParams, param)
	}

	innerFnType := checker.Checker_newFunctionType(c, nil, nil, []*ast.Symbol{subject}, c.GetReturnTypeOfSignature(sig))
	if innerFnType == nil {
		return nil
	}
	return checker.Checker_newCallSignature(c, sig.TypeParameters(), sig.ThisParameter(), outerParams, innerFnType)
}

func isLikelySelfParameter(sym *ast.Symbol) bool {
	if sym == nil {
		return false
	}
	name := strings.ToLower(sym.Name)
	return name == "self" || strings.HasPrefix(name, "self") || name == "this"
}

func omitArgAt(nodes []*ast.Node, index int) []*ast.Node {
	result := make([]*ast.Node, 0, len(nodes)-1)
	for i, node := range nodes {
		if i == index {
			continue
		}
		result = append(result, node)
	}
	return result
}
