package main

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func symbolFixture(shape string) string {
	fields := []struct{ name, kind, value string }{
		{"Name", "string", `"raw-name"`},
		{"Declarations", "[]*Node", "[]*Node{node}"},
		{"Flags", "SymbolFlags", "3"},
		{"CheckFlags", "CheckFlags", "5"},
		{"ValueDeclaration", "*Node", "node"},
		{"Members", "SymbolTable", "SymbolTable{\"member\": parent}"},
		{"Exports", "SymbolTable", "SymbolTable{\"export\": parent}"},
		{"Parent", "*Symbol", "parent"},
		{"ExportSymbol", "*Symbol", "parent"},
	}
	var source, getters, values strings.Builder
	source.WriteString("package ast\ntype Node struct{ Value int }; type SymbolFlags uint32; type CheckFlags uint32; type SymbolTable map[string]*Symbol\ntype Symbol struct {\n")
	for _, field := range fields {
		name := field.name
		if shape == "methods" {
			name = strings.ToLower(name[:1]) + name[1:]
			fmt.Fprintf(&getters, "func (s *Symbol) %s() %s { return s.%s }\n", field.name, field.kind, name)
		}
		fmt.Fprintf(&source, "%s %s\n", name, field.kind)
		fmt.Fprintf(&values, "%s: %s,\n", name, field.value)
	}
	source.WriteString("}\n")
	source.WriteString(getters.String())
	fmt.Fprintf(&source, "func Sample() *Symbol { node:=&Node{Value: 17}; parent:=&Symbol{}; return &Symbol{%s} }\n", values.String())
	source.WriteString("func Empty() *Symbol { return &Symbol{} }\n")
	return source.String()
}

func TestSymbolCompatibilityPreservesRawValues(t *testing.T) {
	for _, provider := range []string{"typescript", "typescript-go"} {
		authored, err := os.ReadFile(filepath.Join("providers", provider, "ast", "compatibility.go"))
		if err != nil {
			t.Fatal(err)
		}
		authored, err = providerSymbolCompatibility(authored)
		if err != nil {
			t.Fatal(err)
		}
		shape := "fields"
		if provider == "typescript" {
			shape = "methods"
		}
		t.Run(provider+"/"+shape, func(t *testing.T) {
			source := symbolFixture(shape)
			compatibility := bytes.ReplaceAll(authored, []byte("github.com/microsoft/typescript-go/internal/ast"), []byte("example.com/provider/internal/ast"))
			root := t.TempDir()
			writeTestFile(t, filepath.Join(root, "go.mod"), []byte("module example.com/provider\n\ngo 1.27\n"))
			writeTestFile(t, filepath.Join(root, "internal", "ast", "symbol.go"), []byte(source))
			writeTestFile(t, filepath.Join(root, "shim", "ast", "compatibility.go"), compatibility)
			writeTestFile(t, filepath.Join(root, "shim", "ast", "compatibility_test.go"), []byte(`package ast
import (
 "testing"
 raw "example.com/provider/internal/ast"
)
func TestRawSymbolValues(t *testing.T) {
 s:=raw.Sample()
 if TSGetSymbolName(s)!="raw-name" || TSGetSymbolFlags(s)!=3 || TSGetSymbolCheckFlags(s)!=5 {t.Fatal("incorrect scalar values")}
 declarations:=TSGetSymbolDeclarations(s)
 if len(declarations)!=1 || declarations[0].Value!=17 {t.Fatal("incorrect declarations")}
 declarations[0]=&raw.Node{Value:42}
 if TSGetSymbolDeclarations(s)[0].Value!=42 {t.Fatal("copied declarations")}
 if TSGetSymbolValueDeclaration(s).Value!=17 {t.Fatal("incorrect value declaration")}
 parent:=TSGetSymbolParent(s)
 if parent==nil || TSGetSymbolExportSymbol(s)!=parent {t.Fatal("changed pointer identity")}
 members:=TSGetSymbolMembers(s); exports:=TSGetSymbolExports(s)
 if members["member"]!=parent || exports["export"]!=parent {t.Fatal("incorrect tables")}
 members["added"]=s; exports["added"]=s
 if TSGetSymbolMembers(s)["added"]!=s || TSGetSymbolExports(s)["added"]!=s {t.Fatal("copied tables")}
 empty:=raw.Empty()
 if TSGetSymbolDeclarations(empty)!=nil || TSGetSymbolMembers(empty)!=nil || TSGetSymbolExports(empty)!=nil {t.Fatal("initialized empty collections")}
 if TSGetSymbolParent(empty)!=nil || TSGetSymbolExportSymbol(empty)!=nil || TSGetSymbolValueDeclaration(empty)!=nil {t.Fatal("changed empty pointers")}
}
`))
			command := exec.Command("go", "test", "./...")
			command.Dir = root
			command.Env = append(os.Environ(), "GOWORK=off")
			if output, err := command.CombinedOutput(); err != nil {
				t.Fatalf("exercise generated %s helpers: %v\n%s", shape, err, output)
			}
		})
	}
}

func TestProviderSymbolCompatibilityKeepsCanonicalOverrides(t *testing.T) {
	source, err := os.ReadFile(filepath.Join("providers", "typescript-go", "ast", "compatibility.go"))
	if err != nil {
		t.Fatal(err)
	}
	provider, adapters, err := splitShimHelpers(map[string][]byte{"ast/compatibility.go": source})
	if err != nil {
		t.Fatal(err)
	}
	if bytes.Contains(provider["ast/compatibility.go"], []byte("NewSourceFileParseOptions")) || bytes.Contains(provider["ast/compatibility.go"], []byte("tspath")) {
		t.Fatal("provider received canonical overrides")
	}
	if !bytes.Contains(provider["ast/compatibility.go"], []byte("TSGetSymbolName")) || !bytes.Contains(adapters["ast/compatibility.go"], []byte("NewSourceFileParseOptions")) {
		t.Fatal("lost Symbol helper or canonical override")
	}
}

func TestBackportSymbolCompatibilityCompilesProviderAndFacade(t *testing.T) {
	root := t.TempDir()
	providerRoot := filepath.Join(root, "provider")
	facadeRoot := filepath.Join(root, "facade")
	writeTestFile(t, filepath.Join(root, "go.mod"), []byte("module example.com/provider\n\ngo 1.27\n"))
	writeTestFile(t, filepath.Join(root, "internal", "ast", "symbol.go"), []byte(symbolFixture("fields")+"\nfunc Version() string {return \"native\"}\n"))
	writeTestFile(t, filepath.Join(providerRoot, "ast", "go.mod"), []byte("module example.com/provider/shim/ast\n\ngo 1.27\n\nrequire example.com/provider v0.0.0\n"))
	writeTestFile(t, filepath.Join(providerRoot, "ast", "shim.go"), []byte(`package ast
import raw "example.com/provider/internal/ast"
type Symbol = raw.Symbol
type Node = raw.Node
type SymbolFlags = raw.SymbolFlags
type CheckFlags = raw.CheckFlags
type SymbolTable = raw.SymbolTable
var Sample = raw.Sample
var Version = raw.Version
`))
	source, err := os.ReadFile(filepath.Join("providers", "typescript-go", "ast", "compatibility.go"))
	if err != nil {
		t.Fatal(err)
	}
	source, err = providerSymbolCompatibility(source)
	if err != nil {
		t.Fatal(err)
	}
	source = append(source, []byte("\nfunc Version() string {return \"canonical\"}\n")...)
	source = bytes.ReplaceAll(source, []byte("github.com/microsoft/typescript-go/internal/ast"), []byte("example.com/provider/internal/ast"))
	provider, adapters, err := splitShimHelpers(map[string][]byte{"ast/compatibility.go": source})
	if err != nil {
		t.Fatal(err)
	}
	if err := writeShimHelpers(providerRoot, provider); err != nil {
		t.Fatal(err)
	}
	if err := generateBackport(providerRoot, facadeRoot, "example.com/provider/internal/", "example.com/provider/shim", adapters); err != nil {
		t.Fatal(err)
	}
	writeTestFile(t, filepath.Join(root, "go.work"), []byte("go 1.27\n\nuse (\n .\n ./provider/ast\n ./facade/ast\n)\n"))
	writeTestFile(t, filepath.Join(facadeRoot, "ast", "compatibility_test.go"), []byte(`package ast
import (
 "testing"
 provider "example.com/provider/shim/ast"
)
func TestSameNativeSymbol(t *testing.T) {
 s:=provider.Sample()
 if provider.Version()!="native" || Version()!="canonical" {t.Fatal("changed provider API or lost canonical override")}
 if TSGetSymbolName(s)!="raw-name" || provider.TSGetSymbolName(s)!="raw-name" {t.Fatal("lost provider or facade getter")}
 if TSGetSymbolParent(s)!=provider.TSGetSymbolParent(s) {t.Fatal("changed native Symbol identity")}
 if TSGetSymbolDeclarations(s)[0]!=provider.TSGetSymbolDeclarations(s)[0] {t.Fatal("changed native declaration identity")}
 TSGetSymbolMembers(s)["facade"]=s
 if provider.TSGetSymbolMembers(s)["facade"]!=s {t.Fatal("facade copied native members")}
}
`))
	command := exec.Command("go", "test", "./provider/ast/...", "./facade/ast/...")
	command.Dir = root
	command.Env = append(os.Environ(), "GOWORK="+filepath.Join(root, "go.work"))
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("compile provider and canonical Symbol helpers: %v\n%s", err, output)
	}
}
