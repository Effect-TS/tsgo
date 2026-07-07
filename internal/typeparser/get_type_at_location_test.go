package typeparser

import (
	"testing"

	"github.com/microsoft/typescript-go/shim/ast"
)

// Import clauses without a default binding declare no symbol; before the
// KindImportClause guard, checker.GetTypeAtLocation panicked on them with a
// nil symbol dereference (recovered silently, see issue #301 investigation).
func TestGetTypeAtLocationImportClause(t *testing.T) {
	t.Parallel()

	sources := map[string]string{
		"/.src/dep.ts": `
export const A = 1
export type T = string
const d = { A }
export default d
`,
		"/.src/main.ts": `
import { A } from "./dep.js"
import * as ns from "./dep.js"
import type { T } from "./dep.js"
import d from "./dep.js"
export const use: T = String(A + ns.A + d.A)
`,
	}

	_, tp, sourceFiles, done := compileAndGetCheckerAndSourceFilesInternal(t, sources)
	defer done()

	var clauses []*ast.Node
	var visit func(node *ast.Node) bool
	visit = func(node *ast.Node) bool {
		if node.Kind == ast.KindImportClause {
			clauses = append(clauses, node)
		}
		node.ForEachChild(visit)
		return false
	}
	sourceFiles["/.src/main.ts"].AsNode().ForEachChild(visit)

	if len(clauses) != 4 {
		t.Fatalf("expected 4 import clauses, got %d", len(clauses))
	}

	for _, clause := range clauses {
		if got := tp.GetTypeAtLocation(clause); got != nil {
			t.Errorf("expected nil type for import clause %q, got %v", clause.Parent.Text(), got)
		}
	}
}
