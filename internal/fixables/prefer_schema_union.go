package fixables

import (
	"fmt"
	"strings"

	"github.com/effect-ts/tsgo/internal/fixable"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/rules"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/ls"
	"github.com/microsoft/TypeScript/tsc/shim/scanner"
)

var PreferSchemaUnionFix = fixable.Fixable{
	Name:        "preferSchemaUnion",
	Description: "Create Schema.Union and retain the type alias",
	ErrorCodes: []int32{
		tsdiag.This_type_alias_unions_decoded_Effect_Schema_types_Prefer_Schema_Union_and_derive_the_type_from_the_resulting_schema_effect_preferSchemaUnion.Code(),
	},
	FixIDs: []string{"preferSchemaUnion_fix"},
	Run:    runPreferSchemaUnionFix,
}

func runPreferSchemaUnionFix(ctx *fixable.Context) []ls.CodeAction {
	matches := rules.AnalyzePreferSchemaUnion(ctx.TypeParser, ctx.SourceFile)
	for _, match := range matches {
		if !match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location) {
			continue
		}

		if !isFixApplicable(ctx, match) {
			continue
		}

		action := ctx.NewFixAction(fixable.FixAction{
			Description: "Compose Schema.Union and derive type",
			Run: func(tracker *rewriter.Tracker) {
				applyPreferSchemaUnionFix(ctx, tracker, match)
			},
		})
		if action != nil {
			return []ls.CodeAction{*action}
		}
		return nil
	}

	return nil
}

func isFixApplicable(ctx *fixable.Context, match rules.PreferSchemaUnionMatch) bool {
	if ctx == nil || ctx.SourceFile == nil || match.Alias == nil {
		return false
	}

	// Limit offered fixes to top-level aliases in external modules
	if !ast.IsExternalModule(ctx.SourceFile) {
		return false
	}
	if match.Alias.Parent == nil || match.Alias.Parent.Kind != ast.KindSourceFile {
		return false
	}

	aliasDecl := match.Alias.AsTypeAliasDeclaration()
	if aliasDecl == nil || aliasDecl.Name() == nil || aliasDecl.Type == nil {
		return false
	}

	// No ambient declarations
	if match.Alias.Flags&ast.NodeFlagsAmbient != 0 || ast.HasAmbientModifier(match.Alias) {
		return false
	}

	// No name collision for the new const in the value namespace
	aliasName := aliasDecl.Name().Text()
	if ctx.Checker.ResolveName(aliasName, match.Alias, ast.SymbolFlagsValue, false) != nil {
		return false
	}

	// No comments inside the replaced union range
	typeNode := aliasDecl.Type.AsNode()
	if hasCommentsInRange(ctx.SourceFile, typeNode.Pos(), typeNode.End()) {
		return false
	}

	// Schema leaves must be simple identifiers referencing same-file initialized const declarations or schema classes preceding the alias
	if len(match.Schemas) == 0 {
		return false
	}
	for _, schemaNode := range match.Schemas {
		if schemaNode == nil || schemaNode.Kind != ast.KindIdentifier {
			return false
		}

		sym := ctx.Checker.GetSymbolAtLocation(schemaNode)
		if sym == nil {
			return false
		}
		if sym.Flags&ast.SymbolFlagsAlias != 0 {
			return false
		}

		if len(sym.Declarations) == 0 {
			return false
		}

		hasValidDecl := false
		for _, decl := range sym.Declarations {
			if decl == nil {
				continue
			}
			if ast.GetSourceFileOfNode(decl) != ctx.SourceFile {
				return false
			}
			// Must strictly precede the alias (no forward or self references)
			if decl.End() > match.Alias.Pos() {
				return false
			}
			// Must not be ambient
			if decl.Flags&ast.NodeFlagsAmbient != 0 || ast.HasAmbientModifier(decl) {
				return false
			}

			if ast.IsVariableDeclaration(decl) {
				vd := decl.AsVariableDeclaration()
				if vd == nil || vd.Initializer == nil || !ast.IsVarConst(decl) {
					return false
				}
				hasValidDecl = true
			} else if ast.IsClassDeclaration(decl) {
				hasValidDecl = true
			}
		}

		if !hasValidDecl {
			return false
		}
	}

	return true
}

func hasCommentsInRange(sf *ast.SourceFile, pos, end int) bool {
	if sf == nil {
		return false
	}
	text := sf.Text()
	if pos < 0 || end > len(text) || pos >= end {
		return false
	}
	rangeText := text[pos:end]
	return strings.Contains(rangeText, "//") || strings.Contains(rangeText, "/*")
}

func applyPreferSchemaUnionFix(ctx *fixable.Context, tracker *rewriter.Tracker, match rules.PreferSchemaUnionMatch) {
	aliasDecl := match.Alias.AsTypeAliasDeclaration()
	if aliasDecl == nil || aliasDecl.Name() == nil || aliasDecl.Type == nil {
		return
	}
	aliasName := aliasDecl.Name().Text()

	// 1. Resolve Schema value import
	schemaModuleName, needImport := resolveSchemaImport(ctx, match.Alias)

	if needImport {
		var specifier *ast.Node
		if schemaModuleName == "Schema" {
			specifier = tracker.NewImportSpecifier(false, nil, tracker.NewIdentifier("Schema"))
		} else {
			specifier = tracker.NewImportSpecifier(false, tracker.NewIdentifier("Schema"), tracker.NewIdentifier(schemaModuleName))
		}
		namedImports := tracker.NewNamedImports(tracker.NewNodeList([]*ast.Node{specifier}))
		importClause := tracker.NewImportClause(ast.KindUnknown, nil, namedImports)
		moduleSpec := tracker.NewStringLiteral("effect", ast.TokenFlagsNone)
		importDecl := tracker.NewImportDeclaration(nil, importClause, moduleSpec, nil)
		ast.SetParentInChildren(importDecl)
		tracker.InsertAtTopOfFile(ctx.SourceFile, []*ast.Statement{importDecl}, false)
	}

	// 2. Build Schema.Union([A, B])
	schemaId := tracker.NewIdentifier(schemaModuleName)
	callee := tracker.NewPropertyAccessExpression(
		schemaId,
		nil,
		tracker.NewIdentifier("Union"),
		ast.NodeFlagsNone,
	)

	arrayElements := make([]*ast.Node, 0, len(match.Schemas))
	for _, schemaNode := range match.Schemas {
		arrayElements = append(arrayElements, tracker.DeepCloneNode(schemaNode))
	}
	arrayLiteral := tracker.NewArrayLiteralExpression(tracker.NewNodeList(arrayElements), false)
	callExpr := tracker.NewCallExpression(callee, nil, nil, tracker.NewNodeList([]*ast.Node{arrayLiteral}), ast.NodeFlagsNone)

	// 3. Build const declaration
	varDecl := tracker.NewVariableDeclaration(tracker.DeepCloneNode(aliasDecl.Name().AsNode()), nil, nil, callExpr)
	varDeclList := tracker.NewVariableDeclarationList(tracker.NewNodeList([]*ast.Node{varDecl}), ast.NodeFlagsConst)

	var modifierList *ast.ModifierList
	if ast.HasSyntacticModifier(match.Alias, ast.ModifierFlagsExport) {
		modifierList = tracker.NewModifierList([]*ast.Node{tracker.NewModifier(ast.KindExportKeyword)})
	}
	constStatement := tracker.NewVariableStatement(modifierList, varDeclList)
	ast.SetParentInChildren(constStatement)

	// 4. Insert const immediately before the alias
	tracker.InsertNodeBefore(ctx.SourceFile, match.Alias, constStatement, false, rewriter.LeadingTriviaOptionNone)

	// 5. Replace RHS of alias with typeof Alias.Type
	schemaType := tracker.NewQualifiedName(tracker.NewIdentifier(aliasName), tracker.NewIdentifier("Type"))
	rhsReplacement := tracker.NewTypeQueryNode(schemaType, nil)
	ast.SetParentInChildren(rhsReplacement)
	tracker.ReplaceNode(ctx.SourceFile, aliasDecl.Type.AsNode(), rhsReplacement, nil)
}

func resolveSchemaImport(ctx *fixable.Context, aliasNode *ast.Node) (string, bool) {
	candidate := typeparser.FindModuleIdentifier(ctx.SourceFile, "Schema")
	if isUsableSchemaImport(ctx, candidate) {
		return candidate, false
	}
	return chooseUnboundSchemaName(ctx.Checker, aliasNode), true
}

func isUsableSchemaImport(ctx *fixable.Context, candidate string) bool {
	if ctx == nil || ctx.SourceFile == nil || candidate == "" {
		return false
	}
	for _, stmt := range ctx.SourceFile.Statements.Nodes {
		if stmt.Kind != ast.KindImportDeclaration {
			continue
		}
		importDecl := stmt.AsImportDeclaration()
		if importDecl == nil || importDecl.ModuleSpecifier == nil || importDecl.ImportClause == nil {
			continue
		}
		if importDecl.AsNode().IsTypeOnly() {
			continue
		}
		clauseNode := importDecl.ImportClause.AsNode()
		if clauseNode.IsTypeOnly() {
			continue
		}
		clause := importDecl.ImportClause.AsImportClause()
		if clause == nil {
			continue
		}

		moduleName := scanner.GetTextOfNode(importDecl.ModuleSpecifier)
		if len(moduleName) >= 2 && (moduleName[0] == '"' || moduleName[0] == '\'') {
			moduleName = moduleName[1 : len(moduleName)-1]
		}

		if clause.NamedBindings == nil {
			continue
		}

		if moduleName == "effect/Schema" && clause.NamedBindings.Kind == ast.KindNamespaceImport {
			nsImport := clause.NamedBindings.AsNamespaceImport()
			if nsImport != nil && nsImport.Name() != nil && nsImport.Name().Text() == candidate {
				if !ast.IsTypeOnlyImportDeclaration(nsImport.AsNode()) {
					if isSymbolFromEffectPackage(ctx, nsImport.Name().AsNode()) {
						return true
					}
				}
			}
		}

		if (moduleName == "effect" || moduleName == "effect/Schema") && clause.NamedBindings.Kind == ast.KindNamedImports {
			namedImports := clause.NamedBindings.AsNamedImports()
			if namedImports == nil || namedImports.Elements == nil {
				continue
			}
			for _, elem := range namedImports.Elements.Nodes {
				spec := elem.AsImportSpecifier()
				if spec == nil || ast.IsTypeOnlyImportDeclaration(elem) {
					continue
				}
				localName := spec.Name().Text()
				importedName := localName
				if spec.PropertyName != nil {
					importedName = spec.PropertyName.Text()
				}
				if localName == candidate && importedName == "Schema" {
					if isSymbolFromEffectPackage(ctx, spec.Name().AsNode()) {
						return true
					}
				}
			}
		}
	}
	return false
}

func chooseUnboundSchemaName(c *checker.Checker, aliasNode *ast.Node) string {
	if !isNameBound(c, aliasNode, "Schema") {
		return "Schema"
	}
	name := "SchemaUnion"
	suffix := 2
	for isNameBound(c, aliasNode, name) {
		name = fmt.Sprintf("SchemaUnion%d", suffix)
		suffix++
	}
	return name
}

func isNameBound(c *checker.Checker, location *ast.Node, name string) bool {
	if c == nil || location == nil {
		return false
	}
	if c.ResolveName(name, location, ast.SymbolFlagsValue, false) != nil {
		return true
	}
	if c.ResolveName(name, location, ast.SymbolFlagsType, false) != nil {
		return true
	}
	return false
}

func isSymbolFromEffectPackage(ctx *fixable.Context, node *ast.Node) bool {
	if ctx == nil || ctx.TypeParser == nil || ctx.Checker == nil || node == nil {
		return false
	}
	sym := ctx.Checker.GetSymbolAtLocation(node)
	if sym == nil {
		return false
	}
	for sym != nil && sym.Flags&ast.SymbolFlagsAlias != 0 {
		sym = ctx.Checker.GetAliasedSymbol(sym)
	}
	if sym == nil || len(sym.Declarations) == 0 {
		return false
	}
	declSf := ast.GetSourceFileOfNode(sym.Declarations[0])
	if declSf == nil {
		return false
	}
	return ctx.TypeParser.IsSourceFileInPackage(declSf, "effect")
}
