package customlint

import (
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"golang.org/x/tools/go/analysis/analysistest"
)

func TestRawSymbolAccess(t *testing.T) {
	fields := []string{"Name", "Declarations", "Flags", "CheckFlags", "ValueDeclaration", "Members", "Exports", "Parent", "ExportSymbol"}
	for _, shape := range []string{"fields", "methods"} {
		t.Run(shape, func(t *testing.T) {
			var native, reads strings.Builder
			native.WriteString("package ast\ntype Node struct { Name string; Flags int }\ntype Symbol struct {\n")
			if shape == "fields" {
				for _, name := range fields {
					fmt.Fprintf(&native, "%s string\n", name)
				}
			}
			native.WriteString("}\n")
			for _, name := range fields {
				if shape == "methods" {
					fmt.Fprintf(&native, "func (*Symbol) %s() string {return \"value\"}\n", name)
				}
				fmt.Fprintf(&reads, "_ = s.%s // want \"use ast.TSGetSymbol%s\"\n", name, name)
			}
			native.WriteString("func (*Symbol) IsStatic() bool {return false}\nfunc SymbolName(*Symbol) string {return \"name\"}\n")
			extra := `_ = alias.Name // want "use ast.TSGetSymbolName"
_ = promoted.Name // want "use ast.TSGetSymbolName"
_ = anonymous.Name // want "use ast.TSGetSymbolName"
_ = old.Name // want "use ast.TSGetSymbolName"
`
			if shape == "methods" {
				extra += `_ = s.Name() // want "use ast.TSGetSymbolName"
_ = (*compiler.Symbol).Name // want "use ast.TSGetSymbolName"
_ = (*Promoted).Name // want "use ast.TSGetSymbolName"
`
			}
			consumer := `package consumer
import (
 compiler "github.com/microsoft/TypeScript/tsc/shim/ast"
 legacy "github.com/microsoft/typescript-go/shim/ast"
)
type Alias = compiler.Symbol
type Promoted struct { *Alias }
type Symbol struct { Name string; Flags int }
func uses(s *compiler.Symbol, old *legacy.Symbol, alias *Alias, promoted *Promoted, node *compiler.Node, local *Symbol, anonymous struct {*Alias}) {
` + reads.String() + extra + `
_ = node.Name
_ = node.Flags
_ = local.Name
_ = local.Flags
_ = compiler.SymbolName(s)
_ = s.IsStatic()
_ = compiler.TSGetSymbolName(s)
}
`
			files := map[string]string{
				"github.com/microsoft/TypeScript/tsc/internal/ast/ast.go": native.String(),
				"github.com/microsoft/typescript-go/internal/ast/ast.go":  native.String(),
				"github.com/microsoft/TypeScript/tsc/shim/ast/ast.go": `package ast
import raw "github.com/microsoft/TypeScript/tsc/internal/ast"
type Symbol = raw.Symbol
type Node = raw.Node
func SymbolName(s *Symbol) string {return raw.SymbolName(s)}
func TSGetSymbolName(s *Symbol) string {return raw.SymbolName(s)}
`,
				"github.com/microsoft/typescript-go/shim/ast/ast.go": `package ast
import raw "github.com/microsoft/typescript-go/internal/ast"
type Symbol = raw.Symbol
`,
				"github.com/effect-ts/tsgo/internal/consumer/consumer.go": consumer,
				"github.com/effect-ts/tsgo/etsgoapi/consumer.go":          strings.ReplaceAll(consumer, "package consumer", "package etsgoapi"),
				"github.com/effect-ts/tsgo/internal/consumer/external_test.go": `package consumer_test
import "github.com/microsoft/TypeScript/tsc/shim/ast"
func external(s *ast.Symbol){_ = s.Name} // want "use ast.TSGetSymbolName"
`,
				"github.com/other/consumer/consumer.go": `package consumer
import "github.com/microsoft/TypeScript/tsc/shim/ast"
func allowed(s *ast.Symbol){_ = s.Name}
`,
			}
			dir, cleanup, err := analysistest.WriteFiles(files)
			if err != nil {
				t.Fatal(err)
			}
			defer cleanup()
			analysistest.Run(t, dir, symbolAccessAnalyzer, "github.com/effect-ts/tsgo/internal/consumer", "github.com/effect-ts/tsgo/etsgoapi", "github.com/microsoft/TypeScript/tsc/shim/ast", "github.com/other/consumer")
		})
	}
}

func TestSymbolAccessMatchesCompatibilityHelpers(t *testing.T) {
	for _, provider := range []string{"typescript", "typescript-go"} {
		t.Run(provider, func(t *testing.T) {
			source, err := os.ReadFile(filepath.Join("..", "gen_shims", "providers", provider, "ast", "compatibility.go"))
			if err != nil {
				t.Fatal(err)
			}
			file, err := parser.ParseFile(token.NewFileSet(), "compatibility.go", source, 0)
			if err != nil {
				t.Fatal(err)
			}
			found := map[string]bool{}
			for _, decl := range file.Decls {
				helper, ok := decl.(*ast.FuncDecl)
				if !ok || !strings.HasPrefix(helper.Name.Name, "TSGetSymbol") || !isSymbolPointerGetter(helper) {
					continue
				}
				member := strings.TrimPrefix(helper.Name.Name, "TSGetSymbol")
				if !rawSymbolMembers[member] {
					t.Errorf("raw Symbol.%s is not enforced by symbolaccess", member)
				}
				found[member] = true
			}
			for member := range rawSymbolMembers {
				if !found[member] {
					t.Errorf("Symbol.%s has no %s compatibility helper", member, provider)
				}
			}
		})
	}
}

func isSymbolPointerGetter(helper *ast.FuncDecl) bool {
	params := helper.Type.Params.List
	if len(params) != 1 {
		return false
	}
	star, ok := params[0].Type.(*ast.StarExpr)
	if !ok {
		return false
	}
	selector, ok := star.X.(*ast.SelectorExpr)
	return ok && selector.Sel.Name == "Symbol"
}
