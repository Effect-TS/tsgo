package fixables

import (
	"slices"
	"strings"

	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

// moduleMember describes a module export that a fix wants to reference.
type moduleMember struct {
	name     string
	isModule func(node *ast.Node) bool // node references the module namespace
	isMember func(node *ast.Node) bool // node references the export itself
}

// moduleMemberReference is a binding, visible at a location, that reaches a moduleMember.
type moduleMemberReference struct {
	node      *ast.Node
	viaModule bool // node is the module namespace, so the member is accessed on it
}

// resolveModuleMember finds a binding that resolves at location to member: the
// receiver when it is the module, otherwise an import binding in scope, trying
// preferred first. It returns nil when nothing reaches the export, so the fix
// never introduces an unbound, shadowed or uninitialized name.
//
// Local aliases are never used: a declaration that precedes location in the
// text may still be uninitialized when location runs (a sibling switch clause,
// or a hoisted function called before the declaration). Import bindings are
// initialized before any module code runs.
func resolveModuleMember(ctx *fixable.Context, location *ast.Node, receiver *ast.Node, preferred string, member moduleMember) *moduleMemberReference {
	if receiver != nil && member.isModule(receiver) && moduleHasMember(ctx, receiver, member) {
		return &moduleMemberReference{node: receiver, viaModule: true}
	}
	symbols := ctx.Checker.GetSymbolsInScope(location, ast.SymbolFlagsValue|ast.SymbolFlagsAlias)
	slices.SortFunc(symbols, func(left, right *ast.Symbol) int {
		leftName, rightName := ast.TSGetSymbolName(left), ast.TSGetSymbolName(right)
		if (leftName == preferred) != (rightName == preferred) {
			if leftName == preferred {
				return -1
			}
			return 1
		}
		return strings.Compare(leftName, rightName)
	})
	for _, symbol := range symbols {
		if len(ast.TSGetSymbolDeclarations(symbol)) != 1 {
			continue
		}
		declaration := ast.TSGetSymbolDeclarations(symbol)[0]
		if declaration.Kind != ast.KindImportSpecifier && declaration.Kind != ast.KindNamespaceImport ||
			ast.IsPartOfTypeOnlyImportOrExportDeclaration(declaration) {
			continue
		}
		name := declaration.Name()
		// The generated identifier must resolve to this import at location, not
		// to a local that shadows it.
		if name == nil || name.Kind != ast.KindIdentifier ||
			ctx.Checker.ResolveName(name.Text(), location, ast.SymbolFlagsValue|ast.SymbolFlagsAlias, false) != symbol {
			continue
		}
		if member.isMember(name) {
			return &moduleMemberReference{node: name}
		}
		if member.isModule(name) && moduleHasMember(ctx, name, member) {
			return &moduleMemberReference{node: name, viaModule: true}
		}
	}
	return nil
}

func moduleHasMember(ctx *fixable.Context, receiver *ast.Node, member moduleMember) bool {
	property := ctx.Checker.GetPropertyOfType(ctx.TypeParser.GetTypeAtLocation(receiver), member.name)
	if property == nil {
		return false
	}
	declaration := ast.TSGetSymbolValueDeclaration(property)
	return declaration != nil && declaration.Name() != nil && member.isMember(declaration.Name())
}

// expression builds the reference: module.member, or the direct binding.
func (r *moduleMemberReference) expression(tracker *rewriter.Tracker, member moduleMember) *ast.Node {
	if !r.viaModule {
		return tracker.NewIdentifier(r.node.Text())
	}
	return tracker.NewPropertyAccessExpression(tracker.DeepCloneNode(r.node), nil, tracker.NewIdentifier(member.name), ast.NodeFlagsNone)
}
