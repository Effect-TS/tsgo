package rules

import (
	"slices"
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	tsdiag "github.com/microsoft/TypeScript/tsc/shim/diagnostics"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
)

var ExperimentalApiUsage = rule.Rule{
	Name:            "experimentalApiUsage",
	Group:           "correctness",
	Description:     "Warns when using an API marked @stability experimental",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.X_0_is_an_experimental_API_effect_experimentalApiUsage.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runStabilityApiUsage(ctx, "experimental")
	},
}

var UnstableApiUsage = rule.Rule{
	Name:            "unstableApiUsage",
	Group:           "correctness",
	Description:     "Warns when using an API marked @stability unstable",
	DefaultSeverity: etscore.SeverityWarning,
	SupportedEffect: []string{"v4"},
	Codes:           []int32{tsdiag.X_0_is_an_unstable_API_Breaking_changes_may_happen_between_versions_effect_unstableApiUsage.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		return runStabilityApiUsage(ctx, "unstable")
	},
}

type declarationStabilityInfo struct {
	stability   string
	declaration *ast.Node
}

func runStabilityApiUsage(ctx *rule.Context, wanted string) []*ast.Diagnostic {
	// Most references to a given API share its symbol. Cache declaration lookups so
	// lazy JSDoc parsing happens only once per declaration in this source file.
	declarationStability := make(map[*ast.Node]declarationStabilityInfo)
	readDeclaration := func(declaration *ast.Node) declarationStabilityInfo {
		if stability, ok := declarationStability[declaration]; ok {
			return stability
		}
		stability := declarationStabilityInfo{stabilityOfDeclaration(declaration), declaration}
		declarationStability[declaration] = stability
		return stability
	}
	readSymbol := func(symbol *ast.Symbol) declarationStabilityInfo {
		for depth := 0; symbol != nil && depth < 32; depth++ {
			for _, declaration := range symbol.Declarations {
				if stability := readDeclaration(declaration); stability.stability != "" {
					return stability
				}
			}
			if symbol.Flags&ast.SymbolFlagsAlias == 0 {
				break
			}
			// The checker synthesizes a declaration-less `default` alias for `export =`
			// and JSON modules and panics when asked for its immediate target.
			if !slices.ContainsFunc(symbol.Declarations, ast.IsAliasSymbolDeclaration) {
				break
			}
			next := ctx.Checker.GetImmediateAliasedSymbol(symbol)
			if next == symbol {
				break
			}
			symbol = next
		}
		return declarationStabilityInfo{}
	}

	allow := newStabilityApiAllowlist(ctx, wanted)
	var diagnostics []*ast.Diagnostic
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		if node.Kind == ast.KindIdentifier && !ast.IsDeclarationNameOrImportPropertyName(node) {
			// The selected overload is authoritative for calls. A tagged overload
			// may differ from other declarations of the same symbol.
			stability := declarationStabilityInfo{}
			selectedDeclaration := (*ast.Node)(nil)
			callee := node
			if parent := node.Parent; parent != nil && parent.Kind == ast.KindPropertyAccessExpression && parent.AsPropertyAccessExpression().Name() == node {
				callee = parent
			}
			if parent := callee.Parent; parent != nil && (parent.Kind == ast.KindCallExpression || parent.Kind == ast.KindNewExpression) && parent.Expression() == callee {
				if signature := ctx.Checker.GetResolvedSignature(parent); signature != nil && signature.Declaration() != nil {
					selectedDeclaration = signature.Declaration()
					stability = readDeclaration(selectedDeclaration)
				}
			}
			symbol := ctx.Checker.GetSymbolAtLocation(node)
			resolvedSymbol := ctx.TypeParser.ReferenceSymbolAtNode(node)
			useSymbol := selectedDeclaration == nil || !symbolHasDeclaration(symbol, selectedDeclaration) && !symbolHasDeclaration(resolvedSymbol, selectedDeclaration)
			if stability.stability == "" && useSymbol {
				stability = readSymbol(symbol)
			}
			if stability.stability == "" && useSymbol {
				stability = readSymbol(resolvedSymbol)
			}
			if stability.stability == wanted {
				name := node.Text()
				allowed, apiName := allow(stability.declaration)
				if allowed {
					return false
				}
				if apiName != "" {
					name = apiName
				}
				message := tsdiag.X_0_is_an_unstable_API_Breaking_changes_may_happen_between_versions_effect_unstableApiUsage
				if wanted == "experimental" {
					message = tsdiag.X_0_is_an_experimental_API_effect_experimentalApiUsage
				}
				diagnostics = append(diagnostics, ctx.NewDiagnostic(ctx.SourceFile, ctx.GetErrorRange(node), message, nil, name))
			}
		}
		node.ForEachChild(walk)
		return false
	}
	walk(ctx.SourceFile.AsNode())
	return diagnostics
}

func symbolHasDeclaration(symbol *ast.Symbol, declaration *ast.Node) bool {
	if symbol == nil || declaration == nil {
		return false
	}
	return slices.Contains(symbol.Declarations, declaration)
}

// stabilitySymbolIsModuleExport reports whether symbol is exported from the
// module under its own name.
func stabilitySymbolIsModuleExport(c *checker.Checker, symbol *ast.Symbol, moduleSymbol *ast.Symbol) bool {
	if symbol == nil || moduleSymbol == nil {
		return false
	}
	return c.TryGetMemberInModuleExportsAndProperties(symbol.Name, moduleSymbol) == symbol
}

// stabilityOwningExportSymbol walks out from a selected declaration to the
// nearest enclosing declaration that is exported from the module. A call or
// construct signature of a dual export has its own `__call` symbol, so the
// export it belongs to can only be found by climbing to the annotated
// declaration (for example the variable the object type is assigned to).
func stabilityOwningExportSymbol(c *checker.Checker, declaration *ast.Node, moduleSymbol *ast.Symbol) *ast.Symbol {
	for node := declaration.Parent; node != nil; node = node.Parent {
		symbol := checker.Checker_getSymbolOfDeclaration(c, node)
		if symbol == nil {
			continue
		}
		if symbol.ExportSymbol != nil {
			symbol = symbol.ExportSymbol
		}
		if stabilitySymbolIsModuleExport(c, symbol, moduleSymbol) {
			return symbol
		}
	}
	return nil
}

func stabilityOfDeclaration(declaration *ast.Node) string {
	if declaration == nil {
		return ""
	}
	// A variable's JSDoc is usually attached to its VariableStatement.
	nodes := []*ast.Node{declaration}
	if declaration.Parent != nil && declaration.Parent.Kind == ast.KindVariableDeclarationList && declaration.Parent.Parent != nil && declaration.Parent.Parent.Kind == ast.KindVariableStatement {
		nodes = append(nodes, declaration.Parent.Parent)
	}
	for _, node := range nodes {
		if node.Flags&ast.NodeFlagsPossiblyContainsStabilityTag == 0 {
			continue
		}
		for _, doc := range node.JSDoc(nil) {
			if doc.AsJSDoc().Tags == nil {
				continue
			}
			for _, tag := range doc.AsJSDoc().Tags.Nodes {
				if tag.Kind != ast.KindJSDocUnknownTag || tag.TagName().Text() != "stability" {
					continue
				}
				comment := tag.AsJSDocUnknownTag().Comment
				if comment != nil && len(comment.Nodes) > 0 && comment.Nodes[0].Kind == ast.KindJSDocText {
					value := strings.TrimSpace(comment.Nodes[0].Text())
					if value == "unstable" || value == "experimental" {
						return value
					}
				}
			}
		}
	}
	return ""
}

// Decisions belong to one rule invocation: per-file overrides can change the
// allow-list, and the tagged declaration distinguishes overloads and reexports.
func newStabilityApiAllowlist(ctx *rule.Context, wanted string) func(*ast.Node) (bool, string) {
	type decision struct {
		allowed bool
		name    string
	}
	decisions := make(map[*ast.Node]decision)
	modules := make(map[*ast.SourceFile]string)
	var entries []string
	if ctx.Options != nil {
		entries = ctx.Options.AllowedUnstableApis
		if wanted == "experimental" {
			entries = ctx.Options.AllowedExperimentalApis
		}
	}
	return func(declaration *ast.Node) (bool, string) {
		if cached, ok := decisions[declaration]; ok {
			return cached.allowed, cached.name
		}
		result := decision{}
		defer func() { decisions[declaration] = result }()
		if declaration == nil {
			return false, ""
		}
		sf := ast.GetSourceFileOfNode(declaration)
		if sf == nil {
			return false, ""
		}
		moduleName, ok := modules[sf]
		if !ok {
			pkg := ctx.TypeParser.PackageJsonForSourceFile(sf)
			if pkg != nil {
				if packageName, ok := pkg.Name.GetValue(); ok && packageName != "" {
					directory := getPackageJsonDirectory(ctx.Program, ctx.Checker, sf)
					moduleName = stabilityApiModuleName(packageName, directory, sf.FileName())
				}
			}
			modules[sf] = moduleName
		}
		if moduleName == "" {
			return false, ""
		}
		moduleSymbol := checker.Checker_getSymbolOfDeclaration(ctx.Checker, sf.AsNode())
		symbol := checker.Checker_getSymbolOfDeclaration(ctx.Checker, declaration)
		if symbol != nil && symbol.ExportSymbol != nil {
			symbol = symbol.ExportSymbol
		}
		// A selected overload (for example a call signature of a dual export)
		// carries its own `__call` symbol rather than the export's, so it is not
		// a module export. Resolve the enclosing exported declaration instead so
		// the name and the per-export allow-list match stay export-scoped.
		if moduleSymbol != nil && !stabilitySymbolIsModuleExport(ctx.Checker, symbol, moduleSymbol) {
			if owner := stabilityOwningExportSymbol(ctx.Checker, declaration, moduleSymbol); owner != nil {
				symbol = owner
			}
		}
		result.name = moduleName
		if moduleSymbol != nil && symbol != nil {
			exported := ctx.Checker.TryGetMemberInModuleExportsAndProperties(symbol.Name, moduleSymbol)
			if exported == symbol {
				result.name += "#" + symbol.Name
			}
		}
		for _, entry := range entries {
			module, member, hasMember := strings.Cut(entry, "#")
			if !hasMember {
				if module != "" && (moduleName == module || strings.HasPrefix(moduleName, module+"/")) {
					result.allowed = true
					break
				}
			} else if module == moduleName && member != "" && moduleSymbol != nil && symbol != nil {
				exported := ctx.Checker.TryGetMemberInModuleExportsAndProperties(member, moduleSymbol)
				if exported != nil && (exported == symbol || checker.Checker_getSymbolIfSameReference(ctx.Checker, resolveStabilityAlias(ctx.Checker, exported), resolveStabilityAlias(ctx.Checker, symbol)) != nil) {
					result.allowed = true
					break
				}
			}
		}
		return result.allowed, result.name
	}
}

// This is a declaration path, not a reconstructed public import specifier.
func stabilityApiModuleName(packageName, directory, fileName string) string {
	if directory == "" {
		return ""
	}
	directory = strings.TrimSuffix(tspath.NormalizePath(directory), "/") + "/"
	fileName = tspath.NormalizePath(fileName)
	if !strings.HasPrefix(fileName, directory) {
		return ""
	}
	relative := strings.TrimPrefix(fileName, directory)
	for _, prefix := range []string{"dist/dts/", "dist/esm/", "dist/cjs/", "src/", "dist/"} {
		if rest, ok := strings.CutPrefix(relative, prefix); ok {
			relative = rest
			break
		}
	}
	for _, extension := range []string{".d.ts", ".d.mts", ".d.cts", ".ts", ".mts", ".cts", ".tsx", ".js", ".mjs", ".cjs", ".jsx"} {
		if rest, ok := strings.CutSuffix(relative, extension); ok {
			relative = rest
			break
		}
	}
	if tspath.GetBaseFileName(relative) == "index" {
		relative = strings.TrimSuffix(strings.TrimSuffix(relative, "index"), "/")
	}
	if relative == "" {
		return packageName
	}
	return packageName + "/" + relative
}

// Synthetic default aliases have no alias declaration and cannot be resolved.
func resolveStabilityAlias(c *checker.Checker, symbol *ast.Symbol) *ast.Symbol {
	for depth := 0; symbol != nil && symbol.Flags&ast.SymbolFlagsAlias != 0 && depth < 32; depth++ {
		if !slices.ContainsFunc(symbol.Declarations, ast.IsAliasSymbolDeclaration) {
			return nil
		}
		next := c.GetImmediateAliasedSymbol(symbol)
		if next == symbol {
			break
		}
		symbol = next
	}
	return symbol
}
