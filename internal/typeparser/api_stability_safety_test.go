package typeparser

import (
	"context"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

const apiStabilityRecursiveAlias = "type Deep<T> = T extends string ? Deep<T> : T\n"

func TestApiStabilitySafetyCompoundOperands(t *testing.T) {
	t.Parallel()

	cases := []struct {
		name string
		dep  string
	}{
		{
			name: "anonymous object compound",
			dep: apiStabilityRecursiveAlias + `type Select<T> = { v: T } extends { v: string } ? Deep<string> : number
export declare const x: Select<string>`,
		},
		{
			name: "anonymous function compound",
			dep: apiStabilityRecursiveAlias + `type Select<T> = (() => T) extends (() => string) ? Deep<string> : number
export declare const x: Select<string>`,
		},
		{
			name: "anonymous method compound",
			dep: apiStabilityRecursiveAlias + `type Select<T> = { m(): T } extends { m(): string } ? Deep<string> : number
export declare const x: Select<string>`,
		},
		{
			name: "mapped compound",
			dep: apiStabilityRecursiveAlias + `type Select<T> = { [K in keyof T]: Deep<K> } extends { toString(): string } ? number : Deep<string>
export declare const x: Select<string>`,
		},
		{
			name: "inferred class property relation",
			dep: apiStabilityRecursiveAlias + `declare function make<T>(): Deep<T>
class F { v = make<string>() }
type Select<T> = T extends { v: number } ? string : number
export declare const x: Select<F>`,
		},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()

			c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
				"/.src/dep.d.ts": test.dep,
				"/.src/test.ts":  `export { x } from "./dep.js"`,
			})
			defer done()
			_ = c.GetDiagnostics(context.Background(), files["/.src/test.ts"])
			x := apiStabilityTestExport(t, c, files["/.src/dep.d.ts"], "x")
			analysis := newApiStabilityAnalysis(tp)
			before := c.TotalInstantiationCount
			verdict := analysis.newSafetyScan().typeNode(ast.TSGetSymbolDeclarations(x)[0].AsVariableDeclaration().Type, apiStabilitySubstitution{}, nil, false)
			delta := c.TotalInstantiationCount - before
			if verdict == apiStabilitySafetySafe {
				t.Errorf("compound operand was authorized as safe")
			}
			if delta != 0 {
				t.Errorf("safety scan instantiated %d types, want 0", delta)
			}
			if globals := c.GetGlobalDiagnostics(); len(globals) != 0 {
				t.Errorf("safety scan produced global diagnostics: %v", globals)
			}
			if len(analysis.sessionSymbols) != 0 {
				t.Errorf("refused scan populated a symbol surface")
			}
		})
	}
}

func TestApiStabilitySafetyDeferredCompoundStaysSafe(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `type Select<T> = { v: T } extends { v: string } ? number : T
export interface X<T> { v: Select<T> }`,
	})
	defer done()
	_ = c.GetDiagnostics(context.Background(), files["/.src/test.ts"])
	x := apiStabilityTestExport(t, c, files["/.src/test.ts"], "X")
	before := c.TotalInstantiationCount
	used := tp.ApiStabilityUsedBySymbol(x)
	delta := c.TotalInstantiationCount - before
	if len(used.dependencies) != 0 {
		t.Fatalf("deferred anonymous operand should expose nothing, got %v", apiStabilityDependencyNames(used))
	}
	if delta != 0 {
		t.Fatalf("deferred anonymous operand instantiated %d types", delta)
	}
	if globals := c.GetGlobalDiagnostics(); len(globals) != 0 {
		t.Fatalf("deferred anonymous operand produced global diagnostics: %v", globals)
	}
}

func TestApiStabilitySafetyStructuralConditionalStaysSelectable(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
interface Holder<T> { value: T extends { value: string } ? number : E }
export declare const x: Holder<{ value: string }>`,
	})
	defer done()
	_ = c.GetDiagnostics(context.Background(), files["/.src/test.ts"])
	x := apiStabilityTestExport(t, c, files["/.src/test.ts"], "x")
	session := tp.NewApiStabilitySession()
	used := session.UsedBySymbol(x)
	if len(used.dependencies) != 0 {
		t.Fatalf("structural conditional selected the true branch; E must stay quiet: %v", apiStabilityDependencyNames(used))
	}
	entry := session.analysis.sessionSymbols[x]
	if !entry.complete || entry.blocked {
		t.Fatalf("structural conditional surface should settle complete: %+v", entry)
	}
}

func TestApiStabilitySafetyTypeLiteralComputedMembers(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
type PropertyAlias = string
/** @stability experimental */
type ParameterAlias = string
/** @stability experimental */
type MethodAlias = string
/** @stability experimental */
type ComputedAlias = string
export declare const x: {
  [Symbol.iterator]?: PropertyAlias
  [Symbol.toStringTag](value: ParameterAlias): MethodAlias
  ["computed"]: ComputedAlias
}
`,
	})
	defer done()
	_ = c.GetDiagnostics(context.Background(), files["/.src/test.ts"])
	x := apiStabilityTestExport(t, c, files["/.src/test.ts"], "x")
	session := tp.NewApiStabilitySession()
	used := session.UsedBySymbol(x)
	for _, want := range []string{"PropertyAlias", "ParameterAlias", "MethodAlias", "ComputedAlias"} {
		if !containsDependency(used, want) {
			t.Fatalf("computed member annotation %s lost its erased alias provenance, got %v", want, apiStabilityDependencyNames(used))
		}
	}
	entry := session.analysis.sessionSymbols[x]
	if !entry.complete || entry.blocked {
		t.Fatalf("computed member surface should settle complete: %+v", entry)
	}
	if globals := c.GetGlobalDiagnostics(); len(globals) != 0 {
		t.Fatalf("computed member traversal produced global diagnostics: %v", globals)
	}
}

func TestApiStabilitySafetyComputedNameDisplayAndSort(t *testing.T) {
	t.Parallel()

	c, _, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `export declare const x: {
  [Symbol.iterator]?: string
  [Symbol.toStringTag](): void
  ["computed"]: number
  [42]: boolean
}
`,
	})
	defer done()
	_ = c.GetDiagnostics(context.Background(), files["/.src/test.ts"])
	x := apiStabilityTestExport(t, c, files["/.src/test.ts"], "x")
	typ := c.GetTypeOfSymbol(x)
	_ = c.GetPropertiesOfType(typ)
	members, ok := checker.GetResolvedMembersOfTypeIfMaterialized(c, typ)
	if !ok {
		t.Fatal("member table should be materialized")
	}
	wantByKey := map[string]string{
		"\xfe@iterator@":    "[Symbol.iterator]",
		"\xfe@toStringTag@": "[Symbol.toStringTag]",
		"computed":          "computed",
		"42":                "42",
	}
	seen := 0
	for key, member := range ast.TSGetSymbolTableEntries(members) {
		wantRendered := ""
		for prefix, rendered := range wantByKey {
			if key == prefix || (len(key) > len(prefix) && key[:len(prefix)] == prefix) {
				wantRendered = rendered
				break
			}
		}
		if wantRendered == "" {
			continue
		}
		seen++
		dependency := ApiStabilityDependency{symbol: member, declaration: ast.TSGetSymbolDeclarations(member)[0]}
		if rendered := ApiStabilityDependencyName(dependency); rendered != wantRendered {
			t.Errorf("member %q displayed as %q, want %q", key, rendered, wantRendered)
		}
		if sorted := apiStabilityDependencySortName(dependency); sorted != wantRendered {
			t.Errorf("member %q sorted as %q, want %q", key, sorted, wantRendered)
		}
	}
	if seen != len(wantByKey) {
		t.Fatalf("expected %d computed members, saw %d", len(wantByKey), seen)
	}

	alias := files["/.src/test.ts"].Statements.Nodes[0].AsVariableStatement().DeclarationList.AsVariableDeclarationList().Declarations.Nodes[0].AsVariableDeclaration()
	wantInOrder := []string{"[Symbol.iterator]", "[Symbol.toStringTag]", "computed", "42"}
	memberNodes := alias.Type.Members()
	if len(memberNodes) != len(wantInOrder) {
		t.Fatalf("expected %d computed members, got %d", len(wantInOrder), len(memberNodes))
	}
	for index, member := range memberNodes {
		dependency := ApiStabilityDependency{declaration: member}
		if rendered := ApiStabilityDependencyName(dependency); rendered != wantInOrder[index] {
			t.Errorf("declaration %d displayed as %q, want %q", index, rendered, wantInOrder[index])
		}
		if sorted := apiStabilityDependencySortName(dependency); sorted != wantInOrder[index] {
			t.Errorf("declaration %d sorted as %q, want %q", index, sorted, wantInOrder[index])
		}
	}
}

func TestApiStabilitySafetyMaterializedReferenceSkipsGuard(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `export interface Box<A> { value: A }; export const box: Box<string> = { value: "x" }`,
	})
	defer done()
	primeApiStabilityChecker(t, c)
	symbol := apiStabilityTestExport(t, c, files["/.src/test.ts"], "box")
	typ := c.GetTypeOfSymbol(symbol)
	_ = c.GetPropertiesOfType(typ)
	if _, ok := checker.GetResolvedMembersOfTypeIfMaterialized(c, typ); !ok {
		t.Fatal("member table should be materialized")
	}
	analysis := newApiStabilityAnalysis(tp)
	surface := newApiStabilitySurface()
	analysis.collectConcreteReferenceSurface(&surface, typ)
	if analysis.safetyWork != 0 {
		t.Fatalf("materialized reference consumed %d guard work units, want 0", analysis.safetyWork)
	}
	if !surface.complete || surface.blocked {
		t.Fatalf("materialized reference surface should be complete, got complete=%v blocked=%v", surface.complete, surface.blocked)
	}
}

func TestApiStabilitySafetySafeVerdictIsReused(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `export interface Box<A> { value: A; map(fn: (value: A) => A): Box<A> }`,
	})
	defer done()
	symbol := apiStabilityTestExport(t, c, files["/.src/test.ts"], "Box")
	declaration := c.GetDeclaredTypeOfSymbol(symbol)
	if declaration == nil {
		t.Fatal("declared type missing")
	}
	analysis := newApiStabilityAnalysis(tp)
	if !analysis.memberTableResolutionIsSafe(declaration, apiStabilitySubstitution{}) {
		t.Fatal("bounded interface member table should be safe")
	}
	spent := analysis.safetyWork
	if spent == 0 {
		t.Fatal("guard unexpectedly consumed no budget on the first request")
	}
	if !analysis.memberTableResolutionIsSafe(declaration, apiStabilitySubstitution{}) {
		t.Fatal("repeated guard request must stay safe")
	}
	if analysis.safetyWork != spent {
		t.Fatalf("completed Safe verdict was not reused: %d -> %d", spent, analysis.safetyWork)
	}
	if len(analysis.safetySafeMemo) == 0 {
		t.Fatal("completed Safe verdict was not recorded")
	}
}

func TestApiStabilitySafetyPostBudgetRefusalIsCheap(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `export interface Box<A> { value: A }`,
	})
	defer done()
	symbol := apiStabilityTestExport(t, c, files["/.src/test.ts"], "Box")
	declaration := c.GetDeclaredTypeOfSymbol(symbol)
	analysis := newApiStabilityAnalysis(tp)
	analysis.safetyWork = apiStabilitySafetyMaxWork + 1

	if analysis.memberTableResolutionIsSafe(declaration, apiStabilitySubstitution{}) {
		t.Fatal("exhausted analysis must refuse member table resolution")
	}
	if analysis.annotationResolutionIsSafe(ast.TSGetSymbolDeclarations(declaration.Symbol())[0], apiStabilitySubstitution{}) {
		t.Fatal("exhausted analysis must refuse annotation resolution")
	}
	if analysis.safetyWork != apiStabilitySafetyMaxWork+1 {
		t.Fatalf("post-budget refusals consumed budget: %d", analysis.safetyWork)
	}
	if len(analysis.safetySafeMemo) != 0 {
		t.Fatal("post-budget refusals must not publish a Safe verdict")
	}
}

func TestApiStabilitySafetySettleSkipsBlockedRecollectionAfterExhaustion(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `export interface Box<A> { value: A }`,
	})
	defer done()
	analysis := newApiStabilityAnalysis(tp)
	symbol := apiStabilityTestExport(t, c, files["/.src/test.ts"], "Box")
	declaration := c.GetDeclaredTypeOfSymbol(symbol)
	key := apiStabilityTypeKey{t: declaration, inspect: apiStabilityExpand}
	surface := newApiStabilitySurface()
	surface.block()
	analysis.storeTypeSurface(key, surface)
	analysis.safetyWork = apiStabilitySafetyMaxWork + 1

	analysis.settle()
	if analysis.safetyWork != apiStabilitySafetyMaxWork+1 {
		t.Fatalf("settle recollection consumed budget after exhaustion: %d", analysis.safetyWork)
	}
	if entry := analysis.sessionTypes[key]; entry.complete {
		t.Fatalf("blocked surface must never settle complete, got %+v", entry)
	}
}

func TestApiStabilitySafetyTypeNameLookupCacheIsIdentitySafe(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
export declare function consume(x: E): E`,
	})
	defer done()
	_ = c.GetDiagnostics(context.Background(), files["/.src/test.ts"])
	analysis := newApiStabilityAnalysis(tp)
	declaration := files["/.src/test.ts"].Statements.Nodes[1].AsFunctionDeclaration()
	typeName := declaration.Parameters.Nodes[0].AsParameterDeclaration().Type.AsTypeReferenceNode().TypeName
	first := analysis.symbolAtTypeNameNode(typeName)
	if first == nil {
		t.Fatal("type name lookup failed")
	}
	if second := analysis.symbolAtTypeNameNode(typeName); second != first {
		t.Fatalf("cached lookup changed identity: %v -> %v", first, second)
	}
	if cached := analysis.typeNameSymbols[typeName]; cached != first {
		t.Fatal("successful lookup was not cached by AST identity")
	}
	if analysis.symbolAtTypeNameNode(nil) != nil {
		t.Fatal("nil node must not resolve")
	}
	if len(analysis.typeNameSymbols) != 1 {
		t.Fatalf("failed or nil lookups must not be cached: %v", analysis.typeNameSymbols)
	}
}
