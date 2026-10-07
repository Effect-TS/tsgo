package typeparser

import (
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// optionalTaggedBase reads declaration metadata only. Every public member,
// including inherited and merged members, must be an optional property or
// method with its own recognized stability tag. Internal members are ignored.
// Call, construct and index signatures cannot be optional. Unknown bases fail
// closed; member types are
// never evaluated merely to decide whether to exempt an inherited base.
func (a *apiStabilityAnalysis) optionalTaggedBase(symbol *ast.Symbol) bool {
	if a.optionalTaggedSymbols[symbol] {
		return true
	}
	pending := []*ast.Symbol{symbol}
	seen := map[*ast.Symbol]bool{}
	for len(pending) > 0 {
		current := pending[len(pending)-1]
		pending = pending[:len(pending)-1]
		if current == nil || current.Flags&(ast.SymbolFlagsInterface|ast.SymbolFlagsClass) == 0 || len(current.Declarations) == 0 || !a.consumeWork() {
			return false
		}
		if seen[current] {
			continue
		}
		seen[current] = true
		if current.Flags&ast.SymbolFlagsClass != 0 && apiStabilitySymbolDeclaresHeritage(current) {
			declared := checker.GetResolvedDeclaredTypeOfSymbolIfMaterialized(a.tp.checker, current)
			if declared == nil {
				return false
			}
			if _, resolved := checker.GetResolvedBaseTypesOfTypeIfMaterialized(a.tp.checker, declared); !resolved {
				return false
			}
		}
		for _, declaration := range current.Declarations {
			var members []*ast.Node
			switch declaration.Kind {
			case ast.KindInterfaceDeclaration:
				if list := declaration.AsInterfaceDeclaration().Members; list != nil {
					members = list.Nodes
				}
			case ast.KindClassDeclaration, ast.KindClassExpression:
				if list := declaration.ClassLikeData().Members; list != nil {
					members = list.Nodes
				}
			default:
				return false
			}
			for _, member := range members {
				if apiStabilitySymbolHasOnlyInternalDeclarations(checker.Checker_getSymbolOfDeclaration(a.tp.checker, member)) {
					continue
				}
				if (member.Kind != ast.KindPropertySignature && member.Kind != ast.KindMethodSignature &&
					member.Kind != ast.KindPropertyDeclaration && member.Kind != ast.KindMethodDeclaration) ||
					!ast.HasQuestionToken(member) || stabilityOfDeclarationTag(member) == "" {
					return false
				}
			}
		}
		for _, declaration := range current.Declarations {
			if declaration.Kind != ast.KindInterfaceDeclaration {
				continue
			}
			for _, node := range ast.GetExtendsHeritageClauseElements(declaration) {
				base := a.symbolAtTypeNameNode(node.AsTypeReferenceNode().TypeName)
				aliases := map[*ast.Symbol]bool{}
				for base != nil && base.Flags&ast.SymbolFlagsAlias != 0 && !aliases[base] {
					aliases[base] = true
					base = ApiStabilityImmediateAliasedSymbol(a.tp.checker, base)
				}
				pending = append(pending, base)
			}
		}
		if current.Flags&ast.SymbolFlagsClass != 0 && apiStabilitySymbolDeclaresHeritage(current) {
			declared := checker.GetResolvedDeclaredTypeOfSymbolIfMaterialized(a.tp.checker, current)
			bases, _ := checker.GetResolvedBaseTypesOfTypeIfMaterialized(a.tp.checker, declared)
			for _, base := range bases {
				if base == nil || base.Symbol() == nil {
					return false
				}
				pending = append(pending, base.Symbol())
			}
		}
	}
	if a.optionalTaggedSymbols == nil {
		a.optionalTaggedSymbols = make(map[*ast.Symbol]bool)
	}
	a.optionalTaggedSymbols[symbol] = true
	return true
}

// optionalTaggedInheritedMember applies the base exception to properties the
// compiler flattened into a derived member table, including superclass factory
// intersection types. Own members are always inspected independently.
func (a *apiStabilityAnalysis) optionalTaggedInheritedMember(member, owner *ast.Symbol) bool {
	declaring := a.memberDeclaringSymbol(member)
	return declaring != nil && owner != nil && declaring != owner && a.optionalTaggedBase(declaring)
}
