package typeparser

import (
	"context"
	"fmt"
	"slices"
	"strings"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
)

func apiStabilityTestExport(t *testing.T, c *checker.Checker, sf *ast.SourceFile, name string) *ast.Symbol {
	t.Helper()
	module := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
	if module == nil {
		t.Fatalf("no module symbol for %s", sf.FileName())
	}
	for _, symbol := range c.GetExportsOfModule(module) {
		if symbol != nil && ast.TSGetSymbolName(symbol) == name {
			return symbol
		}
	}
	t.Fatalf("export %q not found in %s", name, sf.FileName())
	return nil
}

func apiStabilityDependencyNames(used ApiStabilityUsed) []string {
	names := make([]string, 0, len(used.dependencies))
	for _, dependency := range used.dependencies {
		names = append(names, ApiStabilityDependencyName(dependency))
	}
	return names
}

func containsDependency(used ApiStabilityUsed, name string) bool {
	for _, dependency := range used.dependencies {
		if ApiStabilityDependencyName(dependency) == name {
			return true
		}
	}
	return false
}

func primeApiStabilityChecker(t testing.TB, c *checker.Checker) {
	t.Helper()
	program, ok := c.Program().(*compiler.Program)
	if !ok {
		t.Fatalf("program is %T, want *compiler.Program", c.Program())
	}
	_ = program.GetSemanticDiagnostics(context.Background(), nil)
}

func TestApiStabilityDeclaredIndexInfosDoNotResolveComputedNames(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `type Step<T extends unknown[]> = T extends [unknown, ...infer Tail] ? Step<Tail> : "key"
declare const key: Step<[` + strings.Repeat("0,", 60) + `]>
export interface X {
  [key]: number
  [name: string]: number
}
`,
	})
	defer done()
	_ = tp

	x := apiStabilityTestExport(t, c, files["/.src/test.ts"], "X")
	before := c.TotalInstantiationCount
	infos := checker.GetDeclaredIndexInfosOfSymbol(c, x)
	if delta := c.TotalInstantiationCount - before; delta != 0 {
		t.Fatalf("declared index accessor instantiated %d types, want 0", delta)
	}
	if globals := c.GetGlobalDiagnostics(); len(globals) != 0 {
		t.Fatalf("declared index accessor produced global diagnostics: %v", globals)
	}
	if len(infos) != 1 {
		t.Fatalf("got %d declared index infos, want 1", len(infos))
	}
	if info := infos[0]; info == nil || info.ValueType() != nil {
		t.Fatalf("unresolved index value type should stay nil: %v", info)
	}
}

func TestApiStabilityDeclaredCacheSharedAcrossTypeParsers(t *testing.T) {
	t.Parallel()

	c, tp1, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/dep.ts": `/** @stability experimental */
export interface E { value: string }
export interface Leaky { value: E }
`,
		"/.src/consumer.ts": `export interface Consumer { direct: import("./dep").E }
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	dep := files["/.src/dep.ts"]
	consumer := files["/.src/consumer.ts"]
	leaky := apiStabilityTestExport(t, c, dep, "Leaky")

	if tp1.links.ApiStabilityDeclaredSymbol.Has(leaky) {
		t.Fatal("declared cache should start empty")
	}
	deps := tp1.ApiStabilityUsedBySymbol(leaky)
	if !containsDependency(deps, "E") {
		t.Fatalf("Leaky should expose E, got %v", apiStabilityDependencyNames(deps))
	}
	if !tp1.links.ApiStabilityDeclaredSymbol.Has(leaky) {
		t.Fatal("a declared read during the analysis should be cached")
	}

	tp2 := NewTypeParser(c.Program(), c)
	if tp2.links != tp1.links {
		t.Fatal("TypeParser instances over one checker should share EffectLinks")
	}
	if !tp2.links.ApiStabilityDeclaredSymbol.Has(leaky) {
		t.Fatal("second parser did not see the cached declared symbol")
	}
	if got := tp2.ApiStabilityUsedBySymbol(leaky); !containsDependency(got, "E") {
		t.Fatalf("second parser returned a different surface: %v", apiStabilityDependencyNames(got))
	}

	consumerSym := apiStabilityTestExport(t, c, consumer, "Consumer")
	consumerDeps := tp2.ApiStabilityUsedBySymbol(consumerSym)
	if !containsDependency(consumerDeps, "E") {
		t.Fatalf("Consumer should expose E directly, got %v", apiStabilityDependencyNames(consumerDeps))
	}
	if !tp1.links.ApiStabilityDeclaredSymbol.Has(consumerSym) {
		t.Fatal("first parser did not see the cross-file cached declared symbol")
	}
}

func TestApiStabilityNestedNamedSurfaceStaysShallow(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
export interface E { value: string }
export interface Hidden { value: E }
interface Callable { (x: E): void }
export declare const exposed: { hidden: Hidden }
export interface CallableHolder { callable: Callable }
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	testFile := files["/.src/test.ts"]

	hidden := apiStabilityTestExport(t, c, testFile, "Hidden")
	if !containsDependency(tp.ApiStabilityUsedBySymbol(hidden), "E") {
		t.Fatal("Hidden's own surface should expose E")
	}

	exposed := apiStabilityTestExport(t, c, testFile, "exposed")
	exposedDeps := tp.ApiStabilityUsedBySymbol(exposed)
	if containsDependency(exposedDeps, "E") {
		t.Fatalf("nested named Hidden must stay shallow, got %v", apiStabilityDependencyNames(exposedDeps))
	}

	holder := apiStabilityTestExport(t, c, testFile, "CallableHolder")
	holderDeps := tp.ApiStabilityUsedBySymbol(holder)
	if !containsDependency(holderDeps, "E") {
		t.Fatalf("directly exposed public call signatures should expose E, got %v", apiStabilityDependencyNames(holderDeps))
	}
}

func TestApiStabilityCacheDeclaredCeilingIsPerAlias(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/dep.ts": `/** @stability experimental */
export interface E { value: string }
export interface Leaky { value: E }
`,
		"/.src/plain.ts": `export { Leaky } from "./dep"
`,
		"/.src/tagged.ts": `/** @stability experimental */
export { Leaky } from "./dep"
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	plain := apiStabilityTestExport(t, c, files["/.src/plain.ts"], "Leaky")
	tagged := apiStabilityTestExport(t, c, files["/.src/tagged.ts"], "Leaky")

	if level := tp.DeclaredApiStability(plain); level != ApiStabilityStable {
		t.Fatalf("plain reexport symbol level = %v, want stable", level)
	}
	if level := tp.DeclaredApiStability(tagged); level != ApiStabilityStable {
		t.Fatalf("selected-file forwarding tag leaked into the symbol cache: %v", level)
	}

	target := resolveAliasedSymbolForTest(t, c, plain)
	if !containsDependency(tp.ApiStabilityUsedBySymbol(target), "E") {
		t.Fatalf("target surface should expose E")
	}
	if !tp.links.ApiStabilityDeclaredSymbol.Has(target) {
		t.Fatal("the declared target lookup should be cached")
	}
}

func resolveAliasedSymbolForTest(t *testing.T, c *checker.Checker, symbol *ast.Symbol) *ast.Symbol {
	t.Helper()
	seen := map[*ast.Symbol]bool{}
	for symbol != nil && ast.TSGetSymbolFlags(symbol)&ast.SymbolFlagsAlias != 0 {
		if seen[symbol] {
			break
		}
		seen[symbol] = true
		next := c.GetImmediateAliasedSymbol(symbol)
		if next == symbol {
			break
		}
		symbol = next
	}
	if symbol == nil {
		t.Fatal("alias resolution failed")
	}
	return symbol
}

func TestApiStabilitySignatureDeclaredCacheKeepsOverloadsDistinct(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `export function f(x: string): string
/** @stability unstable */
export function f(x: number): string
export function f(x: string | number): string { return "" }
`,
	})
	defer done()

	f := apiStabilityTestExport(t, c, files["/.src/test.ts"], "f")
	signatures := apiStabilityTestSignatures(t, c, f)
	if len(signatures) != 2 {
		t.Fatalf("expected two public overloads, got %d", len(signatures))
	}

	first := tp.DeclaredApiStabilityOfSignature(signatures[0])
	second := tp.DeclaredApiStabilityOfSignature(signatures[1])
	if first.level == second.level {
		t.Fatalf("distinct overload tags collapsed: %v and %v", first.level, second.level)
	}
	if first.level != ApiStabilityStable || second.level != ApiStabilityUnstable {
		t.Fatalf("unexpected overload levels: first=%v second=%v", first.level, second.level)
	}
	if !tp.links.ApiStabilityDeclaredSignature.Has(rawSignature(signatures[0])) ||
		!tp.links.ApiStabilityDeclaredSignature.Has(rawSignature(signatures[1])) {
		t.Fatal("both overload declarations should be cached separately")
	}
}

func apiStabilityTestSignatures(t *testing.T, c *checker.Checker, symbol *ast.Symbol) []*checker.Signature {
	t.Helper()
	if symbol == nil {
		t.Fatal("nil symbol")
	}
	signatures := c.GetSignaturesOfType(c.GetTypeOfSymbol(symbol), checker.SignatureKindCall)
	if len(signatures) == 0 {
		t.Fatalf("symbol %q has no call signatures", ast.TSGetSymbolName(symbol))
	}
	return signatures
}

func TestApiStabilitySignatureSurfaceRetainsSubstitution(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
declare function make<T>(): (x: T) => T
export const f = make<E>()
`,
	})
	defer done()

	f := apiStabilityTestExport(t, c, files["/.src/test.ts"], "f")
	signatures := apiStabilityTestSignatures(t, c, f)
	instantiated := signatures[0]

	session := tp.NewApiStabilitySession()
	computed := session.UsedBySignature(instantiated)
	if !containsDependency(computed, "E") {
		t.Fatalf("instantiated signature should expose E, got %v", apiStabilityDependencyNames(computed))
	}
	key := apiStabilitySignatureKey{signature: instantiated}
	if surface, ok := session.analysis.sessionSignatures[key]; !ok || !surface.complete {
		t.Fatal("completed signature surface should be memoized in the session")
	}
	work := session.analysis.work
	if repeat := session.UsedBySignature(instantiated); !containsDependency(repeat, "E") {
		t.Fatalf("repeated session query lost E, got %v", apiStabilityDependencyNames(repeat))
	}
	if session.analysis.work != work {
		t.Fatal("repeated session query recomputed the memoized signature surface")
	}

	raw := rawSignature(instantiated)
	if raw == instantiated {
		t.Fatal("expected an instantiated signature with a raw target")
	}
	shallow := tp.ApiStabilityUsedBySignature(raw)
	if containsDependency(shallow, "E") {
		t.Fatalf("raw target should stay shallow, got %v", apiStabilityDependencyNames(shallow))
	}
}

func TestApiStabilityComputedIncludesDeclaredMemberStability(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `export interface O {
  /** @stability experimental */
  value: string
}
export interface S {
  /** @stability unstable */
  (x: string): string
}
`,
	})
	defer done()

	o := apiStabilityTestExport(t, c, files["/.src/test.ts"], "O")
	oUsed := tp.ApiStabilityUsedBySymbol(o)
	if oUsed.minimum != ApiStabilityExperimental {
		t.Fatalf("O minimum = %v, want experimental: %v", oUsed.minimum, apiStabilityDependencyNames(oUsed))
	}
	if !containsDependency(oUsed, "value") {
		t.Fatalf("O should expose its tagged property, got %v", apiStabilityDependencyNames(oUsed))
	}

	s := apiStabilityTestExport(t, c, files["/.src/test.ts"], "S")
	sUsed := tp.ApiStabilityUsedBySymbol(s)
	if sUsed.minimum != ApiStabilityUnstable {
		t.Fatalf("S minimum = %v, want unstable: %v", sUsed.minimum, apiStabilityDependencyNames(sUsed))
	}
	foundSignature := false
	for _, dependency := range sUsed.dependencies {
		if dependency.signature != nil && dependency.level == ApiStabilityUnstable && dependency.declaration != nil {
			foundSignature = true
		}
	}
	if !foundSignature {
		t.Fatalf("S should expose its tagged call signature with provenance, got %v", apiStabilityDependencyNames(sUsed))
	}
}

func TestApiStabilityReplacedDefaultStaysClean(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
declare function identity<T = E>(x: T): T
export const f = identity<string>
`,
	})
	defer done()

	f := apiStabilityTestExport(t, c, files["/.src/test.ts"], "f")
	if used := tp.ApiStabilityUsedBySymbol(f); containsDependency(used, "E") {
		t.Fatalf("replaced default must stay clean, got %v", apiStabilityDependencyNames(used))
	}
}

func TestApiStabilityCacheCyclesAndEmptyResultsAreSafe(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
/** @stability unstable */
export interface Node { next: Node; payload: E }
export interface Stable { value: string }
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	testFile := files["/.src/test.ts"]
	node := apiStabilityTestExport(t, c, testFile, "Node")
	session := tp.NewApiStabilitySession()
	nodeUsed := session.UsedBySymbol(node)
	if nodeUsed.incomplete || nodeUsed.minimum != ApiStabilityExperimental || !containsDependency(nodeUsed, "E") {
		t.Fatalf("self-referential Node should expose E, got min=%v deps=%v", nodeUsed.minimum, apiStabilityDependencyNames(nodeUsed))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, nodeUsed, "Node")
	if surface := session.analysis.sessionSymbols[node]; !surface.complete || surface.blocked {
		t.Fatal("a settled cyclic surface should be a complete session result")
	}
	if len(session.analysis.sessionSymbols[node].findings) == 0 {
		t.Fatal("the stored cyclic surface must keep the own tag")
	}
	work := session.analysis.work
	if !containsDependency(session.UsedBySymbol(node), "E") {
		t.Fatal("repeated session query lost Node's E")
	}
	if session.analysis.work != work {
		t.Fatal("repeated session query recomputed the settled cyclic surface")
	}

	stable := apiStabilityTestExport(t, c, testFile, "Stable")
	stableSession := tp.NewApiStabilitySession()
	stableUsed := stableSession.UsedBySymbol(stable)
	if len(stableUsed.dependencies) != 0 {
		t.Fatalf("Stable should have no dependencies, got %v", apiStabilityDependencyNames(stableUsed))
	}
	if surface := stableSession.analysis.sessionSymbols[stable]; !surface.complete || surface.blocked {
		t.Fatal("stable/empty results should be complete session entries")
	}
}

func TestApiStabilityColdLazyFiniteApplicationMatchesWarm(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
type A<T> = T extends string ? A<number> : E
export declare const x: A<string>
`,
	})
	defer done()

	x := apiStabilityTestExport(t, c, files["/.src/test.ts"], "x")
	cold := tp.ApiStabilityUsedBySymbol(x)
	if !containsDependency(cold, "E") {
		t.Fatalf("the cold surface should lazily resolve E, got %v", apiStabilityDependencyNames(cold))
	}

	primeApiStabilityChecker(t, c)
	warm := NewTypeParser(c.Program(), c).ApiStabilityUsedBySymbol(x)
	if !containsDependency(warm, "E") {
		t.Fatalf("the materialized surface should expose E, got %v", apiStabilityDependencyNames(warm))
	}

	recomputed := NewTypeParser(c.Program(), c).ApiStabilityUsedBySymbol(x)
	if !containsDependency(recomputed, "E") {
		t.Fatalf("the materialized recomputation should expose E, got %v", apiStabilityDependencyNames(recomputed))
	}
	if recomputed.minimum != warm.minimum || len(recomputed.dependencies) != len(warm.dependencies) {
		t.Fatalf("warm results disagree: first=%v recomputed=%v", apiStabilityDependencyNames(warm), apiStabilityDependencyNames(recomputed))
	}
}

func TestApiStabilityExplicitStableDeclaredPresence(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability stable */
export function tagged(x: string): string
export function tagged(x: number): number
export function untagged(x: string): string
export function untagged(x: number): number
`,
	})
	defer done()

	tagged := apiStabilityTestExport(t, c, files["/.src/test.ts"], "tagged")
	untagged := apiStabilityTestExport(t, c, files["/.src/test.ts"], "untagged")

	taggedDeclared := tp.DeclaredApiStabilityOfSymbol(tagged)
	if taggedDeclared.level != ApiStabilityStable || taggedDeclared.declaration == nil {
		t.Fatalf("explicit stable root lost its provenance: %+v", taggedDeclared)
	}
	if !tp.links.ApiStabilityDeclaredSymbol.Has(tagged) {
		t.Fatal("explicit stable root should be cached")
	}
	if declared := tp.DeclaredApiStabilityOfSymbol(untagged); declared.level != ApiStabilityStable || declared.declaration != nil {
		t.Fatalf("untagged root should be stable without provenance: %+v", declared)
	}

	taggedSignatures := apiStabilityTestSignatures(t, c, tagged)
	untaggedSignatures := apiStabilityTestSignatures(t, c, untagged)
	if len(taggedSignatures) != 2 || len(untaggedSignatures) != 2 {
		t.Fatalf("unexpected overload counts: tagged=%d untagged=%d", len(taggedSignatures), len(untaggedSignatures))
	}
	for index, signature := range taggedSignatures {
		declared := tp.DeclaredApiStabilityOfSignature(signature)
		if !tp.links.ApiStabilityDeclaredSignature.Has(rawSignature(signature)) {
			t.Fatalf("tagged overload %d should be cached", index)
		}
		if index == 0 {
			if declared.level != ApiStabilityStable || declared.declaration == nil {
				t.Fatalf("explicit stable overload lost its provenance: %+v", declared)
			}
			if !tp.links.ApiStabilityDeclaredDeclaration.Has(declared.declaration) {
				t.Fatal("explicit stable declaration should be cached")
			}
		} else if declared.level != ApiStabilityStable || declared.declaration != nil {
			t.Fatalf("untagged overload should be stable without provenance: %+v", declared)
		}
	}
	for index, signature := range untaggedSignatures {
		if declared := tp.DeclaredApiStabilityOfSignature(signature); declared.declaration != nil {
			t.Fatalf("untagged signature %d has a declaration: %+v", index, declared)
		}
	}
}

func TestApiStabilityComputedPublicMembers(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
interface E { x: string }
/** @stability experimental */
type P = string
const custom = Symbol()
const privateKey = Symbol()
export interface OnlyComputed { [Symbol.iterator](): E; [custom]: P }
export interface MixedComputed {
  plain: number
  [Symbol.iterator](): E
  [custom]: P
  ["literal"]: P
}
export interface TaggedComputed {
  /** @stability experimental */
  [Symbol.iterator](): string
  /** @stability experimental */
  [custom]: string
}
export class ComputedClass {
  plain: number
  [Symbol.iterator](): E
  [custom]: P
  private [privateKey]: E
  protected [Symbol.toStringTag](): E
  static [custom]: P
  static [Symbol.iterator](): E
}
export class HiddenComputed {
  private [privateKey]: E
  protected [Symbol.toStringTag](): E
}
export declare const computedObject: {
  [Symbol.iterator](): E
  [custom]: P
  ["literal"]: P
}
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	for _, name := range []string{"OnlyComputed", "MixedComputed", "ComputedClass", "computedObject"} {
		symbol := apiStabilityTestExport(t, c, files["/.src/test.ts"], name)
		session := tp.NewApiStabilitySession()
		used := session.UsedBySymbol(symbol)
		names := apiStabilityDependencyNames(used)
		if !containsDependency(used, "E") {
			t.Errorf("%s lost its computed public dependency E: %v", name, names)
		}
		if !containsDependency(used, "P") {
			t.Errorf("%s lost its computed public dependency P: %v", name, names)
		}
		if surface := session.analysis.sessionSymbols[symbol]; !surface.complete || surface.blocked {
			t.Errorf("%s computed member surface should settle complete: %+v", name, surface)
		}
	}

	hidden := apiStabilityTestExport(t, c, files["/.src/test.ts"], "HiddenComputed")
	if used := tp.ApiStabilityUsedBySymbol(hidden); containsDependency(used, "E") {
		t.Errorf("private/protected computed members leaked into the surface: %v", apiStabilityDependencyNames(used))
	}

	tagged := apiStabilityTestExport(t, c, files["/.src/test.ts"], "TaggedComputed")
	used := tp.ApiStabilityUsedBySymbol(tagged)
	rendered := map[string]bool{}
	for _, dependency := range used.dependencies {
		rendered[ApiStabilityDependencyName(dependency)] = true
		if strings.HasPrefix(ApiStabilityDependencyName(dependency), "\xfe") {
			t.Errorf("late-bound internal name leaked into a diagnostic: %q", ApiStabilityDependencyName(dependency))
		}
	}
	if !rendered["[Symbol.iterator]"] || !rendered["[custom]"] {
		t.Errorf("tagged computed members should render readable names, got %v", apiStabilityDependencyNames(used))
	}
	if len(used.dependencies) < 2 {
		t.Errorf("tagged computed members should report separately, got %v", apiStabilityDependencyNames(used))
	}
}

func TestApiStabilityEarlyTableComputedMembersNotCachedComplete(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
interface E { x: string }
export interface WellKnownOnly { plain: number; [Symbol.iterator](): E }
export class ColdClass { plain: number; static [Symbol.iterator](): E }
`,
	})
	defer done()
	for _, name := range []string{"WellKnownOnly", "ColdClass"} {
		symbol := apiStabilityTestExport(t, c, files["/.src/test.ts"], name)
		declared := c.GetDeclaredTypeOfSymbol(symbol)
		if _, resolved := checker.GetResolvedMembersOfTypeIfMaterialized(c, declared); resolved {
			t.Fatalf("%s: expected an unmaterialized member table for the early-table path", name)
		}
		session := tp.NewApiStabilitySession()
		used := session.UsedBySymbol(symbol)
		if containsDependency(used, "E") {
			t.Fatalf("%s: the early table cannot see a late-bound member; the surface must stay incomplete", name)
		}
		if surface := session.analysis.sessionSymbols[symbol]; surface.complete {
			t.Fatalf("%s: an incomplete surface must never settle complete: %+v", name, surface)
		}
	}
	primeApiStabilityChecker(t, c)
	for _, name := range []string{"WellKnownOnly", "ColdClass"} {
		symbol := apiStabilityTestExport(t, c, files["/.src/test.ts"], name)
		declared := c.GetDeclaredTypeOfSymbol(symbol)
		if _, resolved := checker.GetResolvedMembersOfTypeIfMaterialized(c, declared); !resolved {
			t.Fatalf("%s: expected the member table to be materialized after checking", name)
		}
		session := tp.NewApiStabilitySession()
		used := session.UsedBySymbol(symbol)
		if !containsDependency(used, "E") {
			t.Fatalf("%s: retry after materialization lost the computed dependency: %v", name, apiStabilityDependencyNames(used))
		}
		if surface := session.analysis.sessionSymbols[symbol]; !surface.complete {
			t.Fatalf("%s: retry should settle a complete surface: %+v", name, surface)
		}
	}
}

func TestApiStabilitySettleSkipRequiresNoAdvancement(t *testing.T) {
	t.Parallel()

	_, tp, _, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `export interface Box<A> { value: A }`,
	})
	defer done()
	analysis := newApiStabilityAnalysis(tp)
	surface := newApiStabilitySurface()
	surface.block()

	if analysis.settleCannotAdvance(surface) {
		t.Fatal("a surface must not be skipped while the guard budget is available")
	}
	analysis.safetyWork = apiStabilitySafetyMaxWork + 1
	surface.observedEpoch = 7
	analysis.materializationEpoch = 7
	if !analysis.settleCannotAdvance(surface) {
		t.Fatal("a blocked surface with no advancement since collection must be skipped")
	}
	analysis.materializationEpoch = 8
	if analysis.settleCannotAdvance(surface) {
		t.Fatal("a materialization advancement must force a recollection")
	}
	analysis.materializationEpoch = 7
	complete := newApiStabilitySurface()
	complete.observedEpoch = 7
	if analysis.settleCannotAdvance(complete) {
		t.Fatal("only blocked surfaces are skipped")
	}
}

func TestApiStabilitySettleRecollectsMaterializedMember(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/dep.ts": `/** @stability experimental */
interface E { value: string }
export interface Root { a: E; z: typeof fn }
declare const object: Root
function fn() { return object.a.value.length }`,
		"/.src/test.ts": `export { Root } from "./dep.js"`,
	})
	defer done()
	sym := apiStabilityTestExport(t, c, files["/.src/dep.ts"], "Root")
	_ = c.GetDeclaredTypeOfSymbol(sym)
	value := ast.TSGetSymbolTableEntry(ast.TSGetSymbolMembers(sym), "a")
	functionType := c.GetTypeOfSymbol(ast.TSGetSymbolTableEntry(ast.TSGetSymbolMembers(sym), "z"))
	signatures := c.GetSignaturesOfType(functionType, checker.SignatureKindCall)
	if len(signatures) != 1 {
		t.Fatal("signature missing")
	}
	if checker.GetResolvedReturnTypeOfSignatureIfMaterialized(c, signatures[0]) != nil {
		t.Fatal("expected cold inferred return")
	}
	if checker.GetResolvedTypeOfSymbolIfMaterialized(c, value) != nil {
		t.Fatal("expected cold a member")
	}
	ran := false
	checker.RegisterAfterCheckSourceFileCallback(func(_ context.Context, _ checker.Program, cc *checker.Checker, sf *ast.SourceFile) {
		if cc != c || sf != files["/.src/test.ts"] {
			return
		}
		ran = true
		analysis := newApiStabilityAnalysis(tp)
		analysis.safetyWork = apiStabilitySafetyMaxWork + 1
		analysis.symbolSurface(sym)
		if checker.GetResolvedTypeOfSymbolIfMaterialized(c, value) == nil {
			t.Error("later ordinary read should have materialized the refused member")
		}
		analysis.settle()
		used := apiStabilityUsedOf(analysis.sessionSymbols[sym], sym)
		if !containsDependency(used, "E") {
			t.Errorf("settle did not retry a newly materialized member: %v", apiStabilityDependencyNames(used))
		}
		if analysis.safetyWork != apiStabilitySafetyMaxWork+1 {
			t.Errorf("post-exhaustion recurrence consumed guard budget: %d", analysis.safetyWork)
		}
		surface := analysis.sessionSymbols[sym]
		if !surface.blocked {
			t.Fatal("a recollection that stays unavailable must remain blocked")
		}
	})
	_ = c.GetDiagnostics(context.Background(), files["/.src/test.ts"])
	if !ran {
		t.Fatal("callback did not run")
	}

	fresh := newApiStabilityAnalysis(tp)
	fresh.symbolSurface(sym)
	fresh.settle()
	used := apiStabilityUsedOf(fresh.sessionSymbols[sym], sym)
	if !containsDependency(used, "E") {
		t.Fatalf("retry analysis lost the dependency: %v", apiStabilityDependencyNames(used))
	}
	if surface := fresh.sessionSymbols[sym]; !surface.complete {
		t.Fatalf("retry analysis should settle a complete surface: %+v", surface)
	}
}

func TestApiStabilityNaturalBudgetMaterializationRetry(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/dep.ts": "type Heavy = [" + strings.Repeat("string,", 17000) + "]\n" + `/** @stability experimental */
interface E { value: string }
export interface Root { "0heavy": Heavy; a: E; z: typeof fn }
declare const object: Root
function fn() { return object.a.value.length }`,
		"/.src/test.ts": `export { Root } from "./dep.js"`,
	})
	defer done()
	sym := apiStabilityTestExport(t, c, files["/.src/dep.ts"], "Root")
	_ = c.GetDeclaredTypeOfSymbol(sym)
	value := ast.TSGetSymbolTableEntry(ast.TSGetSymbolMembers(sym), "a")
	functionType := c.GetTypeOfSymbol(ast.TSGetSymbolTableEntry(ast.TSGetSymbolMembers(sym), "z"))
	signatures := c.GetSignaturesOfType(functionType, checker.SignatureKindCall)
	if len(signatures) != 1 {
		t.Fatal("signature missing")
	}
	if checker.GetResolvedReturnTypeOfSignatureIfMaterialized(c, signatures[0]) != nil {
		t.Fatal("expected cold inferred return")
	}
	if checker.GetResolvedTypeOfSymbolIfMaterialized(c, value) != nil {
		t.Fatal("expected cold a member")
	}
	ran := false
	checker.RegisterAfterCheckSourceFileCallback(func(_ context.Context, _ checker.Program, cc *checker.Checker, sf *ast.SourceFile) {
		if cc != c || sf != files["/.src/test.ts"] {
			return
		}
		ran = true
		analysis := newApiStabilityAnalysis(tp)
		analysis.symbolSurface(sym)
		if !analysis.safetyBudgetExhausted() {
			t.Fatal("the heavy member should exhaust the shared guard budget")
		}
		analysis.settle()
		used := apiStabilityUsedOf(analysis.sessionSymbols[sym], sym)
		if !containsDependency(used, "E") {
			t.Errorf("natural-budget settle missed the materialized member: %v", apiStabilityDependencyNames(used))
		}
		if !analysis.sessionSymbols[sym].blocked {
			t.Fatal("the root remains blocked and must not settle stable")
		}
	})
	_ = c.GetDiagnostics(context.Background(), files["/.src/test.ts"])
	if !ran {
		t.Fatal("callback did not run")
	}
}

func TestApiStabilityRecoveryPropagatesFindingsToEarlierResults(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/dep.ts": `/** @stability experimental */
interface E { value: string }
declare function make(): { a: E; z: typeof fn }
export const root = make()
function fn() { return root.a.value.length }`,
		"/.src/test.ts": `export { root } from "./dep.js"`,
	})
	defer done()

	sym := apiStabilityTestExport(t, c, files["/.src/dep.ts"], "root")
	typ := c.GetTypeOfSymbol(sym)
	members := c.GetPropertiesOfType(typ)
	var value, fnSymbol *ast.Symbol
	for _, m := range members {
		if ast.TSGetSymbolName(m) == "a" {
			value = m
		}
		if ast.TSGetSymbolName(m) == "z" {
			fnSymbol = m
		}
	}
	if value == nil || fnSymbol == nil {
		t.Fatal("expected members a and z")
	}
	fnType := c.GetTypeOfSymbol(fnSymbol)
	signature := c.GetSignaturesOfType(fnType, checker.SignatureKindCall)[0]
	if checker.GetResolvedReturnTypeOfSignatureIfMaterialized(c, signature) != nil ||
		checker.GetResolvedTypeOfSymbolIfMaterialized(c, value) != nil {
		t.Fatal("expected cold member and inferred return")
	}

	ran := false
	checker.RegisterAfterCheckSourceFileCallback(func(_ context.Context, _ checker.Program, cc *checker.Checker, sf *ast.SourceFile) {
		if cc != c || sf != files["/.src/test.ts"] {
			return
		}
		ran = true
		analysis := newApiStabilityAnalysis(tp)
		analysis.safetyWork = apiStabilitySafetyMaxWork + 1
		analysis.sessionSignatures[apiStabilitySignatureKey{signature: signature}] = cycleCutSurface()
		analysis.symbolSurface(sym)
		if checker.GetResolvedTypeOfSymbolIfMaterialized(c, value) != nil {
			t.Fatal("member was materialized before settlement")
		}
		analysis.settle()
		if analysis.safetyWork != apiStabilitySafetyMaxWork+1 {
			t.Errorf("recovery consumed guard work: %d", analysis.safetyWork)
		}
		if !analysis.sessionSymbols[sym].blocked {
			t.Error("the exhausted root must stay blocked, not settle stable")
		}
		used := apiStabilityUsedOf(analysis.sessionSymbols[sym], sym)
		found := false
		for _, component := range analysis.sessionComponents {
			if containsDependency(apiStabilityUsedOf(component, nil), "E") {
				found = true
			}
		}
		if !found {
			t.Fatalf("recovery sweep did not recollect the component: %v", apiStabilityDependencyNames(used))
		}
		if !containsDependency(used, "E") {
			t.Errorf("recovery found E in a component but failed to propagate it to the root: %v", apiStabilityDependencyNames(used))
		}
	})
	_ = c.GetDiagnostics(context.Background(), files["/.src/test.ts"])
	if !ran {
		t.Fatal("callback did not run")
	}
}

func TestApiStabilityDeclaredPersistsAndCompleteSurfacesReuse(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/dep.ts": `/** @stability experimental */
export interface E { value: string }
export interface Leaky { value: E }
export interface AlsoLeaky { other: E }
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	leaky := apiStabilityTestExport(t, c, files["/.src/dep.ts"], "Leaky")
	alsoLeaky := apiStabilityTestExport(t, c, files["/.src/dep.ts"], "AlsoLeaky")

	session := tp.NewApiStabilitySession()
	used := session.UsedBySymbol(leaky)
	if used.incomplete || !containsDependency(used, "E") {
		t.Fatalf("Leaky should be a complete surface exposing E, got %v (incomplete=%v)", apiStabilityDependencyNames(used), used.incomplete)
	}

	work := session.analysis.work
	repeat := session.UsedBySymbol(leaky)
	if repeat.incomplete || !containsDependency(repeat, "E") {
		t.Fatalf("repeated session query lost E, got %v (incomplete=%v)", apiStabilityDependencyNames(repeat), repeat.incomplete)
	}
	if session.analysis.work != work {
		t.Fatal("repeated session query recomputed a completed surface")
	}

	declared := c.GetDeclaredTypeOfSymbol(leaky)
	if apiStabilitySharedTypeEntry(tp, declared, apiStabilityExpand) == nil {
		t.Fatal("the complete Leaky type surface should be published for reuse")
	}
	next := tp.NewApiStabilitySession()
	nextUsed := next.UsedBySymbol(alsoLeaky)
	if nextUsed.incomplete || !containsDependency(nextUsed, "E") {
		t.Fatalf("AlsoLeaky should be a complete surface exposing E, got %v (incomplete=%v)", apiStabilityDependencyNames(nextUsed), nextUsed.incomplete)
	}
	direct := newApiStabilityAnalysis(tp)
	direct.typeSurface(declared, apiStabilityExpand, apiStabilitySubstitution{})
	if direct.work != 0 {
		t.Fatalf("the published type snapshot consumed %d work", direct.work)
	}

	tp2 := NewTypeParser(c.Program(), c)
	if !tp2.links.ApiStabilityDeclaredSymbol.Has(leaky) {
		t.Fatal("declared symbol should be cached per checker")
	}
	tagOnly := apiStabilityTestExport(t, c, files["/.src/dep.ts"], "E")
	if apiStabilitySharedTypeEntry(tp2, c.GetDeclaredTypeOfSymbol(tagOnly), apiStabilityExpand) != nil {
		t.Fatal("the tagged E type must not be published before a computed query")
	}
	before := c.TotalInstantiationCount
	declaredLevel := tp2.DeclaredApiStability(leaky)
	if declaredLevel != ApiStabilityStable {
		t.Fatalf("Leaky declared = %v, want stable", declaredLevel)
	}
	if delta := c.TotalInstantiationCount - before; delta != 0 {
		t.Fatalf("declared read instantiated %d types, want 0", delta)
	}

	fresh := NewTypeParser(c.Program(), c).NewApiStabilitySession()
	again := fresh.UsedBySymbol(leaky)
	if again.incomplete || len(again.dependencies) != len(used.dependencies) {
		t.Fatalf("a later session disagrees with the first: first=%v again=%v", apiStabilityDependencyNames(used), apiStabilityDependencyNames(again))
	}
}

func TestApiStabilityInstantiationsDoNotContaminate(t *testing.T) {
	t.Parallel()

	type export struct {
		declaration string
		name        string
		leaks       bool
	}
	tests := []struct {
		name    string
		prelude string
		exports []export
	}{
		{
			"generic arguments",
			"interface Box<T> { value: T }",
			[]export{{"export type Leaky = Box<E>", "Leaky", true}, {"export type Clean = Box<string>", "Clean", false}},
		},
		{
			"signature substitution",
			"declare function make<T>(): (x: T) => T",
			[]export{{"export const leaky = make<E>()", "leaky", true}, {"export const clean = make<string>()", "clean", false}},
		},
		{
			"nested signature substitution",
			"declare function make<T>(): { nested: { fn: (x: T) => T } }",
			[]export{{"export const leaky = make<E>()", "leaky", true}, {"export const clean = make<string>()", "clean", false}},
		},
		{
			"shared raw member target",
			"type Deep<T> = T extends string ? Deep<T> : T\ninterface DeepHolder<T> { value: Deep<T> }",
			[]export{{"export declare const leaky: DeepHolder<E>", "leaky", true}, {"export declare const clean: DeepHolder<string>", "clean", false}},
		},
		{
			"heritage materializations",
			"interface Base<T> { value: T }",
			[]export{{"export interface Leaky extends Base<E> {}", "Leaky", true}, {"export interface Clean extends Base<string> {}", "Clean", false}},
		},
		{
			"mutual cycle roots",
			"type B = { c: C; payload: E }\ntype C = { b: B }",
			[]export{{"export type A = { b: B }", "A", true}, {"export type Z = { c: C }", "Z", true}},
		},
	}
	for _, test := range tests {
		for _, declarationsReversed := range []bool{false, true} {
			for _, queriesReversed := range []bool{false, true} {
				t.Run(fmt.Sprintf("%s/declarationsReversed=%t/queriesReversed=%t", test.name, declarationsReversed, queriesReversed), func(t *testing.T) {
					t.Parallel()
					declarations := []string{test.exports[0].declaration, test.exports[1].declaration}
					if declarationsReversed {
						slices.Reverse(declarations)
					}
					c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
						"/.src/test.ts": "/** @stability experimental */\ninterface E { value: string }\n" + test.prelude + "\n" + strings.Join(declarations, "\n") + "\n",
					})
					defer done()
					primeApiStabilityChecker(t, c)

					exports := slices.Clone(test.exports)
					if queriesReversed {
						slices.Reverse(exports)
					}
					for _, export := range exports {
						used := tp.ApiStabilityUsedBySymbol(apiStabilityTestExport(t, c, files["/.src/test.ts"], export.name))
						if containsDependency(used, "E") != export.leaks {
							t.Fatalf("%s exposes E = %t, want %t: %v", export.name, !export.leaks, export.leaks, apiStabilityDependencyNames(used))
						}
					}
				})
			}
		}
	}
}

func TestApiStabilityDeclaredReadsDoNotCompute(t *testing.T) {
	t.Parallel()

	for _, prime := range []bool{false, true} {
		t.Run(fmt.Sprintf("prime=%t", prime), func(t *testing.T) {
			t.Parallel()
			c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
				"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
export interface Leaky { value: E }
/** @stability unstable */
export interface Tagged { leak: string }
/** @stability unstable */
export declare function taggedFn(): string
`,
			})
			defer done()
			if prime {
				primeApiStabilityChecker(t, c)
			}

			testFile := files["/.src/test.ts"]
			leaky := apiStabilityTestExport(t, c, testFile, "Leaky")
			tagged := apiStabilityTestExport(t, c, testFile, "Tagged")
			taggedFn := apiStabilityTestExport(t, c, testFile, "taggedFn")
			signature := apiStabilityTestSignatures(t, c, taggedFn)[0]

			before := c.TotalInstantiationCount
			levels := []ApiStabilityLevel{
				tp.DeclaredApiStability(leaky),
				tp.DeclaredApiStabilityOfSymbol(tagged).level,
				tp.DeclaredApiStabilityOfSymbol(taggedFn).level,
				tp.DeclaredApiStabilityOfSignature(signature).level,
			}
			if delta := c.TotalInstantiationCount - before; delta != 0 {
				t.Fatalf("declared reads instantiated %d types, want 0", delta)
			}
			if want := []ApiStabilityLevel{ApiStabilityStable, ApiStabilityUnstable, ApiStabilityUnstable, ApiStabilityUnstable}; !slices.Equal(levels, want) {
				t.Fatalf("declared levels = %v, want %v", levels, want)
			}
			for _, symbol := range []*ast.Symbol{leaky, tagged, taggedFn} {
				if !tp.links.ApiStabilityDeclaredSymbol.Has(symbol) {
					t.Fatalf("declared lookup of %s should be cached", ast.TSGetSymbolName(symbol))
				}
			}
			if !tp.links.ApiStabilityDeclaredSignature.Has(rawSignature(signature)) {
				t.Fatal("declared signature lookup should be cached")
			}
			for _, symbol := range []*ast.Symbol{leaky, tagged} {
				if apiStabilitySharedTypeEntry(tp, c.GetDeclaredTypeOfSymbol(symbol), apiStabilityExpand) != nil {
					t.Fatalf("a declared read published the %s type surface", ast.TSGetSymbolName(symbol))
				}
			}
			if apiStabilitySharedSignatureEntry(tp, signature) != nil {
				t.Fatal("a declared read published a signature surface")
			}

			if prime {
				used := tp.NewApiStabilitySession().UsedBySymbol(taggedFn)
				if used.incomplete || used.minimum != ApiStabilityStable || len(used.dependencies) != 0 {
					t.Fatalf("taggedFn surface should be empty, got min=%v deps=%v", used.minimum, apiStabilityDependencyNames(used))
				}
			}
		})
	}
}

func TestApiStabilityCachesAreCheckerLocal(t *testing.T) {
	t.Parallel()

	source := `/** @stability experimental */
export interface E { value: string }
export interface Leaky { value: E }
export const f = (x: E): E => x
/** @stability unstable */
export interface Shared { leak: E }
export interface Holder { child: Shared }
`
	c1, tp1, sf1, done1 := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done1()
	c2, tp2, sf2, done2 := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done2()
	primeApiStabilityChecker(t, c1)
	primeApiStabilityChecker(t, c2)

	if tp1.links == tp2.links {
		t.Fatal("different checkers must not share EffectLinks")
	}
	leaky1 := apiStabilityTestExport(t, c1, sf1, "Leaky")
	leaky2 := apiStabilityTestExport(t, c2, sf2, "Leaky")
	shared1 := c1.GetDeclaredTypeOfSymbol(apiStabilityTestExport(t, c1, sf1, "Shared"))
	shared2 := c2.GetDeclaredTypeOfSymbol(apiStabilityTestExport(t, c2, sf2, "Shared"))
	if leaky1 == leaky2 || shared1 == shared2 {
		t.Fatal("checkers should produce distinct identities")
	}

	query := func(t *testing.T, c *checker.Checker, tp *TypeParser, sf *ast.SourceFile) {
		t.Helper()
		if !containsDependency(tp.ApiStabilityUsedBySymbol(apiStabilityTestExport(t, c, sf, "Leaky")), "E") {
			t.Fatal("Leaky should expose E")
		}
		if !containsDependency(tp.ApiStabilityUsedBySignature(apiStabilityTestSignatures(t, c, apiStabilityTestExport(t, c, sf, "f"))[0]), "E") {
			t.Fatal("f's signature should expose E")
		}
		if !containsDependency(tp.NewApiStabilitySession().UsedBySymbol(apiStabilityTestExport(t, c, sf, "Holder")), "Shared") {
			t.Fatal("Holder should expose Shared")
		}
	}

	query(t, c1, tp1, sf1)
	if !tp1.links.ApiStabilityDeclaredSymbol.Has(leaky1) {
		t.Fatal("the first checker should cache its declared lookup")
	}
	if apiStabilitySharedTypeEntry(tp1, shared1, apiStabilityShallow) == nil {
		t.Fatal("the first checker should publish its shallow snapshot")
	}
	if tp2.links.ApiStabilityDeclaredSymbol.Has(leaky1) || apiStabilitySharedTypeEntry(tp2, shared1, apiStabilityShallow) != nil {
		t.Fatal("the second checker observed the first checker's caches")
	}
	if tp2.links.ApiStabilityDeclaredSymbol.Has(leaky2) || apiStabilitySharedTypeEntry(tp2, shared2, apiStabilityShallow) != nil {
		t.Fatal("the second checker cached results before its own queries")
	}

	query(t, c2, tp2, sf2)
	if tp1.links.ApiStabilityDeclaredSymbol.Has(leaky2) || apiStabilitySharedTypeEntry(tp1, shared2, apiStabilityShallow) != nil {
		t.Fatal("the first checker observed the second checker's caches")
	}
}
