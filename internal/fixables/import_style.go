package fixables

import (
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/importstyle"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var PreferBarrelImportFix = fixable.Fixable{
	Name:        "preferBarrelImport",
	Description: "Import the module from its barrel and qualify the uses of its named imports",
	ErrorCodes:  []int32{tsdiag.X_0_is_imported_directly_but_barrelImportPackages_expects_named_imports_from_the_barrel_1_Use_2_instead_effect_preferBarrelImport.Code()},
	Run:         runPreferBarrelImportFix,
}

var PreferNamespaceImportFix = fixable.Fixable{
	Name:        "preferNamespaceImport",
	Description: "Import the module as a namespace and qualify the uses of the named import",
	ErrorCodes:  []int32{tsdiag.X_0_is_imported_by_name_from_1_but_namespaceImportPackages_expects_a_namespace_import_Use_2_instead_effect_preferNamespaceImport.Code()},
	Run:         runPreferNamespaceImportFix,
}

func runPreferBarrelImportFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzePreferBarrelImport(ctx.TypeParser, ctx.Program, ctx.SourceFile, ctx.Options) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if action := preferBarrelImportAction(ctx, match); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}
	return nil
}

func preferBarrelImportAction(ctx *fixable.Context, match rules.PreferBarrelImportMatch) *ls.CodeAction {
	sf := ctx.SourceFile
	decl := match.ImportDeclaration
	importDecl := decl.AsImportDeclaration()
	clause := importDecl.ImportClause.AsImportClause()
	if clause.Name() != nil || !importDeclarationRewritable(sf, decl) {
		return nil
	}
	quote := moduleSpecifierQuote(importDecl.ModuleSpecifier)
	semicolon := importDeclarationSemicolon(sf, decl)

	// `import * as R from "effect/Record"` keeps its binding, so only the
	// declaration changes.
	if clause.NamedBindings.Kind == ast.KindNamespaceImport {
		text := importKeyword(match.TypeOnly) + "{ " + match.Binding + " } from " + quote + match.Barrel.Specifier + quote + semicolon
		return ctx.NewFixAction(fixable.FixAction{
			Description: importDescription(match.TypeOnly) + match.Binding + " from '" + match.Barrel.Specifier + "'",
			Run: func(tracker *rewriter.Tracker) {
				tracker.ReplaceTextRangeWithText(sf, importDeclarationRange(sf, decl), text)
			},
		})
	}

	elements := clause.NamedBindings.AsNamedImports().Elements
	if elements == nil || len(elements.Nodes) == 0 {
		return nil
	}
	typeOnly := match.TypeOnly
	if !typeOnly {
		typeOnly = true
		for _, element := range elements.Nodes {
			if !element.IsTypeOnly() {
				typeOnly = false
				break
			}
		}
	}
	namespace := importAlias(ctx.Options, match.Specifier, match.Barrel.ExportName)
	reuse, ok := findBarrelBinding(sf, decl, match.Barrel, namespace, typeOnly)
	if !ok {
		return nil
	}
	bindings := make([]importBindingRewrite, 0, len(elements.Nodes))
	for _, element := range elements.Nodes {
		member := element.Name()
		if propertyName := element.PropertyName(); propertyName != nil {
			member = propertyName
		}
		if member.Kind != ast.KindIdentifier {
			return nil
		}
		bindings = append(bindings, importBindingRewrite{symbol: element.Symbol(), member: member.Text()})
	}
	uses, ok := qualifyBindingUses(ctx.TypeParser, ctx.Checker, sf, namespace, bindings, reuse, decl)
	if !ok {
		return nil
	}

	binding := match.Barrel.ExportName
	if namespace != binding {
		binding += " as " + namespace
	}
	text := importKeyword(typeOnly) + "{ " + binding + " } from " + quote + match.Barrel.Specifier + quote + semicolon
	description := importDescription(typeOnly) + binding + " from '" + match.Barrel.Specifier + "' and qualify the uses"
	if reuse != nil {
		description = "Qualify the uses with the existing " + namespace + " import from '" + match.Barrel.Specifier + "'"
	}
	return ctx.NewFixAction(fixable.FixAction{
		Description: description,
		Run: func(tracker *rewriter.Tracker) {
			if reuse != nil {
				tracker.Delete(sf, decl)
			} else {
				tracker.ReplaceTextRangeWithText(sf, importDeclarationRange(sf, decl), text)
			}
			uses.apply(tracker, sf)
		},
	})
}

func runPreferNamespaceImportFix(ctx *fixable.Context) []ls.CodeAction {
	for _, match := range rules.AnalyzePreferNamespaceImport(ctx.TypeParser, ctx.Checker, ctx.SourceFile, ctx.Options) {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}
		if action := preferNamespaceImportAction(ctx, match); action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}
	return nil
}

func preferNamespaceImportAction(ctx *fixable.Context, match rules.PreferNamespaceImportMatch) *ls.CodeAction {
	sf := ctx.SourceFile
	decl := match.ImportDeclaration
	importDecl := decl.AsImportDeclaration()
	clause := importDecl.ImportClause.AsImportClause()
	if match.Target.Specifier == "" || !importDeclarationRewritable(sf, decl) {
		return nil
	}
	elements := clause.NamedBindings.AsNamedImports().Elements
	// The reported specifier is the declaration's only binding, so the new
	// import takes the declaration's place.
	sole := clause.Name() == nil && len(elements.Nodes) == 1

	namespace := match.Target.Name
	var reuse *ast.Symbol
	var uses bindingUses
	if match.Target.Member != "" {
		if !scanner.IsIdentifierText(match.Target.Member, core.LanguageVariantStandard) {
			return nil
		}
		namespace = importAlias(ctx.Options, match.Target.Specifier, match.Target.Name)
		var ok bool
		reuse, ok = findNamespaceBinding(sf, decl, match.Target.Specifier, namespace, match.TypeOnly)
		if !ok {
			return nil
		}
		bindings := []importBindingRewrite{{symbol: match.Element.Symbol(), member: match.Target.Member}}
		uses, ok = qualifyBindingUses(ctx.TypeParser, ctx.Checker, sf, namespace, bindings, reuse, match.Element)
		if !ok {
			return nil
		}
	}

	quote := moduleSpecifierQuote(importDecl.ModuleSpecifier)
	text := importKeyword(match.TypeOnly) + "* as " + namespace + " from " + quote + match.Target.Specifier + quote + importDeclarationSemicolon(sf, decl)
	element := match.Element
	description := importDescription(match.TypeOnly) + "'" + match.Target.Specifier + "' as " + namespace
	switch {
	case reuse != nil:
		description = "Qualify the uses with the existing " + namespace + " import of '" + match.Target.Specifier + "'"
	case match.Target.Member != "":
		description += " and qualify the uses"
	}
	return ctx.NewFixAction(fixable.FixAction{
		Description: description,
		Run: func(tracker *rewriter.Tracker) {
			switch {
			case reuse != nil:
				tracker.Delete(sf, element)
			case sole:
				tracker.ReplaceTextRangeWithText(sf, importDeclarationRange(sf, decl), text)
			default:
				tracker.Delete(sf, element)
				tracker.ReplaceTextRangeWithText(sf, core.NewTextRange(decl.End(), decl.End()), "\n"+text)
			}
			uses.apply(tracker, sf)
		},
	})
}

// importBindingRewrite is an imported binding whose uses become `Namespace.member`.
type importBindingRewrite struct {
	symbol *ast.Symbol
	member string
}

type bindingUseEdit struct {
	textRange core.TextRange
	text      string
}

type bindingUses []bindingUseEdit

func (uses bindingUses) apply(tracker *rewriter.Tracker, sf *ast.SourceFile) {
	for _, use := range uses {
		tracker.ReplaceTextRangeWithText(sf, use.textRange, use.text)
	}
}

// qualifyBindingUses computes the edits that turn each use of the bindings
// into `namespace.member`. It returns false when the rewrite would change what
// a name refers to: when namespace is already bound in the file or names a
// global the file reads, or when a binding is re-exported, since a qualified
// name cannot stand in an export specifier. Identifiers inside replaced are
// ignored, and uses of reuse, an existing import of the namespace, are allowed.
func qualifyBindingUses(tp *typeparser.TypeParser, c *checker.Checker, sf *ast.SourceFile, namespace string, bindings []importBindingRewrite, reuse *ast.Symbol, replaced *ast.Node) (bindingUses, bool) {
	var uses bindingUses
	ok := true
	var visit ast.Visitor
	visit = func(node *ast.Node) bool {
		if !ok {
			return true
		}
		if node == replaced {
			return false
		}
		if node.Kind != ast.KindIdentifier {
			node.ForEachChild(visit)
			return false
		}
		if isMemberName(node) {
			return false
		}
		symbol := referencedSymbol(tp, c, node)
		for _, binding := range bindings {
			if symbol == nil || symbol != binding.symbol {
				continue
			}
			if node.Parent.Kind == ast.KindExportSpecifier {
				ok = false
				return true
			}
			qualified := namespace + "." + binding.member
			if node.Parent.Kind == ast.KindShorthandPropertyAssignment {
				qualified = node.Text() + ": " + qualified
			}
			uses = append(uses, bindingUseEdit{
				textRange: core.NewTextRange(scanner.GetTokenPosOfNode(node, sf, false), node.End()),
				text:      qualified,
			})
			return false
		}
		if node.Text() == namespace && (reuse == nil || symbol != reuse) {
			ok = false
			return true
		}
		return false
	}
	sf.AsNode().ForEachChild(visit)
	return uses, ok
}

func referencedSymbol(tp *typeparser.TypeParser, c *checker.Checker, identifier *ast.Node) *ast.Symbol {
	if identifier.Parent.Kind == ast.KindShorthandPropertyAssignment {
		return c.GetShorthandAssignmentValueSymbol(identifier.Parent)
	}
	if identifier.Parent.Kind == ast.KindExportSpecifier && identifier.Parent.Parent.Parent.AsExportDeclaration().ModuleSpecifier == nil {
		exportSpecifier := identifier.Parent
		local := exportSpecifier.PropertyName()
		if local == nil {
			local = exportSpecifier.Name()
		}
		if local != identifier {
			return nil
		}
		return c.ResolveName(identifier.Text(), identifier, ast.SymbolFlagsValue|ast.SymbolFlagsType|ast.SymbolFlagsNamespace|ast.SymbolFlagsAlias, false)
	}
	return tp.GetSymbolAtLocation(identifier)
}

// isMemberName reports whether an identifier names a member rather than
// referring to a binding in scope.
func isMemberName(identifier *ast.Node) bool {
	parent := identifier.Parent
	switch parent.Kind {
	case ast.KindPropertyAccessExpression, ast.KindPropertyDeclaration, ast.KindPropertySignature, ast.KindMethodDeclaration,
		ast.KindMethodSignature, ast.KindGetAccessor, ast.KindSetAccessor, ast.KindEnumMember, ast.KindPropertyAssignment,
		ast.KindJsxAttribute, ast.KindImportAttribute:
		return parent.Name() == identifier
	case ast.KindQualifiedName:
		return parent.AsQualifiedName().Right == identifier
	case ast.KindBindingElement, ast.KindImportSpecifier:
		return parent.PropertyName() == identifier
	case ast.KindExportSpecifier:
		// In `export { local as exported }` only `local` refers to a binding.
		return parent.PropertyName() != nil && parent.Name() == identifier
	case ast.KindLabeledStatement, ast.KindBreakStatement, ast.KindContinueStatement:
		return true
	}
	return false
}

// findBarrelBinding finds another import of the barrel's export bound to
// namespace. It returns false when namespace is bound by one that can't serve
// the rewrite because it is type-only.
func findBarrelBinding(sf *ast.SourceFile, reported *ast.Node, barrel importstyle.Barrel, namespace string, typeOnly bool) (*ast.Symbol, bool) {
	for _, stmt := range sf.Statements.Nodes {
		if stmt == reported || stmt.Kind != ast.KindImportDeclaration {
			continue
		}
		importDecl := stmt.AsImportDeclaration()
		if importDecl.ImportClause == nil || importDecl.ModuleSpecifier.Kind != ast.KindStringLiteral || importDecl.ModuleSpecifier.Text() != barrel.Specifier {
			continue
		}
		namedBindings := importDecl.ImportClause.AsImportClause().NamedBindings
		if namedBindings == nil || namedBindings.Kind != ast.KindNamedImports {
			continue
		}
		for _, element := range namedBindings.AsNamedImports().Elements.Nodes {
			imported := element.Name()
			if propertyName := element.PropertyName(); propertyName != nil {
				imported = propertyName
			}
			if element.Name().Text() != namespace || imported.Text() != barrel.ExportName {
				continue
			}
			if !typeOnly && (importDecl.ImportClause.IsTypeOnly() || element.IsTypeOnly()) {
				return nil, false
			}
			return element.Symbol(), true
		}
	}
	return nil, true
}

// findNamespaceBinding finds another `import * as namespace from specifier`.
// It returns false when that import is type-only and the rewrite needs a value.
func findNamespaceBinding(sf *ast.SourceFile, reported *ast.Node, specifier string, namespace string, typeOnly bool) (*ast.Symbol, bool) {
	for _, stmt := range sf.Statements.Nodes {
		if stmt == reported || stmt.Kind != ast.KindImportDeclaration {
			continue
		}
		importDecl := stmt.AsImportDeclaration()
		if importDecl.ImportClause == nil || importDecl.ModuleSpecifier.Kind != ast.KindStringLiteral || importDecl.ModuleSpecifier.Text() != specifier {
			continue
		}
		namedBindings := importDecl.ImportClause.AsImportClause().NamedBindings
		if namedBindings == nil || namedBindings.Kind != ast.KindNamespaceImport || namedBindings.Name().Text() != namespace {
			continue
		}
		if !typeOnly && importDecl.ImportClause.IsTypeOnly() {
			return nil, false
		}
		return namedBindings.Symbol(), true
	}
	return nil, true
}

// importAlias returns the name auto-import gives a namespace of the package
// that specifier imports, applying importAliases.
func importAlias(options *etscore.ResolvedEffectPluginOptions, specifier string, name string) string {
	if options == nil {
		return name
	}
	packageName := strings.ToLower(importstyle.PackageName(specifier))
	for pkg, alias := range options.ImportAliases {
		if strings.ToLower(pkg) == packageName && alias != "" {
			return alias
		}
	}
	return name
}

// importDeclarationRewritable reports whether the declaration can be rewritten
// without losing text: it carries no import attributes and no comment.
func importDeclarationRewritable(sf *ast.SourceFile, decl *ast.Node) bool {
	importDecl := decl.AsImportDeclaration()
	if importDecl.Attributes != nil {
		return false
	}
	text := sf.Text()
	start := scanner.GetTokenPosOfNode(decl, sf, false)
	specifierStart := scanner.GetTokenPosOfNode(importDecl.ModuleSpecifier, sf, false)
	rest := text[start:specifierStart] + text[importDecl.ModuleSpecifier.End():decl.End()]
	return !strings.Contains(rest, "//") && !strings.Contains(rest, "/*")
}

func importDeclarationRange(sf *ast.SourceFile, decl *ast.Node) core.TextRange {
	return core.NewTextRange(scanner.GetTokenPosOfNode(decl, sf, false), decl.End())
}

func importDeclarationSemicolon(sf *ast.SourceFile, decl *ast.Node) string {
	if strings.HasSuffix(sf.Text()[:decl.End()], ";") {
		return ";"
	}
	return ""
}

func moduleSpecifierQuote(moduleSpecifier *ast.Node) string {
	if moduleSpecifier.AsStringLiteral().TokenFlags&ast.TokenFlagsSingleQuote != 0 {
		return "'"
	}
	return "\""
}

func importDescription(typeOnly bool) string {
	if typeOnly {
		return "Import type "
	}
	return "Import "
}

func importKeyword(typeOnly bool) string {
	if typeOnly {
		return "import type "
	}
	return "import "
}
