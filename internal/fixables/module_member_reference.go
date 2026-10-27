package fixables

import (
	"slices"
	"strings"

	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// moduleMember describes a module export that a fix wants to reference.
type moduleMember struct {
	name     string
	isModule func(node *ast.Node) bool // node references the module namespace
	isMember func(node *ast.Node) bool // node references the export itself
}

// moduleMemberReference is a path of names, visible at a location, that reaches a moduleMember.
type moduleMemberReference struct {
	path      typeparser.ModuleReference
	viaModule bool // path is the module namespace, so the member is accessed on it
}

// resolveModuleMember finds a reference that resolves at location to member:
// preferred, the file's import of the module, when it reaches member,
// otherwise an import binding in scope. It returns nil when nothing reaches
// the export, so the fix never introduces an unbound, shadowed or
// uninitialized name.
//
// Local aliases are never used: a declaration that precedes location in the
// text may still be uninitialized when location runs (a sibling switch clause,
// or a hoisted function called before the declaration). Import bindings are
// initialized before any module code runs.
func resolveModuleMember(ctx *fixable.Context, location *ast.Node, preferred typeparser.ModuleReference, member moduleMember) *moduleMemberReference {
	if reference := importedModuleMember(ctx, location, preferred, member); reference != nil {
		return reference
	}
	symbols := ctx.Checker.GetSymbolsInScope(location, ast.SymbolFlagsValue|ast.SymbolFlagsAlias)
	slices.SortFunc(symbols, func(left, right *ast.Symbol) int {
		return strings.Compare(ast.TSGetSymbolName(left), ast.TSGetSymbolName(right))
	})
	for _, symbol := range symbols {
		name := importBindingName(ctx, location, symbol)
		if name == nil {
			continue
		}
		if member.isMember(name) {
			return &moduleMemberReference{path: typeparser.ModuleReference{name.Text()}}
		}
		if member.isModule(name) && typeHasMember(ctx, ctx.TypeParser.GetTypeAtLocation(name), member) {
			return &moduleMemberReference{path: typeparser.ModuleReference{name.Text()}, viaModule: true}
		}
	}
	return nil
}

// importedModuleMember returns module as a reference to member when its first
// name resolves at location to an import binding and the value reached by
// following the rest of the path has member, or nil otherwise.
func importedModuleMember(ctx *fixable.Context, location *ast.Node, module typeparser.ModuleReference, member moduleMember) *moduleMemberReference {
	if len(module) == 0 {
		return nil
	}
	name := importBindingName(ctx, location, ctx.Checker.ResolveName(module[0], location, ast.SymbolFlagsValue|ast.SymbolFlagsAlias, false))
	if name == nil {
		return nil
	}
	t := ctx.TypeParser.GetTypeAtLocation(name)
	for _, property := range module[1:] {
		symbol := ctx.Checker.GetPropertyOfType(t, property)
		if symbol == nil {
			return nil
		}
		t = ctx.Checker.GetTypeOfSymbol(symbol)
	}
	if !typeHasMember(ctx, t, member) {
		return nil
	}
	return &moduleMemberReference{path: module, viaModule: true}
}

// importBindingName returns the name of symbol's value import declaration
// when that name resolves to symbol at location, so a generated identifier
// reaches the import and not a local that shadows it.
func importBindingName(ctx *fixable.Context, location *ast.Node, symbol *ast.Symbol) *ast.Node {
	if symbol == nil || len(ast.TSGetSymbolDeclarations(symbol)) != 1 {
		return nil
	}
	declaration := ast.TSGetSymbolDeclarations(symbol)[0]
	if declaration.Kind != ast.KindImportSpecifier && declaration.Kind != ast.KindNamespaceImport ||
		ast.IsPartOfTypeOnlyImportOrExportDeclaration(declaration) {
		return nil
	}
	name := declaration.Name()
	if name == nil || name.Kind != ast.KindIdentifier ||
		ctx.Checker.ResolveName(name.Text(), location, ast.SymbolFlagsValue|ast.SymbolFlagsAlias, false) != symbol {
		return nil
	}
	return name
}

func typeHasMember(ctx *fixable.Context, t *checker.Type, member moduleMember) bool {
	if t == nil {
		return false
	}
	property := ctx.Checker.GetPropertyOfType(t, member.name)
	if property == nil {
		return false
	}
	declaration := ast.TSGetSymbolValueDeclaration(property)
	return declaration != nil && declaration.Name() != nil && member.isMember(declaration.Name())
}

// expression builds the reference: module.member, or the direct binding.
func (r *moduleMemberReference) expression(tracker *rewriter.Tracker, member moduleMember) *ast.Node {
	if r.viaModule {
		return tracker.NewPropertyAccessPath(r.path.Append(member.name)...)
	}
	return tracker.NewPropertyAccessPath(r.path...)
}
