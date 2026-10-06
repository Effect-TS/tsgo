package rules

import (
	"slices"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
)

// ApiStabilityLeak reports exported APIs whose public surface exposes a type
// that is marked with a lower stability than the export itself. A stable export
// may not expose unstable or experimental types, and an unstable export may not
// expose experimental types. Experimental exports impose no restriction.
var ApiStabilityLeak = rule.Rule{
	Name:            "apiStabilityLeak",
	Group:           "maintainers",
	Description:     "Reports exported APIs whose public surface exposes a less stable type",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v4"},
	Codes: []int32{
		tsdiag.X_0_exposes_1_an_2_API_effect_apiStabilityLeak.Code(),
	},
	Run: checkApiStabilityLeaks,
}

// checkApiStabilityLeaks compares each export's declared stability ceiling with
// the actual stability of its public surface. Every export is inspected in its
// own ApiStabilitySession, so the export's type and signatures share one
// traversal and unpublishable results stay local to it. Declared stability
// lookups persist per checker, and a complete, settled, context-free concrete
// type or signature surface is reused by later exports, files and TypeParsers
// over the same checker. The ceiling and the diagnostic location are per
// export.
func checkApiStabilityLeaks(ctx *rule.Context) []*ast.Diagnostic {
	moduleSymbol := checker.Checker_getSymbolOfDeclaration(ctx.Checker, ctx.SourceFile.AsNode())
	if moduleSymbol == nil {
		return nil
	}

	var diagnostics []*ast.Diagnostic
	for _, exportSymbol := range sortedApiStabilityExports(ctx, moduleSymbol) {
		if exportSymbol == nil {
			continue
		}
		if apiStabilityExportIsInternal(ctx, moduleSymbol, exportSymbol) {
			continue
		}
		ceiling := apiStabilityCeiling(ctx, exportSymbol)
		if ceiling >= typeparser.ApiStabilityExperimental {
			continue
		}
		target := resolveStabilityAlias(ctx.Checker, exportSymbol)
		if target == nil {
			continue
		}
		location := apiStabilityExportLocation(ctx, exportSymbol)
		if location == nil {
			continue
		}

		session := ctx.TypeParser.NewApiStabilitySession()
		walker := newApiStabilityWalker(ctx, session, exportSymbol, target, location, ceiling)
		walker.inspectExport()
		diagnostics = append(diagnostics, walker.diagnostics...)
	}
	return diagnostics
}

// sortedApiStabilityExports returns a module's exports in a deterministic
// order. The checker returns them in Go map order; each export is analyzed
// independently from its own session, so visit order only decides the order in
// which bounded per-export analyses consume their own budgets. Sorting by name
// (the unique export-table key) makes the reported findings reproducible across
// runs and processes. Diagnostics are sorted later, so this only removes
// analysis-order nondeterminism.
func sortedApiStabilityExports(ctx *rule.Context, module *ast.Symbol) []*ast.Symbol {
	exports := ctx.Checker.GetExportsOfModule(module)
	slices.SortStableFunc(exports, func(left, right *ast.Symbol) int {
		if left == nil || right == nil {
			if left == right {
				return 0
			}
			if left == nil {
				return 1
			}
			return -1
		}
		if left.Name < right.Name {
			return -1
		}
		if left.Name > right.Name {
			return 1
		}
		return 0
	})
	return exports
}

// apiStabilityCeiling returns the stability ceiling that governs an export. A
// forwarding tag written on a re-export declaration in the selected file takes
// precedence over the forwarded symbol's own stability, including when it is
// stricter; presence is therefore returned separately from the declared level.
// The tag is source-specific, so it is read here rather than through the
// per-checker symbol cache that only sees the forwarded symbol's declarations.
func apiStabilityCeiling(ctx *rule.Context, exportSymbol *ast.Symbol) typeparser.ApiStabilityLevel {
	ceiling := ctx.TypeParser.DeclaredApiStability(exportSymbol)
	if forwarded, present := apiStabilityForwardingStability(ctx, exportSymbol); present {
		ceiling = forwarded
	}
	return ceiling
}

// apiStabilityWalker walks one export's public surface through the export's
// analysis-local ApiStabilitySession and reports every dependency whose
// declared stability exceeds the export's ceiling. Namespace re-exports are
// enumerated through their module symbol so type-only exports are not lost. The
// walker holds no type-graph state: cycle safety and memoization belong to the
// session; when the export is inspected, its unpublishable results are
// discarded and its complete, context-free surfaces are reused through the
// per-checker stores.
type apiStabilityWalker struct {
	ctx          *rule.Context
	session      *typeparser.ApiStabilitySession
	ceiling      typeparser.ApiStabilityLevel
	exportSymbol *ast.Symbol
	exportTarget *ast.Symbol
	location     *ast.Node

	// inspectedModules breaks namespace re-export cycles by resolved module
	// identity. offenders is keyed by offender declaration, symbol or signature,
	// so the same type is reported once per export regardless of how many paths
	// reach it.
	inspectedModules map[*ast.Symbol]bool
	offenders        map[apiStabilityOffenderKey]bool

	diagnostics []*ast.Diagnostic
}

type apiStabilityOffenderKey struct {
	symbol      *ast.Symbol
	signature   *checker.Signature
	declaration *ast.Node
}

func newApiStabilityWalker(
	ctx *rule.Context,
	session *typeparser.ApiStabilitySession,
	exportSymbol *ast.Symbol,
	exportTarget *ast.Symbol,
	location *ast.Node,
	ceiling typeparser.ApiStabilityLevel,
) *apiStabilityWalker {
	return &apiStabilityWalker{
		ctx:              ctx,
		session:          session,
		ceiling:          ceiling,
		exportSymbol:     exportSymbol,
		exportTarget:     exportTarget,
		location:         location,
		inspectedModules: make(map[*ast.Symbol]bool),
		offenders:        make(map[apiStabilityOffenderKey]bool),
	}
}

// inspectExport seeds the walk. Namespace re-exports expose their members'
// surfaces, so modules are enumerated explicitly.
func (w *apiStabilityWalker) inspectExport() {
	if w.exportTarget == nil {
		return
	}
	w.inspectSymbol(w.exportTarget)
}

func (w *apiStabilityWalker) inspectSymbol(symbol *ast.Symbol) {
	if symbol == nil {
		return
	}
	if symbol.Flags&ast.SymbolFlagsModule != 0 {
		w.inspectModule(symbol)
		return
	}
	for _, dependency := range w.session.UsedBySymbol(symbol).Dependencies {
		w.recordDependency(dependency)
	}
}

// inspectModule enumerates a namespace module's exports and inspects each
// member. Modules are guarded by resolved identity so namespace re-export cycles
// terminate. Type-only exports (interfaces and aliases) are not value properties
// of the namespace, so they are enumerated through the module symbol.
func (w *apiStabilityWalker) inspectModule(module *ast.Symbol) {
	if module == nil || w.inspectedModules[module] {
		return
	}
	w.inspectedModules[module] = true
	for _, member := range sortedApiStabilityExports(w.ctx, module) {
		if member == nil {
			continue
		}
		// An `@internal` member is not part of the namespace's public API, so
		// neither its declared stability nor its surface is inspected.
		if apiStabilityExportIsInternal(w.ctx, module, member) {
			continue
		}
		// A namespace exposes each member's own declared stability as part of the
		// surface, so a tagged member is reported even when it is not reachable
		// through another member's dependencies.
		w.recordDeclared(member)
		w.inspectSymbol(resolveStabilityAlias(w.ctx.Checker, member))
	}
}

// recordDeclared reports a namespace member whose own declared stability is
// above the namespace owner's ceiling. The member's surface is inspected
// separately for dependencies that themselves exceed the ceiling.
func (w *apiStabilityWalker) recordDeclared(symbol *ast.Symbol) {
	if symbol == nil {
		return
	}
	declared := w.ctx.TypeParser.DeclaredApiStabilityOfSymbol(symbol)
	w.report(typeparser.ApiStabilityDependency{
		Symbol:      symbol,
		Declaration: declared.Declaration,
		Level:       declared.Level,
	})
}

// recordDependency reports a dependency whose declared stability is above the
// export's ceiling. The owner's ceiling governs the entire surface; a member's
// own `@stability` tag never loosens it, and no separate stricter-member
// requirement is imposed.
func (w *apiStabilityWalker) recordDependency(dependency typeparser.ApiStabilityDependency) {
	w.report(dependency)
}

func (w *apiStabilityWalker) report(dependency typeparser.ApiStabilityDependency) {
	if dependency.Level <= w.ceiling {
		return
	}
	if dependency.Symbol == nil && dependency.Signature == nil && dependency.Declaration == nil {
		return
	}
	if w.sameAsExport(dependency) {
		return
	}
	key := apiStabilityOffenderKey{
		symbol:      dependency.Symbol,
		signature:   dependency.Signature,
		declaration: dependency.Declaration,
	}
	if w.offenders[key] {
		return
	}
	w.offenders[key] = true
	w.diagnostics = append(w.diagnostics, w.ctx.NewDiagnostic(
		w.ctx.SourceFile,
		w.ctx.GetErrorRange(w.location),
		tsdiag.X_0_exposes_1_an_2_API_effect_apiStabilityLeak,
		nil,
		apiStabilityExportDisplayName(w.ctx, w.exportSymbol),
		typeparser.ApiStabilityDependencyName(dependency),
		apiStabilityLevelName(dependency.Level),
	))
}

// sameAsExport filters the export's own symbol from its dependency list. A
// signature or distinct declaration finding is never the export itself, so it
// is still reported.
func (w *apiStabilityWalker) sameAsExport(dependency typeparser.ApiStabilityDependency) bool {
	if dependency.Symbol == nil || dependency.Signature != nil {
		return false
	}
	c := w.ctx.Checker
	return checker.Checker_getSymbolIfSameReference(c, dependency.Symbol, w.exportSymbol) != nil ||
		checker.Checker_getSymbolIfSameReference(c, dependency.Symbol, w.exportTarget) != nil
}

func apiStabilityLevelName(level typeparser.ApiStabilityLevel) string {
	if level == typeparser.ApiStabilityExperimental {
		return "experimental"
	}
	return "unstable"
}

func apiStabilityExportDisplayName(ctx *rule.Context, exportSymbol *ast.Symbol) string {
	name := exportSymbol.Name
	pkg := ctx.TypeParser.PackageJsonForSourceFile(ctx.SourceFile)
	if pkg == nil {
		return name
	}
	packageName, ok := pkg.Name.GetValue()
	if !ok || packageName == "" {
		return name
	}
	directory := getPackageJsonDirectory(ctx.Program, ctx.Checker, ctx.SourceFile)
	if moduleName := stabilityApiModuleName(packageName, directory, string(ctx.SourceFile.FileName())); moduleName != "" {
		return moduleName + "#" + name
	}
	return name
}

// apiStabilityExportLocation returns a node in the selected file that names the
// export, preferring a local declaration and falling back to the `export * from`
// declaration that forwards it.
func apiStabilityExportLocation(ctx *rule.Context, exportSymbol *ast.Symbol) *ast.Node {
	for _, declaration := range exportSymbol.Declarations {
		if declaration == nil || ast.GetSourceFileOfNode(declaration) != ctx.SourceFile {
			continue
		}
		if name := ast.GetNameOfDeclaration(declaration); name != nil {
			return name
		}
		return declaration
	}
	return apiStabilityReexportLocation(ctx, exportSymbol)
}

// apiStabilityForwardingStability returns the stability declared by a re-export
// declaration in the selected file that forwards this export: a star, a named
// specifier or a namespace export. The boolean reports whether any forwarding
// declaration carries a tag, which lets the caller give an explicit forwarding
// tag precedence even when it tightens the inherited ceiling.
func apiStabilityForwardingStability(ctx *rule.Context, exportSymbol *ast.Symbol) (typeparser.ApiStabilityLevel, bool) {
	if exportSymbol == nil {
		return typeparser.ApiStabilityStable, false
	}
	best := typeparser.ApiStabilityStable
	present := false
	for _, statement := range ctx.SourceFile.AsSourceFile().Statements.Nodes {
		if statement == nil || statement.Kind != ast.KindExportDeclaration {
			continue
		}
		declaration := statement.AsExportDeclaration()
		if declaration == nil {
			continue
		}
		stability := typeparser.StabilityTagOfDeclaration(statement)
		if stability == "" {
			continue
		}
		if !apiStabilityExportDeclarationForwards(ctx, declaration, exportSymbol) {
			continue
		}
		present = true
		if level := typeparser.ApiStabilityLevelFromTag(stability); level > best {
			best = level
		}
	}
	return best, present
}

// apiStabilityExportDeclarationForwards reports whether an export declaration in
// the selected file forwards the given export symbol.
func apiStabilityExportDeclarationForwards(ctx *rule.Context, declaration *ast.ExportDeclaration, exportSymbol *ast.Symbol) bool {
	if declaration.ExportClause == nil {
		if declaration.ModuleSpecifier == nil {
			return false
		}
		moduleSymbol := ctx.Checker.GetSymbolAtLocation(declaration.ModuleSpecifier)
		if moduleSymbol != nil && moduleSymbol.Flags&ast.SymbolFlagsAlias != 0 {
			moduleSymbol = ctx.Checker.GetAliasedSymbol(moduleSymbol)
		}
		if moduleSymbol == nil {
			return false
		}
		exported := ctx.Checker.TryGetMemberInModuleExportsAndProperties(exportSymbol.Name, moduleSymbol)
		return exported != nil && checker.Checker_getSymbolIfSameReference(ctx.Checker, exported, exportSymbol) != nil
	}
	switch declaration.ExportClause.Kind {
	case ast.KindNamespaceExport:
		namespace := declaration.ExportClause.AsNamespaceExport()
		return namespace != nil && namespace.Name() != nil && namespace.Name().Text() == exportSymbol.Name
	case ast.KindNamedExports:
		named := declaration.ExportClause.AsNamedExports()
		if named == nil || named.Elements == nil {
			return false
		}
		for _, element := range named.Elements.Nodes {
			if element == nil || element.Kind != ast.KindExportSpecifier {
				continue
			}
			specifier := element.AsExportSpecifier()
			if specifier != nil && specifier.Name() != nil && specifier.Name().Text() == exportSymbol.Name {
				return true
			}
		}
	}
	return false
}

// apiStabilityInternalVisit identifies one module-scoped visibility question:
// whether a symbol is internal as exported from a module. Recursion cycles are
// broken by this identity.
type apiStabilityInternalVisit struct {
	module *ast.Symbol
	symbol *ast.Symbol
}

// apiStabilityExportIsInternal reports whether an export is marked `@internal`
// and must not be checked. Native TypeScript recognizes the same tag for
// `stripInternal` declaration emit: a tagged declaration is omitted from the
// emitted declarations, so it is not part of the public API.
//
// An export is internal only when every path that exposes it from the module
// is internal: a declaration without the tag is public, a named specifier,
// star or namespace forwarding without the tag is public, and an untagged
// forwarding inherits the visibility of the target it forwards. A merged
// symbol with at least one non-internal declaration therefore stays public, so
// a public overload is still checked when only the implementation is tagged,
// while an untagged re-export of an internal target is skipped because the
// forwarded target is not public API. The check reads metadata only: parsed
// JSDoc tags, declarations and re-export statements, never types, so it never
// touches the dependency caches.
func apiStabilityExportIsInternal(ctx *rule.Context, module *ast.Symbol, exportSymbol *ast.Symbol) bool {
	if ctx == nil || module == nil || exportSymbol == nil {
		return false
	}
	return apiStabilitySymbolIsInternal(ctx, module, exportSymbol, make(map[apiStabilityInternalVisit]bool))
}

// apiStabilitySymbolIsInternal reports whether a symbol is internal as exported
// from one module. Declarations of the symbol that live in the module are its
// paths; a star re-export is inspected separately because it is not a
// declaration of the forwarded symbol. A path that revisits an in-progress
// visibility question reports no new public declaration: public declarations
// are always collected when their module is entered, before that module's
// forwards are followed, so a cycle can only be reached after every reachable
// declaration path has already decided its visibility.
//
// When the module's export table selects the symbol for its own name and the
// symbol has a declaration in the module, that explicit declaration is the
// compiler's export winner: a redundant `export *` forward of the same name
// must not defeat a tag written on the explicit declaration. Star paths are
// still followed when the symbol is only reachable through them, or when the
// selected export resolves to a different symbol.
func apiStabilitySymbolIsInternal(ctx *rule.Context, module *ast.Symbol, symbol *ast.Symbol, visiting map[apiStabilityInternalVisit]bool) bool {
	if module == nil || symbol == nil {
		return false
	}
	visit := apiStabilityInternalVisit{module: module, symbol: symbol}
	if visiting[visit] {
		return true
	}
	visiting[visit] = true
	defer delete(visiting, visit)

	files := apiStabilityModuleSourceFiles(module)
	if len(files) == 0 {
		return false
	}
	found := false
	for _, declaration := range symbol.Declarations {
		if declaration == nil || !apiStabilityDeclarationInFiles(declaration, files) {
			continue
		}
		found = true
		if !apiStabilityDeclarationPathIsInternal(ctx, module, symbol, declaration, visiting) {
			return false
		}
	}
	if found && apiStabilitySymbolIsModuleExport(ctx, module, symbol) {
		return true
	}
	for _, statement := range apiStabilityModuleExportStatements(module) {
		if statement == nil || statement.Kind != ast.KindExportDeclaration {
			continue
		}
		declaration := statement.AsExportDeclaration()
		// Named and namespace forwards are declarations of the forwarded
		// symbol; only a star forward is not, so it is inspected here.
		if declaration == nil || declaration.ExportClause != nil {
			continue
		}
		if !apiStabilityExportDeclarationForwards(ctx, declaration, symbol) {
			continue
		}
		found = true
		if !apiStabilityStarPathIsInternal(ctx, symbol, statement, declaration, visiting) {
			return false
		}
	}
	return found
}

// apiStabilitySymbolIsModuleExport reports whether a module's own export table
// selects exactly this symbol for its name. It is the metadata-only identity
// test that lets an explicit export declaration take precedence over redundant
// star forwards; it resolves aliases but never reads a type.
func apiStabilitySymbolIsModuleExport(ctx *rule.Context, module *ast.Symbol, symbol *ast.Symbol) bool {
	if module == nil || symbol == nil {
		return false
	}
	exported := ctx.Checker.TryGetMemberInModuleExportsAndProperties(symbol.Name, module)
	return checker.Checker_getSymbolIfSameReference(ctx.Checker, exported, symbol) != nil
}

// apiStabilityDeclarationPathIsInternal reports whether one declaration of an
// exported symbol is an internal path. A tag on the declaration or, for a named
// or namespace forward, on its enclosing export declaration marks the path
// internal; an untagged named forward stays internal only when the target it
// forwards is itself internal. An import alias follows the imported symbol to
// the module it was exported from, so `export { X }` after `import { X }`
// inherits the original declaration's tag; an `export import` alias follows the
// target of its module reference the same way. A namespace forward without a tag
// is public: its container is public, and its members are filtered individually.
func apiStabilityDeclarationPathIsInternal(ctx *rule.Context, module *ast.Symbol, symbol *ast.Symbol, declaration *ast.Node, visiting map[apiStabilityInternalVisit]bool) bool {
	if typeparser.InternalTagOfDeclaration(declaration) {
		return true
	}
	statement := apiStabilityForwardingStatementOf(declaration)
	if statement != nil && typeparser.InternalTagOfDeclaration(statement) {
		return true
	}
	switch declaration.Kind {
	case ast.KindExportSpecifier:
		targetModule := module
		if statement != nil {
			if exportDeclaration := statement.AsExportDeclaration(); exportDeclaration != nil && exportDeclaration.ModuleSpecifier != nil {
				targetModule = apiStabilityModuleOfSpecifier(ctx, exportDeclaration.ModuleSpecifier)
			}
		}
		return apiStabilityAliasedTargetIsInternal(ctx, targetModule, symbol, visiting)
	case ast.KindExportAssignment:
		return apiStabilityAliasedTargetIsInternal(ctx, module, symbol, visiting)
	case ast.KindImportSpecifier, ast.KindImportClause:
		return apiStabilityImportTargetIsInternal(ctx, declaration, symbol, visiting)
	case ast.KindImportEqualsDeclaration:
		return apiStabilityImportEqualsTargetIsInternal(ctx, symbol, declaration, visiting)
	}
	return false
}

// apiStabilityImportTargetIsInternal follows an untagged import alias to the
// symbol it imports and reports whether that symbol is internal as exported
// from the module it was imported from. An `export { X }` or
// `export default X` of an imported identifier re-exposes the same declaration,
// so it is internal exactly when the imported declaration is. Only the alias
// target named by the import declaration is followed; no other identifier is
// inferred and no type is read.
func apiStabilityImportTargetIsInternal(ctx *rule.Context, declaration *ast.Node, symbol *ast.Symbol, visiting map[apiStabilityInternalVisit]bool) bool {
	if symbol == nil || symbol.Flags&ast.SymbolFlagsAlias == 0 {
		return false
	}
	if !slices.ContainsFunc(symbol.Declarations, ast.IsAliasSymbolDeclaration) {
		return false
	}
	targetModule := apiStabilityImportTargetModule(ctx, declaration)
	if targetModule == nil {
		return false
	}
	target := typeparser.ApiStabilityImmediateAliasedSymbol(ctx.Checker, symbol)
	if target == nil || target == symbol {
		return false
	}
	return apiStabilitySymbolIsInternal(ctx, targetModule, target, visiting)
}

// apiStabilityImportTargetModule resolves the module an import alias targets
// through the import declaration that contains the alias. An unresolvable
// specifier reports no module, which keeps the import path public.
func apiStabilityImportTargetModule(ctx *rule.Context, declaration *ast.Node) *ast.Symbol {
	for node := declaration; node != nil; node = node.Parent {
		switch node.Kind {
		case ast.KindImportDeclaration:
			return apiStabilityModuleOfSpecifier(ctx, node.AsImportDeclaration().ModuleSpecifier)
		case ast.KindSourceFile, ast.KindModuleBlock:
			return nil
		}
	}
	return nil
}

// apiStabilityImportEqualsTargetIsInternal follows an untagged `import X = ...`
// alias, including the `export import` form, to the target it names and reports
// whether that target is internal as exported from its own module. The checker's
// native immediate alias target is preferred; when the checker left it
// unresolved (a namespace import qualifier, a chained alias or an unknown
// require), the target is read from the qualifier module's export table or from
// the referenced module itself, which is the same declaration metadata the
// checker's own resolution uses. Only the written module reference is followed
// and no type is read, so a tag on the alias declaration or on the target's
// declaration keeps exactly the precedence the named and default import paths
// give it.
func apiStabilityImportEqualsTargetIsInternal(ctx *rule.Context, symbol *ast.Symbol, declaration *ast.Node, visiting map[apiStabilityInternalVisit]bool) bool {
	if symbol == nil || symbol.Flags&ast.SymbolFlagsAlias == 0 {
		return false
	}
	if !slices.ContainsFunc(symbol.Declarations, ast.IsAliasSymbolDeclaration) {
		return false
	}
	importEquals := declaration.AsImportEqualsDeclaration()
	if importEquals == nil || importEquals.ModuleReference == nil {
		return false
	}
	reference := importEquals.ModuleReference
	target := typeparser.ApiStabilityImmediateAliasedSymbol(ctx.Checker, symbol)
	if target == nil {
		if name := apiStabilityImportEqualsTargetName(reference); name != "" {
			if qualifier := apiStabilityModuleOfEntity(ctx, apiStabilityImportEqualsQualifier(reference)); qualifier != nil {
				target = ctx.Checker.TryGetMemberInModuleExportsAndProperties(name, qualifier)
			}
		}
	}
	if target == nil && reference.Kind == ast.KindExternalModuleReference {
		// A native `require` alias stands for the module itself; the module
		// reference is that target when the checker has not resolved it.
		target = apiStabilityModuleOfEntity(ctx, apiStabilityImportEqualsQualifier(reference))
	}
	if target == nil || target == symbol {
		return false
	}
	modules := apiStabilityDeclarationModules(ctx, target)
	if len(modules) == 0 {
		return false
	}
	// The alias forwards one symbol; it is internal only when every module
	// that declares that symbol exposes it internally, like any merged path.
	for _, module := range modules {
		if !apiStabilitySymbolIsInternal(ctx, module, target, visiting) {
			return false
		}
	}
	return true
}

// apiStabilityImportEqualsQualifier returns the entity name that qualifies the
// target of an import-equals reference: the left side of a qualified name (the
// namespace the named member belongs to), the require argument of an external
// module reference, or nil for a bare identifier target.
func apiStabilityImportEqualsQualifier(reference *ast.Node) *ast.Node {
	if reference == nil {
		return nil
	}
	switch reference.Kind {
	case ast.KindQualifiedName:
		if qualified := reference.AsQualifiedName(); qualified != nil {
			return qualified.Left
		}
	case ast.KindExternalModuleReference:
		if external := reference.AsExternalModuleReference(); external != nil {
			return external.Expression
		}
	}
	return nil
}

// apiStabilityImportEqualsTargetName returns the member name an import-equals
// reference names inside its qualifier module. A bare identifier names itself
// and an external `require` names the module, so both report no member name.
func apiStabilityImportEqualsTargetName(reference *ast.Node) string {
	if reference == nil || reference.Kind != ast.KindQualifiedName {
		return ""
	}
	qualified := reference.AsQualifiedName()
	if qualified == nil || qualified.Right == nil {
		return ""
	}
	return qualified.Right.Text()
}

// apiStabilityDeclarationModule returns the module symbol that directly exports
// a declaration: the nearest enclosing declared namespace, or the source file's
// module for a top-level declaration. It is the metadata-only fallback for an
// alias target whose module is not named by a module reference, such as a bare
// identifier target or a chained alias.
func apiStabilityDeclarationModule(ctx *rule.Context, declaration *ast.Node) *ast.Symbol {
	if declaration == nil {
		return nil
	}
	for node := declaration; node != nil; node = node.Parent {
		switch node.Kind {
		case ast.KindModuleDeclaration, ast.KindSourceFile:
			return checker.Checker_getSymbolOfDeclaration(ctx.Checker, node)
		}
	}
	return nil
}

// apiStabilityDeclarationModules returns the distinct modules that declare a
// symbol, in declaration order. A symbol merged from several modules has one
// visibility path per module.
func apiStabilityDeclarationModules(ctx *rule.Context, symbol *ast.Symbol) []*ast.Symbol {
	if symbol == nil {
		return nil
	}
	var modules []*ast.Symbol
	for _, declaration := range symbol.Declarations {
		if declaration == nil {
			continue
		}
		module := apiStabilityDeclarationModule(ctx, declaration)
		if module == nil || slices.Contains(modules, module) {
			continue
		}
		modules = append(modules, module)
	}
	return modules
}

// apiStabilityStarPathIsInternal reports whether one star re-export path is
// internal. The tag on the star declaration marks the whole path internal;
// otherwise the path inherits the visibility of the symbol in the module it is
// forwarded from.
func apiStabilityStarPathIsInternal(ctx *rule.Context, symbol *ast.Symbol, statement *ast.Node, declaration *ast.ExportDeclaration, visiting map[apiStabilityInternalVisit]bool) bool {
	if typeparser.InternalTagOfDeclaration(statement) {
		return true
	}
	return apiStabilitySymbolIsInternal(ctx, apiStabilityModuleOfSpecifier(ctx, declaration.ModuleSpecifier), symbol, visiting)
}

// apiStabilityAliasedTargetIsInternal follows an untagged alias declaration to
// its immediate target and reports whether the target is internal. The checker
// synthesizes declaration-less aliases for `export =` and JSON modules and
// panics when asked for their immediate target, so those stay public.
func apiStabilityAliasedTargetIsInternal(ctx *rule.Context, module *ast.Symbol, symbol *ast.Symbol, visiting map[apiStabilityInternalVisit]bool) bool {
	if symbol == nil || symbol.Flags&ast.SymbolFlagsAlias == 0 {
		return false
	}
	if !slices.ContainsFunc(symbol.Declarations, ast.IsAliasSymbolDeclaration) {
		return false
	}
	target := typeparser.ApiStabilityImmediateAliasedSymbol(ctx.Checker, symbol)
	if target == nil || target == symbol {
		return false
	}
	return apiStabilitySymbolIsInternal(ctx, module, target, visiting)
}

// apiStabilityForwardingStatementOf returns the export declaration statement a
// named specifier or namespace export belongs to. A forwarding tag is written
// above the whole statement, so the statement carries the tag for every
// specifier inside it.
func apiStabilityForwardingStatementOf(declaration *ast.Node) *ast.Node {
	if declaration == nil || declaration.Parent == nil {
		return nil
	}
	if declaration.Parent.Kind == ast.KindExportDeclaration {
		return declaration.Parent
	}
	if declaration.Parent.Parent != nil && declaration.Parent.Parent.Kind == ast.KindExportDeclaration {
		return declaration.Parent.Parent
	}
	return nil
}

// apiStabilityModuleSourceFiles returns the source files a module symbol is
// declared in. A source file module is declared by the file itself; a declared
// namespace or ambient module is declared by its module declaration, and its
// members live in the file containing that declaration. Merged namespace
// declarations therefore contribute every file they appear in.
func apiStabilityModuleSourceFiles(module *ast.Symbol) []*ast.SourceFile {
	if module == nil {
		return nil
	}
	var files []*ast.SourceFile
	for _, declaration := range module.Declarations {
		if declaration == nil {
			continue
		}
		var file *ast.SourceFile
		switch declaration.Kind {
		case ast.KindSourceFile:
			file = declaration.AsSourceFile()
		case ast.KindModuleDeclaration:
			file = ast.GetSourceFileOfNode(declaration)
		}
		if file == nil || slices.Contains(files, file) {
			continue
		}
		files = append(files, file)
	}
	return files
}

// apiStabilityModuleExportStatements returns the statements that can forward
// exports out of a module with `export *`: the top-level statements of a source
// file, or the statements of the module blocks of its declared namespaces and
// ambient modules. Named and namespace forwards are declarations of the
// forwarded symbol itself and are inspected through symbol.Declarations; only
// star forwards need this separate enumeration. A dotted namespace declaration
// nests module declarations, so the body chain is followed to its module block.
func apiStabilityModuleExportStatements(module *ast.Symbol) []*ast.Node {
	if module == nil {
		return nil
	}
	var statements []*ast.Node
	for _, declaration := range module.Declarations {
		if declaration == nil {
			continue
		}
		switch declaration.Kind {
		case ast.KindSourceFile:
			if file := declaration.AsSourceFile(); file != nil {
				statements = append(statements, file.Statements.Nodes...)
			}
		case ast.KindModuleDeclaration:
			if block := apiStabilityModuleBlockOf(declaration); block != nil {
				statements = append(statements, block.Statements.Nodes...)
			}
		}
	}
	return statements
}

// apiStabilityModuleBlockOf returns the module block at the end of a module
// declaration's body chain. A namespace declared without a body (an ambient
// declaration) has no block and forwards nothing.
func apiStabilityModuleBlockOf(declaration *ast.Node) *ast.ModuleBlock {
	for node := declaration; node != nil && node.Kind == ast.KindModuleDeclaration; {
		body := node.AsModuleDeclaration().Body
		if body == nil {
			return nil
		}
		if body.Kind == ast.KindModuleBlock {
			return body.AsModuleBlock()
		}
		node = body
	}
	return nil
}

func apiStabilityDeclarationInFiles(declaration *ast.Node, files []*ast.SourceFile) bool {
	file := ast.GetSourceFileOfNode(declaration)
	return file != nil && slices.Contains(files, file)
}

// apiStabilityModuleOfSpecifier resolves the module symbol a module specifier
// refers to. An unresolved specifier reports no module, which keeps the
// forwarding path public.
func apiStabilityModuleOfSpecifier(ctx *rule.Context, moduleSpecifier *ast.Expression) *ast.Symbol {
	return apiStabilityModuleOfEntity(ctx, moduleSpecifier)
}

// apiStabilityModuleOfEntity resolves the module symbol an entity name or module
// specifier refers to, following an import alias to the module it names. An
// unresolved entity or a symbol that declares no module reports no module, which
// keeps the forwarding path public.
func apiStabilityModuleOfEntity(ctx *rule.Context, entity *ast.Node) *ast.Symbol {
	if entity == nil {
		return nil
	}
	module := ctx.Checker.GetSymbolAtLocation(entity)
	if module == nil {
		return nil
	}
	if module.Flags&ast.SymbolFlagsAlias != 0 {
		module = ctx.Checker.GetAliasedSymbol(module)
	}
	if module == nil || len(apiStabilityModuleSourceFiles(module)) == 0 {
		return nil
	}
	return module
}

func apiStabilityReexportLocation(ctx *rule.Context, exportSymbol *ast.Symbol) *ast.Node {
	var fallback *ast.Node
	for _, statement := range ctx.SourceFile.AsSourceFile().Statements.Nodes {
		if statement == nil || statement.Kind != ast.KindExportDeclaration {
			continue
		}
		declaration := statement.AsExportDeclaration()
		if declaration == nil || declaration.ModuleSpecifier == nil || declaration.ExportClause != nil {
			continue
		}
		if fallback == nil {
			fallback = statement
		}
		moduleSymbol := ctx.Checker.GetSymbolAtLocation(declaration.ModuleSpecifier)
		if moduleSymbol == nil {
			continue
		}
		if moduleSymbol.Flags&ast.SymbolFlagsAlias != 0 {
			moduleSymbol = ctx.Checker.GetAliasedSymbol(moduleSymbol)
		}
		if moduleSymbol == nil {
			continue
		}
		exported := ctx.Checker.TryGetMemberInModuleExportsAndProperties(exportSymbol.Name, moduleSymbol)
		if exported != nil && checker.Checker_getSymbolIfSameReference(ctx.Checker, exported, exportSymbol) != nil {
			return statement
		}
	}
	return fallback
}
