package rules

import (
	"github.com/effect-ts/effect-typescript-go/etscore"
	"github.com/effect-ts/effect-typescript-go/internal/rule"
	"github.com/effect-ts/effect-typescript-go/internal/typeparser"
	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
	"github.com/microsoft/typescript-go/shim/core"
	tsdiag "github.com/microsoft/typescript-go/shim/diagnostics"
	"github.com/microsoft/typescript-go/shim/scanner"
)

type moduleAlternative struct {
	Alternative string
	Package     string
	Module      string
}

var moduleAlternativesV3 = map[string]moduleAlternative{
	"fs":               {Alternative: "FileSystem", Module: "fs", Package: "@effect/platform"},
	"node:fs":          {Alternative: "FileSystem", Module: "fs", Package: "@effect/platform"},
	"fs/promises":      {Alternative: "FileSystem", Module: "fs", Package: "@effect/platform"},
	"node:fs/promises": {Alternative: "FileSystem", Module: "fs", Package: "@effect/platform"},
	"path":             {Alternative: "Path", Module: "path", Package: "@effect/platform"},
	"node:path":        {Alternative: "Path", Module: "path", Package: "@effect/platform"},
	"path/posix":       {Alternative: "Path", Module: "path", Package: "@effect/platform"},
	"node:path/posix":  {Alternative: "Path", Module: "path", Package: "@effect/platform"},
	"path/win32":       {Alternative: "Path", Module: "path", Package: "@effect/platform"},
	"node:path/win32":  {Alternative: "Path", Module: "path", Package: "@effect/platform"},
	"child_process":    {Alternative: "CommandExecutor", Module: "child_process", Package: "@effect/platform"},
	"node:child_process": {Alternative: "CommandExecutor", Module: "child_process", Package: "@effect/platform"},
}

var moduleAlternativesV4 = map[string]moduleAlternative{
	"fs":               {Alternative: "FileSystem", Module: "fs", Package: "effect"},
	"node:fs":          {Alternative: "FileSystem", Module: "fs", Package: "effect"},
	"fs/promises":      {Alternative: "FileSystem", Module: "fs", Package: "effect"},
	"node:fs/promises": {Alternative: "FileSystem", Module: "fs", Package: "effect"},
	"path":             {Alternative: "Path", Module: "path", Package: "effect"},
	"node:path":        {Alternative: "Path", Module: "path", Package: "effect"},
	"path/posix":       {Alternative: "Path", Module: "path", Package: "effect"},
	"node:path/posix":  {Alternative: "Path", Module: "path", Package: "effect"},
	"path/win32":       {Alternative: "Path", Module: "path", Package: "effect"},
	"node:path/win32":  {Alternative: "Path", Module: "path", Package: "effect"},
	"child_process":    {Alternative: "ChildProcess", Module: "child_process", Package: "effect"},
	"node:child_process": {Alternative: "ChildProcess", Module: "child_process", Package: "effect"},
}

var NodeBuiltinImport = rule.Rule{
	Name:            "nodeBuiltinImport",
	Group:           "effectNative",
	Description:     "Warns when importing Node.js built-in modules that have Effect-native counterparts",
	DefaultSeverity: etscore.SeverityOff,
	SupportedEffect: []string{"v3", "v4"},
	Codes:           []int32{tsdiag.Prefer_using_0_from_1_instead_of_the_Node_js_2_module_effect_nodeBuiltinImport.Code()},
	Run: func(ctx *rule.Context) []*ast.Diagnostic {
		matches := AnalyzeNodeBuiltinImport(ctx.Checker, ctx.SourceFile)
		diags := make([]*ast.Diagnostic, len(matches))
		for i, m := range matches {
			diags[i] = ctx.NewDiagnostic(
				m.SourceFile,
				m.Location,
				tsdiag.Prefer_using_0_from_1_instead_of_the_Node_js_2_module_effect_nodeBuiltinImport,
				nil,
				m.Alternative,
				m.Package,
				m.Module,
			)
		}
		return diags
	},
}

type NodeBuiltinImportMatch struct {
	SourceFile  *ast.SourceFile
	Location    core.TextRange
	Alternative string
	Package     string
	Module      string
}

func AnalyzeNodeBuiltinImport(c *checker.Checker, sf *ast.SourceFile) []NodeBuiltinImportMatch {
	alternatives := moduleAlternativesV4
	if typeparser.SupportedEffectVersion(c) == typeparser.EffectMajorV3 {
		alternatives = moduleAlternativesV3
	}

	var matches []NodeBuiltinImportMatch

	for _, stmt := range sf.Statements.Nodes {
		switch stmt.Kind {
		case ast.KindImportDeclaration:
			importDecl := stmt.AsImportDeclaration()
			if importDecl.ModuleSpecifier == nil || importDecl.ModuleSpecifier.Kind != ast.KindStringLiteral {
				continue
			}
			specifier := importDecl.ModuleSpecifier.AsStringLiteral().Text
			if alt, ok := alternatives[specifier]; ok {
				matches = append(matches, NodeBuiltinImportMatch{
					SourceFile:  sf,
					Location:    scanner.GetErrorRangeForNode(sf, importDecl.ModuleSpecifier),
					Alternative: alt.Alternative,
					Package:     alt.Package,
					Module:      alt.Module,
				})
			}

		case ast.KindVariableStatement:
			varStmt := stmt.AsVariableStatement()
			if varStmt.DeclarationList == nil {
				continue
			}
			declList := varStmt.DeclarationList.AsVariableDeclarationList()
			if declList.Declarations == nil {
				continue
			}
			for _, declNode := range declList.Declarations.Nodes {
				decl := declNode.AsVariableDeclaration()
				if decl.Initializer == nil || decl.Initializer.Kind != ast.KindCallExpression {
					continue
				}
				callExpr := decl.Initializer.AsCallExpression()
				if callExpr.Expression == nil || callExpr.Expression.Kind != ast.KindIdentifier {
					continue
				}
				if scanner.GetTextOfNode(callExpr.Expression) != "require" {
					continue
				}
				if callExpr.Arguments == nil || len(callExpr.Arguments.Nodes) != 1 {
					continue
				}
				arg := callExpr.Arguments.Nodes[0]
				if arg.Kind != ast.KindStringLiteral {
					continue
				}
				specifier := arg.AsStringLiteral().Text
				if alt, ok := alternatives[specifier]; ok {
					matches = append(matches, NodeBuiltinImportMatch{
						SourceFile:  sf,
						Location:    scanner.GetErrorRangeForNode(sf, arg),
						Alternative: alt.Alternative,
						Package:     alt.Package,
						Module:      alt.Module,
					})
				}
			}
		}
	}

	return matches
}
