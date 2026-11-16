package typeparser

import (
	"slices"
	"testing"

	"github.com/effect-ts/tsgo/internal/readonly"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// identifierOccurrences maps each reference to its occurrence index among the
// identifiers named text in sf (the index findIdentifierByText takes), and
// checks that the references are identifiers in source order.
func identifierOccurrences(t *testing.T, sf *ast.SourceFile, text string, references readonly.Slice[*ast.Node]) []int {
	t.Helper()
	var occurrences []int
	previousEnd := -1
	for _, reference := range references.All() {
		if reference.Kind != ast.KindIdentifier || reference.Text() != text {
			t.Fatalf("expected identifier %q, got %v %q", text, reference.Kind, reference.Text())
		}
		if reference.Pos() < previousEnd {
			t.Fatalf("references are not in source order")
		}
		previousEnd = reference.End()
		occurrence := -1
		for i := 0; ; i++ {
			candidate := findIdentifierByText(t, sf, text, i)
			if candidate == reference {
				occurrence = i
				break
			}
			if candidate.Pos() > reference.Pos() {
				break
			}
		}
		occurrences = append(occurrences, occurrence)
	}
	return occurrences
}

func expectOccurrences(t *testing.T, got []int, want ...int) {
	t.Helper()
	if !slices.Equal(got, want) {
		t.Fatalf("expected references at occurrences %v, got %v", want, got)
	}
}

func TestReferencesToSymbol_Identifiers(t *testing.T) {
	t.Parallel()

	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `export function f(e: number) {
  return e + e
}`)
	defer done()

	symbol := tp.GetSymbolAtLocation(findIdentifierByText(t, sf, "e", 0))
	if symbol == nil {
		t.Fatal("expected parameter symbol")
	}
	expectOccurrences(t, identifierOccurrences(t, sf, "e", tp.ReferencesToSymbol(sf.AsNode(), symbol)), 0, 1, 2)
}

func TestReferencesToSymbol_ShorthandProperty(t *testing.T) {
	t.Parallel()

	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `export const f = (e: number) => ({ e, other: e })`)
	defer done()

	parameter := findIdentifierByText(t, sf, "e", 0)
	symbol := tp.GetSymbolAtLocation(parameter)
	if symbol == nil {
		t.Fatal("expected parameter symbol")
	}
	body := parameter.Parent.Parent.Body()
	references := tp.ReferencesToSymbol(body, symbol)
	expectOccurrences(t, identifierOccurrences(t, sf, "e", references), 1, 2)
	if parent := references.At(0).Parent; parent == nil || parent.Kind != ast.KindShorthandPropertyAssignment {
		t.Fatal("expected the first reference to be the shorthand property name")
	}
}

func TestReferencesToSymbol_ImportAlias(t *testing.T) {
	t.Parallel()

	c, tp, sourceFiles, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/a.ts": `export const Foo = 1`,
		"/.src/b.ts": `import { Foo as Bar } from "./a"
export const value = Bar
export const object = { Bar }`,
	})
	defer done()

	a := sourceFiles["/.src/a.ts"]
	b := sourceFiles["/.src/b.ts"]
	moduleSymbol := checker.Checker_getSymbolOfDeclaration(c, a.AsNode())
	if moduleSymbol == nil {
		t.Fatal("expected module symbol for a.ts")
	}
	exportFoo := c.TryGetMemberInModuleExportsAndProperties("Foo", moduleSymbol)
	if exportFoo == nil {
		t.Fatal("expected exported Foo symbol")
	}

	// Skip the import declaration, whose specifier names also resolve to Foo.
	statements := b.Statements.Nodes
	var occurrences []int
	for _, statement := range statements[1:] {
		occurrences = append(occurrences, identifierOccurrences(t, b, "Bar", tp.ReferencesToSymbol(statement, exportFoo))...)
	}
	expectOccurrences(t, occurrences, 1, 2)
}

func TestReferencesToSymbol_MergedExport(t *testing.T) {
	t.Parallel()

	c, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `export const Foo = 1
export const Bar = Foo + 1`)
	defer done()

	moduleSymbol := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
	if moduleSymbol == nil {
		t.Fatal("expected module symbol")
	}
	exportFoo := c.TryGetMemberInModuleExportsAndProperties("Foo", moduleSymbol)
	if exportFoo == nil {
		t.Fatal("expected exported Foo symbol")
	}
	expectOccurrences(t, identifierOccurrences(t, sf, "Foo", tp.ReferencesToSymbol(sf.AsNode(), exportFoo)), 0, 1)
}

func TestReferencesToSymbol_ShadowedBindingIsNotAReference(t *testing.T) {
	t.Parallel()

	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `export function f(e: number) {
  const g = (e: string) => ({ e, length: e.length })
  return [e, g]
}`)
	defer done()

	symbol := tp.GetSymbolAtLocation(findIdentifierByText(t, sf, "e", 0))
	if symbol == nil {
		t.Fatal("expected parameter symbol")
	}
	expectOccurrences(t, identifierOccurrences(t, sf, "e", tp.ReferencesToSymbol(sf.AsNode(), symbol)), 0, 4)
}

func TestReferencesToSymbol_NilInputs(t *testing.T) {
	t.Parallel()

	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `export const e = 1`)
	defer done()

	symbol := tp.GetSymbolAtLocation(findIdentifierByText(t, sf, "e", 0))
	if symbol == nil {
		t.Fatal("expected symbol")
	}
	if references := tp.ReferencesToSymbol(nil, symbol); references.Len() != 0 {
		t.Fatalf("expected no references for a nil root, got %d", references.Len())
	}
	if references := tp.ReferencesToSymbol(sf.AsNode(), nil); references.Len() != 0 {
		t.Fatalf("expected no references for a nil symbol, got %d", references.Len())
	}
}
