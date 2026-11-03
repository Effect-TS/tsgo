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
// reaches the import and not a local that shadows it. Bindings that are
// type-only, directly or through a type-only re-export, are not values.
func importBindingName(ctx *fixable.Context, location *ast.Node, symbol *ast.Symbol) *ast.Node {
	if symbol == nil || len(ast.TSGetSymbolDeclarations(symbol)) != 1 || ctx.Checker.GetTypeOnlyAliasDeclaration(symbol) != nil {
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
	if declaration == nil && ast.TSGetSymbolFlags(property)&ast.SymbolFlagsAlias != 0 && len(ast.TSGetSymbolDeclarations(property)) == 1 {
		// Aliased exports such as `export { undefined_ as undefined }` are declared by their export specifier.
		declaration = ast.TSGetSymbolDeclarations(property)[0]
	}
	return declaration != nil && declaration.Name() != nil && member.isMember(declaration.Name())
}

// expression builds the reference: module.member, or the direct binding.
func (r *moduleMemberReference) expression(tracker *rewriter.Tracker, member moduleMember) *ast.Node {
	if r.viaModule {
		return tracker.NewPropertyAccessPath(r.path.Append(member.name)...)
	}
	return tracker.NewPropertyAccessPath(r.path...)
}

// calleeModuleMemberReference keeps the qualifier a callee is written with:
// Q.f gives Q.<member>, and a named import of f gives a bare <member> from the
// same import. When the bare name has to be imported, namedImports is the list
// to add it to. Otherwise it falls back to module, the file's reference to the
// member's module.
func calleeModuleMemberReference(ctx *fixable.Context, callee *ast.Node, module typeparser.ModuleReference, member moduleMember) (reference *moduleMemberReference, namedImports *ast.Node) {
	if qualifier := calleeModuleQualifier(callee, member); qualifier != nil {
		if reference := importedModuleMember(ctx, callee, qualifier, member); reference != nil {
			return reference, nil
		}
	}
	if reference, namedImports := namedImportModuleMember(ctx, callee, member); reference != nil {
		return reference, namedImports
	}
	return resolveModuleMember(ctx, callee, module, member), nil
}

// calleeModuleQualifier returns the module path of a Q.f callee whose Q is
// member's module.
func calleeModuleQualifier(callee *ast.Node, member moduleMember) typeparser.ModuleReference {
	callee = ast.SkipParentheses(callee)
	if !ast.IsPropertyAccessExpression(callee) {
		return nil
	}
	qualifier := callee.Expression()
	if !member.isModule(qualifier) {
		return nil
	}
	return identifierPath(qualifier)
}

// identifierPath returns the names of an identifier or a chain of property
// accesses on identifiers, or nil for any other expression.
func identifierPath(node *ast.Node) typeparser.ModuleReference {
	switch {
	case ast.IsIdentifier(node):
		return typeparser.ModuleReference{node.Text()}
	case ast.IsPropertyAccessExpression(node) && ast.IsIdentifier(node.Name()):
		if path := identifierPath(node.Expression()); path != nil {
			return path.Append(node.Name().Text())
		}
	}
	return nil
}

// namedImportModuleMember handles a callee that is a named import binding of
// member's module: it reuses a bare member binding already in scope, or returns
// the bare member name with the callee's NamedImports to add it to when the
// name is free and the imported module exports it.
func namedImportModuleMember(ctx *fixable.Context, callee *ast.Node, member moduleMember) (*moduleMemberReference, *ast.Node) {
	callee = ast.SkipParentheses(callee)
	if !ast.IsIdentifier(callee) {
		return nil, nil
	}
	binding := importBindingName(ctx, callee, ctx.Checker.ResolveName(callee.Text(), callee, ast.SymbolFlagsValue|ast.SymbolFlagsAlias, false))
	if binding == nil || binding.Parent == nil || !ast.IsImportSpecifier(binding.Parent) {
		return nil, nil
	}
	namedImports := binding.Parent.Parent
	if namedImports == nil || !ast.IsNamedImports(namedImports) || namedImports.Parent == nil || namedImports.Parent.Parent == nil ||
		!ast.IsImportDeclaration(namedImports.Parent.Parent) {
		return nil, nil
	}
	moduleSymbol := ctx.TypeParser.GetSymbolAtLocation(namedImports.Parent.Parent.AsImportDeclaration().ModuleSpecifier)
	if moduleSymbol == nil {
		return nil, nil
	}
	export := ctx.Checker.TryGetMemberInModuleExports(member.name, moduleSymbol)
	if export == nil {
		return nil, nil
	}
	if ast.TSGetSymbolFlags(export)&ast.SymbolFlagsAlias != 0 {
		export = ctx.Checker.GetAliasedSymbol(export)
	}
	exportDeclaration := ast.TSGetSymbolValueDeclaration(export)
	if exportDeclaration == nil || exportDeclaration.Name() == nil || !member.isMember(exportDeclaration.Name()) {
		return nil, nil
	}

	existing := ctx.Checker.ResolveName(member.name, callee, ast.SymbolFlagsValue|ast.SymbolFlagsAlias, false)
	if existing == nil {
		return &moduleMemberReference{path: typeparser.ModuleReference{member.name}}, namedImports
	}
	if name := importBindingName(ctx, callee, existing); name != nil && member.isMember(name) {
		return &moduleMemberReference{path: typeparser.ModuleReference{name.Text()}}, nil
	}
	return nil, nil
}
