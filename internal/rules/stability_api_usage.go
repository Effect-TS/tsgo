package rules

import (
	"slices"
	"strings"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
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
	// Declared stability is cached per checker in TypeParser, so references to the
	// same symbol or selected overload reuse one lookup across source files. The
	// selected overload's own tag always wins over the symbol-level tag.
	readSymbol := func(symbol *ast.Symbol) declarationStabilityInfo {
		if symbol == nil {
			return declarationStabilityInfo{}
		}
		info := ctx.TypeParser.DeclaredApiStabilityOfSymbol(symbol)
		return declarationStabilityInfo{stability: typeparser.ApiStabilityLevelTag(info.Level), declaration: info.Declaration}
	}
	readSignature := func(signature *checker.Signature) declarationStabilityInfo {
		if signature == nil {
			return declarationStabilityInfo{}
		}
		info := ctx.TypeParser.DeclaredApiStabilityOfSignature(signature)
		return declarationStabilityInfo{stability: typeparser.ApiStabilityLevelTag(info.Level), declaration: info.Declaration}
	}

	allow := newStabilityApiAllowlist(ctx, wanted)
	var diagnostics []*ast.Diagnostic
	report := func(node *ast.Node, name string, stability declarationStabilityInfo) bool {
		if stability.stability != wanted {
			return false
		}
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
		return true
	}
	var walk ast.Visitor
	walk = func(node *ast.Node) bool {
		if node == nil {
			return false
		}
		// Object literal keys declare local properties, but also use the matching
		// properties of their contextual type. Resolve those declarations before
		// the ordinary reference path excludes declaration names.
		if ast.IsObjectLiteralElement(node) && node.Name() != nil && node.Parent != nil && node.Parent.Kind == ast.KindObjectLiteralExpression {
			if name := ast.GetTextOfPropertyName(node.Name()); name != "" {
				if contextualType := ctx.Checker.GetContextualType(node.Parent, checker.ContextFlagsNone); contextualType != nil {
					// Keep the original symbols and let the checker filter union
					// branches by discriminants before reading their own tags.
					properties := ctx.Checker.GetPropertySymbolsFromContextualType(node, contextualType, false)
					// Generic inference can point back to the literal's own
					// property. Use the constraint instead, as go-to-definition
					// does, so its stability tag is not lost to inference.
					if slices.ContainsFunc(properties, func(symbol *ast.Symbol) bool { return symbol.ValueDeclaration == node }) {
						if constraintType := ctx.Checker.GetContextualType(node.Parent, checker.ContextFlagsIgnoreNodeInferences); constraintType != nil {
							if constraintProperties := ctx.Checker.GetPropertySymbolsFromContextualType(node, constraintType, false); len(constraintProperties) > 0 {
								properties = constraintProperties
							}
						}
					}
					for _, symbol := range properties {
						if symbol.ValueDeclaration == node {
							continue
						}
						if report(node.Name(), name, readSymbol(symbol)) {
							break
						}
					}
				}
			}
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
					stability = readSignature(signature)
					if stability.stability == "" {
						// A signature with no own tag still carries the selected
						// declaration so the symbol fallback can be suppressed for it.
						stability = declarationStabilityInfo{declaration: selectedDeclaration}
					}
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
			report(node, node.Text(), stability)
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
					moduleName = stabilityApiModuleName(packageName, directory, string(sf.FileName()))
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
	seen := make(map[*ast.Symbol]bool)
	for symbol != nil && symbol.Flags&ast.SymbolFlagsAlias != 0 {
		if seen[symbol] {
			return nil
		}
		seen[symbol] = true
		if !slices.ContainsFunc(symbol.Declarations, ast.IsAliasSymbolDeclaration) {
			return nil
		}
		next := typeparser.ApiStabilityImmediateAliasedSymbol(c, symbol)
		if next == symbol {
			break
		}
		symbol = next
	}
	return symbol
}
