// Command port_manifest lists the declarations of Go packages, for `repoctl tsrs port-*` (the Rust port bookkeeping).
//
// It parses source only (no type checking, so it needs neither the dependencies nor the compiler shims) and prints
// one JSON object per declaration on stdout:
//
//	{"file":"internal/typeparser/effect_type.go","line":36,"endLine":60,"name":"TypeParser.EffectType",
//	 "kind":"method","generated":false,"hash":"…"}
//
// file is relative to -root. name is the function name, Recv.Name for a method (pointer and type parameters
// dropped), the type name for a type, or the first name of a const or var declaration (a parenthesized group is one
// entry). kind is func, method, type, const, var, test (Test/Benchmark/Fuzz/Example in a _test.go file) or
// testhelper (any other declaration in a _test.go file). hash is a truncated SHA-256 of the declaration's source
// text including its doc comment, so a changed body or comment shows up as a changed declaration.
//
// Usage: GOWORK=off go run . -root <repo> <package dir relative to root>...
package main

import (
	"bufio"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"flag"
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

type decl struct {
	File      string `json:"file"`
	Line      int    `json:"line"`
	EndLine   int    `json:"endLine"`
	Name      string `json:"name"`
	Kind      string `json:"kind"`
	Generated bool   `json:"generated"`
	Hash      string `json:"hash"`
}

func main() {
	root := flag.String("root", ".", "repository root; file paths are printed relative to it")
	flag.Parse()
	out := bufio.NewWriter(os.Stdout)
	defer out.Flush()
	enc := json.NewEncoder(out)
	for _, dir := range flag.Args() {
		if err := listPackage(*root, dir, enc); err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}
	}
}

func listPackage(root, dir string, enc *json.Encoder) error {
	entries, err := os.ReadDir(filepath.Join(root, filepath.FromSlash(dir)))
	if err != nil {
		return err
	}
	names := make([]string, 0, len(entries))
	for _, entry := range entries {
		if !entry.IsDir() && strings.HasSuffix(entry.Name(), ".go") {
			names = append(names, entry.Name())
		}
	}
	sort.Strings(names)
	for _, name := range names {
		rel := name
		if dir != "." {
			rel = strings.TrimSuffix(filepath.ToSlash(dir), "/") + "/" + name
		}
		if err := listFile(root, rel, enc); err != nil {
			return err
		}
	}
	return nil
}

func listFile(root, rel string, enc *json.Encoder) error {
	src, err := os.ReadFile(filepath.Join(root, filepath.FromSlash(rel)))
	if err != nil {
		return err
	}
	fset := token.NewFileSet()
	file, err := parser.ParseFile(fset, rel, src, parser.ParseComments|parser.SkipObjectResolution)
	if err != nil {
		return err
	}
	generated := ast.IsGenerated(file)
	isTest := strings.HasSuffix(rel, "_test.go")
	emit := func(node ast.Node, doc *ast.CommentGroup, name, kind string) error {
		if isTest && kind != "test" {
			kind = "testhelper"
		}
		start := node.Pos()
		if doc != nil {
			start = doc.Pos()
		}
		text := src[fset.Position(start).Offset:fset.Position(node.End()).Offset]
		sum := sha256.Sum256(text)
		return enc.Encode(decl{
			File:      rel,
			Line:      fset.Position(node.Pos()).Line,
			EndLine:   fset.Position(node.End()).Line,
			Name:      name,
			Kind:      kind,
			Generated: generated,
			Hash:      hex.EncodeToString(sum[:8]),
		})
	}
	for _, d := range file.Decls {
		if gen, ok := d.(*ast.GenDecl); ok {
			switch gen.Tok {
			case token.TYPE:
				for _, spec := range gen.Specs {
					ts := spec.(*ast.TypeSpec)
					doc := ts.Doc
					var node ast.Node = ts
					if len(gen.Specs) == 1 {
						doc, node = gen.Doc, gen
					}
					if err := emit(node, doc, ts.Name.Name, "type"); err != nil {
						return err
					}
				}
			case token.CONST, token.VAR:
				if len(gen.Specs) == 0 {
					continue
				}
				name := gen.Specs[0].(*ast.ValueSpec).Names[0].Name
				if err := emit(gen, gen.Doc, name, gen.Tok.String()); err != nil {
					return err
				}
			}
			continue
		}
		fn, ok := d.(*ast.FuncDecl)
		if !ok {
			continue
		}
		name, kind := fn.Name.Name, "func"
		if fn.Recv != nil && len(fn.Recv.List) > 0 {
			name, kind = receiverName(fn.Recv.List[0].Type)+"."+name, "method"
		}
		if isTest && fn.Recv == nil && isTestName(fn.Name.Name) {
			kind = "test"
		}
		if err := emit(fn, fn.Doc, name, kind); err != nil {
			return err
		}
	}
	return nil
}

func receiverName(expr ast.Expr) string {
	switch t := expr.(type) {
	case *ast.StarExpr:
		return receiverName(t.X)
	case *ast.IndexExpr:
		return receiverName(t.X)
	case *ast.IndexListExpr:
		return receiverName(t.X)
	case *ast.ParenExpr:
		return receiverName(t.X)
	case *ast.Ident:
		return t.Name
	}
	return "?"
}

func isTestName(name string) bool {
	for _, prefix := range []string{"Test", "Benchmark", "Fuzz", "Example"} {
		if strings.HasPrefix(name, prefix) {
			return true
		}
	}
	return false
}
