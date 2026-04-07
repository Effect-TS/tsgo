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

	_, candidates := checker.GetResolvedSignatureForSignatureHelp(node, len(call.Arguments.Nodes)-1, c)
	for _, subjectIndex := range subjectIndexes {
		derived := derivePipeableSignatureFromDataFirst(c, resolved, subjectIndex)
		if derived == nil {
			continue
		}

		for _, candidate := range candidates {
			if candidate == nil || candidate.Declaration() == nil {
				continue
			}
			candidateSymbol := checker.Checker_getSymbolOfDeclaration(c, candidate.Declaration())
			if candidateSymbol == nil || checker.Checker_getSymbolIfSameReference(c, resolvedSymbol, candidateSymbol) == nil {
				continue
			}
			candidateReturn := c.GetReturnTypeOfSignature(candidate)
			if candidateReturn == nil {
				continue
			}
			returnedSigs := c.GetSignaturesOfType(candidateReturn, checker.SignatureKindCall)
			hasUnaryReturnedCall := false
			for _, returnedSig := range returnedSigs {
				if returnedSig != nil && len(returnedSig.Parameters()) == 1 {
					hasUnaryReturnedCall = true
					break
				}
			}
			if !hasUnaryReturnedCall {
				continue
			}
			if !checker.Checker_isSignatureAssignableTo(c, candidate, derived, false) {
				continue
			}

			return &ParsedDataFirstOrLastCall{
				Node:         call,
				Callee:       call.Expression,
				Subject:      call.Arguments.Nodes[subjectIndex],
				Args:         omitArgAt(call.Arguments.Nodes, subjectIndex),
				SubjectIndex: subjectIndex,
			}
		}
	}

	return nil
}

func derivePipeableSignatureFromDataFirst(c *checker.Checker, sig *checker.Signature, subjectIndex int) *checker.Signature {
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
