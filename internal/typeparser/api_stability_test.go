package typeparser

import (
	"context"
	"strings"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
)

// apiStabilityTestExport looks up a module export by name so the cache tests can
// exercise the public TypeParser API the same way the diagnostic does.
func apiStabilityTestExport(t *testing.T, c *checker.Checker, sf *ast.SourceFile, name string) *ast.Symbol {
	t.Helper()
	module := checker.Checker_getSymbolOfDeclaration(c, sf.AsNode())
	if module == nil {
		t.Fatalf("no module symbol for %s", sf.FileName())
	}
	for _, symbol := range c.GetExportsOfModule(module) {
		if symbol != nil && symbol.Name == name {
			return symbol
		}
	}
	t.Fatalf("export %q not found in %s", name, sf.FileName())
	return nil
}

func apiStabilityDependencyNames(used ApiStabilityUsed) []string {
	names := make([]string, 0, len(used.Dependencies))
	for _, dependency := range used.Dependencies {
		names = append(names, ApiStabilityDependencyName(dependency))
	}
	return names
}

func containsDependency(used ApiStabilityUsed, name string) bool {
	for _, dependency := range used.Dependencies {
		if ApiStabilityDependencyName(dependency) == name {
			return true
		}
	}
	return false
}

// primeApiStabilityChecker type checks the program against the checker, the
// way the normal rule lifecycle does before a diagnostic run. Only then are the
// public types the analysis reads through its cached-materialization peeks
// available; an unprimed checker models a first run whose type-driven surfaces
// must stay incomplete (and are never reported as stable).
func primeApiStabilityChecker(t testing.TB, c *checker.Checker) {
	t.Helper()
	program, ok := c.Program().(*compiler.Program)
	if !ok {
		t.Fatalf("program is %T, want *compiler.Program", c.Program())
	}
	_ = program.GetSemanticDiagnostics(context.Background(), nil)
}

// TestApiStabilityDeclaredIndexInfosDoNotResolveComputedNames proves the
// declared index-info accessor inspects index signatures without resolving the
// symbol's late-bound member table: a cold interface carrying a recursively
// computed property name is inspected with zero checker instantiation and no
// compiler diagnostic.
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
	infos := c.GetDeclaredIndexInfosOfSymbol(x)
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

// TestApiStabilityDeclaredCacheSharedAcrossTypeParsers proves declared lookups
// persist per checker: a second TypeParser built from the same checker reuses
// the first parser's declared results, so a run over a different source file
// does not recompute the same tag lookup. Computed-surface persistence is
// covered by TestApiStabilityDeclaredPersistsAndCompleteSurfacesReuse.
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

	// A fresh parser over the same checker (the situation when another source
	// file later runs the rule) shares the declared EffectLinks cache.
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

	// A symbol declared in a different source file resolves through the same
	// declared store, which is what lets separate file runs reuse tag lookups.
	consumerSym := apiStabilityTestExport(t, c, consumer, "Consumer")
	consumerDeps := tp2.ApiStabilityUsedBySymbol(consumerSym)
	if !containsDependency(consumerDeps, "E") {
		t.Fatalf("Consumer should expose E directly, got %v", apiStabilityDependencyNames(consumerDeps))
	}
	if !tp1.links.ApiStabilityDeclaredSymbol.Has(consumerSym) {
		t.Fatal("first parser did not see the cross-file cached declared symbol")
	}
}

// TestApiStabilityNestedNamedSurfaceStaysShallow proves the documented shallow
// named-dependency boundary: a named interface reached inside another surface is
// reported as a dependency but its arbitrary members are not expanded into the
// owner's dependencies. Directly exposed callable signatures are still searched.
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

// TestApiStabilityCacheDeclaredCeilingIsPerAlias verifies the declared-stability
// cache stores the symbol's own declaration tag only. A tag on a selected-file
// `export ... from` declaration is source-specific and is applied by the rule,
// so it must not leak into the reusable symbol cache. Both aliases resolve to
// the same target, whose dependency surface is one shared per-checker result
// keyed by the concrete target identity, independent of the alias and of any
// forwarding ceiling.
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

	// The symbol-level lookup sees only the forwarded symbol's own declarations,
	// so the selected-file forwarding tag is not part of the shared cache.
	if level := tp.DeclaredApiStability(plain); level != ApiStabilityStable {
		t.Fatalf("plain reexport symbol level = %v, want stable", level)
	}
	if level := tp.DeclaredApiStability(tagged); level != ApiStabilityStable {
		t.Fatalf("selected-file forwarding tag leaked into the symbol cache: %v", level)
	}

	// Both aliases resolve to the same target, so the actual surface (which
	// contains E) is computed independently of any forwarding ceiling.
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
	for symbol != nil && symbol.Flags&ast.SymbolFlagsAlias != 0 {
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

// TestApiStabilityCacheDifferentCheckerIsolation proves two checkers do not
// share declared results or pointer identity and that each analyzer computes an
// independent session-local surface.
func TestApiStabilityCacheDifferentCheckerIsolation(t *testing.T) {
	t.Parallel()

	source := `/** @stability experimental */
interface E { value: string }
export interface Leaky { value: E }
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

	sym1 := apiStabilityTestExport(t, c1, sf1, "Leaky")
	sym2 := apiStabilityTestExport(t, c2, sf2, "Leaky")
	if sym1 == sym2 {
		t.Fatal("checkers should produce distinct symbol pointers")
	}

	if !containsDependency(tp1.ApiStabilityUsedBySymbol(sym1), "E") {
		t.Fatal("first checker surface should expose E")
	}
	if !tp1.links.ApiStabilityDeclaredSymbol.Has(sym1) {
		t.Fatal("first checker should have cached its declared lookup")
	}
	if tp2.links.ApiStabilityDeclaredSymbol.Has(sym1) {
		t.Fatal("second checker observed the first checker's declared cache")
	}
	if !containsDependency(tp2.ApiStabilityUsedBySymbol(sym2), "E") {
		t.Fatal("second checker surface should expose E independently")
	}
	if tp1.links.ApiStabilityDeclaredSymbol.Has(sym2) {
		t.Fatal("first checker observed the second checker's declared cache")
	}
}

// TestApiStabilityCacheGenericArgumentsDoNotContaminate proves the type-keyed
// cache keeps distinct instantiations distinct: an experimental type argument
// in one instantiation must not appear in another, in both declaration orders.
func TestApiStabilityCacheGenericArgumentsDoNotContaminate(t *testing.T) {
	t.Parallel()

	for _, order := range []string{"leaky-first", "clean-first"} {
		t.Run(order, func(t *testing.T) {
			t.Parallel()
			leakyDeclaration := "export type Leaky = Box<E>"
			cleanDeclaration := "export type Clean = Box<string>"
			declarations := leakyDeclaration + "\n" + cleanDeclaration
			if order == "clean-first" {
				declarations = cleanDeclaration + "\n" + leakyDeclaration
			}

			c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
				"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
interface Box<T> { value: T }
` + declarations + "\n",
			})
			defer done()
			primeApiStabilityChecker(t, c)

			leaky := apiStabilityTestExport(t, c, files["/.src/test.ts"], "Leaky")
			clean := apiStabilityTestExport(t, c, files["/.src/test.ts"], "Clean")

			if !containsDependency(tp.ApiStabilityUsedBySymbol(leaky), "E") {
				t.Fatal("Box<E> should expose E")
			}
			if containsDependency(tp.ApiStabilityUsedBySymbol(clean), "E") {
				t.Fatalf("Box<string> must not inherit E, got %v", apiStabilityDependencyNames(tp.ApiStabilityUsedBySymbol(clean)))
			}
		})
	}
}

// TestApiStabilityDeclaredReadsDoNotCompute proves the declared caches are
// independent of the computed caches: asking for a declared level must never
// trigger a symbol or signature surface computation.
func TestApiStabilityDeclaredReadsDoNotCompute(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
export interface Leaky { value: E }
export function f(x: E): E { return x }
`,
	})
	defer done()

	leaky := apiStabilityTestExport(t, c, files["/.src/test.ts"], "Leaky")
	f := apiStabilityTestExport(t, c, files["/.src/test.ts"], "f")
	signatures := apiStabilityTestSignatures(t, c, f)

	before := c.TotalInstantiationCount
	if level := tp.DeclaredApiStability(leaky); level != ApiStabilityStable {
		t.Fatalf("Leaky declared = %v, want stable", level)
	}
	_ = tp.DeclaredApiStabilityOfSignature(signatures[0])
	if delta := c.TotalInstantiationCount - before; delta != 0 {
		t.Fatalf("declared reads instantiated %d types, want 0", delta)
	}

	// A fresh computed session starts empty: declared reads never populate a
	// symbol or signature memo and never publish a shared surface.
	session := tp.NewApiStabilitySession()
	if len(session.analysis.sessionSymbols) != 0 || len(session.analysis.sessionTypes) != 0 ||
		len(session.analysis.sessionSignatures) != 0 || len(session.analysis.sessionComponents) != 0 {
		t.Fatal("declared reads left computed analysis state")
	}
	if !tp.links.ApiStabilityDeclaredSymbol.Has(leaky) {
		t.Fatal("declared symbol should be cached")
	}
	if !tp.links.ApiStabilityDeclaredSignature.Has(rawSignature(signatures[0])) {
		t.Fatal("declared signature should be cached")
	}
}

// TestApiStabilitySignatureDeclaredCacheKeepsOverloadsDistinct proves a tagged
// overload keeps its own declared level even when the symbol-level lookup picks
// up another declaration's tag, and that both are cached separately.
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
	if first.Level == second.Level {
		t.Fatalf("distinct overload tags collapsed: %v and %v", first.Level, second.Level)
	}
	if first.Level != ApiStabilityStable || second.Level != ApiStabilityUnstable {
		t.Fatalf("unexpected overload levels: first=%v second=%v", first.Level, second.Level)
	}
	if !tp.links.ApiStabilityDeclaredSignature.Has(rawSignature(signatures[0])) ||
		!tp.links.ApiStabilityDeclaredSignature.Has(rawSignature(signatures[1])) {
		t.Fatal("both overload declarations should be cached separately")
	}
}

// apiStabilityTestSignatures returns the public call signatures of an exported
// symbol so the cache tests can exercise the signature APIs directly.
func apiStabilityTestSignatures(t *testing.T, c *checker.Checker, symbol *ast.Symbol) []*checker.Signature {
	t.Helper()
	if symbol == nil {
		t.Fatal("nil symbol")
	}
	signatures := c.GetSignaturesOfType(c.GetTypeOfSymbol(symbol), checker.SignatureKindCall)
	if len(signatures) == 0 {
		t.Fatalf("symbol %q has no call signatures", symbol.Name)
	}
	return signatures
}

// TestApiStabilitySignatureSurfaceRetainsSubstitution proves an instantiated
// inferred return keeps its represented argument while the raw target stays
// shallow, and that a session memoizes the completed signature surface.
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

	// The raw target's type parameter is not an experimental dependency, so the
	// shallow form must not report E.
	raw := rawSignature(instantiated)
	if raw == instantiated {
		t.Fatal("expected an instantiated signature with a raw target")
	}
	shallow := tp.ApiStabilityUsedBySignature(raw)
	if containsDependency(shallow, "E") {
		t.Fatalf("raw target should stay shallow, got %v", apiStabilityDependencyNames(shallow))
	}
}

// TestApiStabilityComputedSignatureCacheSeparatesSubstitutionContexts proves the
// same raw signature analyzed under different enclosing substitutions is not
// conflated: the clean instantiation stays clean and the leaking one reports,
// regardless of which is computed first.
func TestApiStabilityComputedSignatureCacheSeparatesSubstitutionContexts(t *testing.T) {
	t.Parallel()

	for _, order := range []string{"clean-first", "leaky-first"} {
		t.Run(order, func(t *testing.T) {
			t.Parallel()
			clean := "export const clean = make<string>()"
			leaky := "export const leaky = make<E>()"
			declarations := clean + "\n" + leaky
			if order == "leaky-first" {
				declarations = leaky + "\n" + clean
			}
			c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
				"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
declare function make<T>(): (x: T) => T
` + declarations + "\n",
			})
			defer done()
			primeApiStabilityChecker(t, c)

			cleanSym := apiStabilityTestExport(t, c, files["/.src/test.ts"], "clean")
			leakySym := apiStabilityTestExport(t, c, files["/.src/test.ts"], "leaky")

			cleanUsed := tp.ApiStabilityUsedBySymbol(cleanSym)
			if containsDependency(cleanUsed, "E") {
				t.Fatalf("clean instantiation must not inherit E, got %v", apiStabilityDependencyNames(cleanUsed))
			}
			leakyUsed := tp.ApiStabilityUsedBySymbol(leakySym)
			if !containsDependency(leakyUsed, "E") {
				t.Fatalf("leaky instantiation should expose E, got %v", apiStabilityDependencyNames(leakyUsed))
			}
		})
	}
}

// TestApiStabilityComputedSignatureCacheSeparatesNestedSubstitutionContexts
// covers the same contamination through a nested inferred object surface. The
// computed signature cache is keyed by the concrete signature identity, so the
// clean and leaking materializations never share a cached result in either
// order.
func TestApiStabilityComputedSignatureCacheSeparatesNestedSubstitutionContexts(t *testing.T) {
	t.Parallel()

	for _, order := range []string{"clean-first", "leaky-first"} {
		t.Run(order, func(t *testing.T) {
			t.Parallel()
			clean := "export const clean = make<string>()"
			leaky := "export const leaky = make<E>()"
			declarations := clean + "\n" + leaky
			if order == "leaky-first" {
				declarations = leaky + "\n" + clean
			}
			c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
				"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
declare function make<T>(): { nested: { fn: (x: T) => T } }
` + declarations + "\n",
			})
			defer done()
			primeApiStabilityChecker(t, c)

			cleanSym := apiStabilityTestExport(t, c, files["/.src/test.ts"], "clean")
			leakySym := apiStabilityTestExport(t, c, files["/.src/test.ts"], "leaky")

			cleanUsed := tp.ApiStabilityUsedBySymbol(cleanSym)
			if containsDependency(cleanUsed, "E") {
				t.Fatalf("clean nested instantiation must not inherit E, got %v", apiStabilityDependencyNames(cleanUsed))
			}
			leakyUsed := tp.ApiStabilityUsedBySymbol(leakySym)
			if !containsDependency(leakyUsed, "E") {
				t.Fatalf("leaky nested instantiation should expose E, got %v", apiStabilityDependencyNames(leakyUsed))
			}
		})
	}
}

// TestApiStabilityComputedIncludesDeclaredMemberStability proves the computed
// minimum covers the declared stability of public properties and call
// signatures, with provenance naming the offender.
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
	if oUsed.Minimum != ApiStabilityExperimental {
		t.Fatalf("O minimum = %v, want experimental: %v", oUsed.Minimum, apiStabilityDependencyNames(oUsed))
	}
	if !containsDependency(oUsed, "value") {
		t.Fatalf("O should expose its tagged property, got %v", apiStabilityDependencyNames(oUsed))
	}

	s := apiStabilityTestExport(t, c, files["/.src/test.ts"], "S")
	sUsed := tp.ApiStabilityUsedBySymbol(s)
	if sUsed.Minimum != ApiStabilityUnstable {
		t.Fatalf("S minimum = %v, want unstable: %v", sUsed.Minimum, apiStabilityDependencyNames(sUsed))
	}
	foundSignature := false
	for _, dependency := range sUsed.Dependencies {
		if dependency.Signature != nil && dependency.Level == ApiStabilityUnstable && dependency.Declaration != nil {
			foundSignature = true
		}
	}
	if !foundSignature {
		t.Fatalf("S should expose its tagged call signature with provenance, got %v", apiStabilityDependencyNames(sUsed))
	}
}

// TestApiStabilityReplacedDefaultStaysClean proves a generic default replaced by
// an explicit argument is not part of the represented surface, including the
// annotation provenance pass.
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

// TestApiStabilitySignatureCacheIsolation proves a computed signature surface
// is checker-local: two checkers compute the same-looking signature
// independently and neither can observe the other's analysis.
func TestApiStabilitySignatureCacheIsolation(t *testing.T) {
	t.Parallel()

	source := `/** @stability experimental */
interface E { value: string }
export const f = (x: E): E => x
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

	f1 := apiStabilityTestExport(t, c1, sf1, "f")
	f2 := apiStabilityTestExport(t, c2, sf2, "f")
	sig1 := apiStabilityTestSignatures(t, c1, f1)[0]
	sig2 := apiStabilityTestSignatures(t, c2, f2)[0]

	if !containsDependency(tp1.ApiStabilityUsedBySignature(sig1), "E") {
		t.Fatal("first checker signature should expose E")
	}
	if !containsDependency(tp2.ApiStabilityUsedBySignature(sig2), "E") {
		t.Fatal("second checker signature should expose E independently")
	}
	// A fresh session never observes an earlier session's local signature
	// memo, whichever checker or signature produced an earlier result; only a
	// published complete snapshot can be reused, and it is composed anew.
	if session := tp2.NewApiStabilitySession(); len(session.analysis.sessionSignatures) != 0 {
		t.Fatal("a fresh session observed an earlier session's local signature memo")
	}
}

// TestApiStabilityCacheCyclesAndEmptyResultsAreSafe proves a cyclic surface
// still reports its leak, that settled results are reusable inside the session
// that computed them, and that a stable/empty surface is a complete memo entry.
func TestApiStabilityCacheCyclesAndEmptyResultsAreSafe(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
export interface Node { next: Node; payload: E }
type B = { c: C; payload: E }
type C = { b: B }
export type A = { b: B }
export type Z = { c: C }
export interface Stable { value: string }
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	testFile := files["/.src/test.ts"]
	node := apiStabilityTestExport(t, c, testFile, "Node")
	session := tp.NewApiStabilitySession()
	if !containsDependency(session.UsedBySymbol(node), "E") {
		t.Fatal("self-referential Node should expose E")
	}
	// A settled cyclic surface is complete and reusable within its session.
	if surface := session.analysis.sessionSymbols[node]; !surface.complete || surface.blocked {
		t.Fatal("a settled cyclic surface should be a complete session result")
	}
	work := session.analysis.work
	if !containsDependency(session.UsedBySymbol(node), "E") {
		t.Fatal("repeated session query lost Node's E")
	}
	if session.analysis.work != work {
		t.Fatal("repeated session query recomputed the settled cyclic surface")
	}
	// The same symbol computation starts fresh for a new export: symbol results
	// stay analysis-local, and only complete type/signature snapshots may be
	// reused by a later session.
	if fresh := tp.NewApiStabilitySession(); len(fresh.analysis.sessionSymbols) != 0 {
		t.Fatal("a fresh session observed the earlier computed symbol surface")
	}

	// A leak reached through a mutual cycle must be reported for every root,
	// regardless of which root was analyzed first.
	a := apiStabilityTestExport(t, c, testFile, "A")
	z := apiStabilityTestExport(t, c, testFile, "Z")
	if !containsDependency(tp.ApiStabilityUsedBySymbol(a), "E") {
		t.Fatal("A should expose E through the B/C cycle")
	}
	if !containsDependency(tp.ApiStabilityUsedBySymbol(z), "E") {
		t.Fatal("Z should expose E through the B/C cycle")
	}
	if fresh := tp.NewApiStabilitySession(); len(fresh.analysis.sessionSymbols) != 0 {
		t.Fatal("mutual-cycle queries published symbol state")
	}

	stable := apiStabilityTestExport(t, c, testFile, "Stable")
	stableSession := tp.NewApiStabilitySession()
	stableUsed := stableSession.UsedBySymbol(stable)
	if len(stableUsed.Dependencies) != 0 {
		t.Fatalf("Stable should have no dependencies, got %v", apiStabilityDependencyNames(stableUsed))
	}
	if surface := stableSession.analysis.sessionSymbols[stable]; !surface.complete || surface.blocked {
		t.Fatal("stable/empty results should be complete session entries")
	}
}

// TestApiStabilityConcreteCarrierSeparatesSharedRawTargets proves the type and
// signature caches keep concrete identities: the same raw component (an
// interface member whose type depends on the interface's type parameter) is
// analyzed under two distinct concrete outer materializations and must not
// contaminate either direction, in both declaration orders.
func TestApiStabilityConcreteCarrierSeparatesSharedRawTargets(t *testing.T) {
	t.Parallel()

	for _, order := range []string{"clean-first", "leaky-first"} {
		t.Run(order, func(t *testing.T) {
			t.Parallel()
			clean := "export declare const clean: DeepHolder<string>"
			leaky := "export declare const leaky: DeepHolder<E>"
			declarations := clean + "\n" + leaky
			if order == "leaky-first" {
				declarations = leaky + "\n" + clean
			}
			c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
				"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
type Deep<T> = T extends string ? Deep<T> : T
interface DeepHolder<T> { value: Deep<T> }
` + declarations + "\n",
			})
			defer done()
			primeApiStabilityChecker(t, c)

			cleanSym := apiStabilityTestExport(t, c, files["/.src/test.ts"], "clean")
			leakySym := apiStabilityTestExport(t, c, files["/.src/test.ts"], "leaky")
			if containsDependency(tp.ApiStabilityUsedBySymbol(cleanSym), "E") {
				t.Fatal("DeepHolder<string> must not expose E")
			}
			if !containsDependency(tp.ApiStabilityUsedBySymbol(leakySym), "E") {
				t.Fatal("DeepHolder<E> should expose E")
			}
		})
	}
}

// TestApiStabilityConcreteCarrierSeparatesHeritageMaterializations covers the
// same concrete-identity requirement through inherited members: two exports
// inherit the same shared base declaration with different concrete arguments.
func TestApiStabilityConcreteCarrierSeparatesHeritageMaterializations(t *testing.T) {
	t.Parallel()

	for _, order := range []string{"clean-first", "leaky-first"} {
		t.Run(order, func(t *testing.T) {
			t.Parallel()
			clean := "export interface Clean extends Base<string> {}"
			leaky := "export interface Leaky extends Base<E> {}"
			declarations := clean + "\n" + leaky
			if order == "leaky-first" {
				declarations = leaky + "\n" + clean
			}
			c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
				"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
interface Base<T> { value: T }
` + declarations + "\n",
			})
			defer done()
			primeApiStabilityChecker(t, c)

			cleanSym := apiStabilityTestExport(t, c, files["/.src/test.ts"], "Clean")
			leakySym := apiStabilityTestExport(t, c, files["/.src/test.ts"], "Leaky")
			if containsDependency(tp.ApiStabilityUsedBySymbol(cleanSym), "E") {
				t.Fatal("Clean extends Base<string> and must not expose E")
			}
			if !containsDependency(tp.ApiStabilityUsedBySymbol(leakySym), "E") {
				t.Fatal("Leaky extends Base<E> and should expose E")
			}
		})
	}
}

// TestApiStabilityCycleOrderDoesNotHideLeaks analyzes the mutual cycle roots in
// the reverse order so a cached intermediate result can never suppress the leak.
func TestApiStabilityCycleOrderDoesNotHideLeaks(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
type B = { c: C; payload: E }
type C = { b: B }
export type A = { b: B }
export type Z = { c: C }
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	testFile := files["/.src/test.ts"]
	z := apiStabilityTestExport(t, c, testFile, "Z")
	if !containsDependency(tp.ApiStabilityUsedBySymbol(z), "E") {
		t.Fatal("Z analyzed first should still expose E through the cycle")
	}
	a := apiStabilityTestExport(t, c, testFile, "A")
	if !containsDependency(tp.ApiStabilityUsedBySymbol(a), "E") {
		t.Fatal("A analyzed second should still expose E through the cached cycle")
	}
}

// TestApiStabilityColdLazyReadsMaterialize proves a cold public surface is
// materialized through the checker's ordinary lazy reads: the analysis no
// longer needs the language service to warm the checker first. The cold surface
// exposes E and is complete in its session, and the same checker after type
// checking recomputes to the same surface from a fresh session.
func TestApiStabilityColdLazyReadsMaterialize(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
type Deep<T> = T extends string ? Deep<T> : T
interface Callable<T> { (x: T): void }
export declare const f: Callable<Deep<E>>
`,
	})
	defer done()

	f := apiStabilityTestExport(t, c, files["/.src/test.ts"], "f")
	coldSession := tp.NewApiStabilitySession()
	cold := coldSession.UsedBySymbol(f)
	if !containsDependency(cold, "E") {
		t.Fatalf("the cold surface should lazily materialize E, got %v", apiStabilityDependencyNames(cold))
	}
	if surface := coldSession.analysis.sessionSymbols[f]; !surface.complete || surface.blocked {
		t.Fatal("a complete cold surface should settle complete in its session")
	}

	// A warmed checker materializes the declaration the way the language
	// service would, and a fresh session agrees with the cold result.
	primeApiStabilityChecker(t, c)
	fresh := NewTypeParser(c.Program(), c)
	warm := fresh.ApiStabilityUsedBySymbol(f)
	if !containsDependency(warm, "E") {
		t.Fatal("the materialized surface should expose E")
	}
	if cold.Minimum != warm.Minimum || len(cold.Dependencies) != len(warm.Dependencies) {
		t.Fatalf("cold and warm results disagree: cold=%v warm=%v", apiStabilityDependencyNames(cold), apiStabilityDependencyNames(warm))
	}
}

// TestApiStabilityColdLazyFiniteApplicationMatchesWarm proves a finite
// recursive alias application resolves through the ordinary lazy read on a
// genuinely cold checker, and the cold session result matches the materialized
// recomputation in a new session.
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

	// The materialized checker is recomputed by a fresh session; it must agree
	// with the first cold and warm results.
	recomputed := NewTypeParser(c.Program(), c).ApiStabilityUsedBySymbol(x)
	if !containsDependency(recomputed, "E") {
		t.Fatalf("the materialized recomputation should expose E, got %v", apiStabilityDependencyNames(recomputed))
	}
	if recomputed.Minimum != warm.Minimum || len(recomputed.Dependencies) != len(warm.Dependencies) {
		t.Fatalf("warm results disagree: first=%v recomputed=%v", apiStabilityDependencyNames(warm), apiStabilityDependencyNames(recomputed))
	}
}

// TestApiStabilityExplicitStableDeclaredPresence pins that branch-style
// `@stability stable` tags are parsed as an explicit value, not as an absent
// tag. The root symbol and each public signature keep their declaration
// provenance in the per-checker declared caches, while an untagged overload
// stays stable with no declaration. The leak rule's ceiling and the forwarding
// presence check read these caches, so explicit stable roots and explicitly
// stable signatures stay distinguishable from untagged ones.
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
	if taggedDeclared.Level != ApiStabilityStable || taggedDeclared.Declaration == nil {
		t.Fatalf("explicit stable root lost its provenance: %+v", taggedDeclared)
	}
	if !tp.links.ApiStabilityDeclaredSymbol.Has(tagged) {
		t.Fatal("explicit stable root should be cached")
	}
	if declared := tp.DeclaredApiStabilityOfSymbol(untagged); declared.Level != ApiStabilityStable || declared.Declaration != nil {
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
			if declared.Level != ApiStabilityStable || declared.Declaration == nil {
				t.Fatalf("explicit stable overload lost its provenance: %+v", declared)
			}
			if !tp.links.ApiStabilityDeclaredDeclaration.Has(declared.Declaration) {
				t.Fatal("explicit stable declaration should be cached")
			}
		} else if declared.Level != ApiStabilityStable || declared.Declaration != nil {
			t.Fatalf("untagged overload should be stable without provenance: %+v", declared)
		}
	}
	for index, signature := range untaggedSignatures {
		if declared := tp.DeclaredApiStabilityOfSignature(signature); declared.Declaration != nil {
			t.Fatalf("untagged signature %d has a declaration: %+v", index, declared)
		}
	}
}

// TestApiStabilityComputedPublicMembers pins that a root public surface
// traverses public computed members of interfaces, classes and anonymous object
// types: well-known symbols, custom unique symbols and literal computed names
// all contribute their represented member types, while private and protected
// computed members stay excluded. It also pins that a checker late-bound name
// never leaks into a diagnostic: a tagged computed member renders its readable
// declaration spelling.
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
	for _, dependency := range used.Dependencies {
		rendered[ApiStabilityDependencyName(dependency)] = true
		if strings.HasPrefix(ApiStabilityDependencyName(dependency), "\xfe") {
			t.Errorf("late-bound internal name leaked into a diagnostic: %q", ApiStabilityDependencyName(dependency))
		}
	}
	if !rendered["[Symbol.iterator]"] || !rendered["[custom]"] {
		t.Errorf("tagged computed members should render readable names, got %v", apiStabilityDependencyNames(used))
	}
	if len(used.Dependencies) < 2 {
		t.Errorf("tagged computed members should report separately, got %v", apiStabilityDependencyNames(used))
	}
}

// TestApiStabilityEarlyTableComputedMembersNotCachedComplete pins the
// complete-result invariant for the early member table: an interface whose
// computed member has not been late-bound yet must stay incomplete and never be
// reported as a complete surface, because its early table omits the member.
// Once the checker materializes the member table a retry settles complete with
// the computed dependency.
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
		if _, resolved := c.GetResolvedMembersOfTypeIfMaterialized(declared); resolved {
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
		if _, resolved := c.GetResolvedMembersOfTypeIfMaterialized(declared); !resolved {
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

// TestApiStabilitySettleSkipRequiresNoAdvancement pins the corrected
// settlement predicate: a blocked surface is only skipped after the guard
// budget is exhausted when no analysis read advanced the materialization epoch
// since it was collected and no earlier settle round advanced any surface.
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

// TestApiStabilitySettleRecollectsMaterializedMember pins the
// P1 within one analysis: a component the guard refused while the budget was
// exhausted can be materialized by a later ordinary read in the same collection,
// and settlement must recollect the blocked root instead of assuming it cannot
// advance. The result still stays blocked so a later analysis retries it,
// and no new guard read is consumed.
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
	value := sym.Members["a"]
	functionType := c.GetTypeOfSymbol(sym.Members["z"])
	signatures := c.GetSignaturesOfType(functionType, checker.SignatureKindCall)
	if len(signatures) != 1 {
		t.Fatal("signature missing")
	}
	if c.GetResolvedReturnTypeOfSignatureIfMaterialized(signatures[0]) != nil {
		t.Fatal("expected cold inferred return")
	}
	if c.GetResolvedTypeOfSymbolIfMaterialized(value) != nil {
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
		if c.GetResolvedTypeOfSymbolIfMaterialized(value) == nil {
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

	// A later analysis on the materialized checker retries and settles complete.
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

// TestApiStabilityNaturalBudgetMaterializationRetry reproduces the natural-budget
// probe: the shared guard budget is exhausted by an
// ordinary member while a later inferred return materializes a previously
// refused experimental member. Settlement must collect the finding, keep the
// side effects bounded and leave the surface blocked for a later retry.
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
	value := sym.Members["a"]
	functionType := c.GetTypeOfSymbol(sym.Members["z"])
	signatures := c.GetSignaturesOfType(functionType, checker.SignatureKindCall)
	if len(signatures) != 1 {
		t.Fatal("signature missing")
	}
	if c.GetResolvedReturnTypeOfSignatureIfMaterialized(signatures[0]) != nil {
		t.Fatal("expected cold inferred return")
	}
	if c.GetResolvedTypeOfSymbolIfMaterialized(value) != nil {
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

// TestApiStabilityRecoveryPropagatesFindingsToEarlierResults pins the
// recovery settlement defect: a recovery round can discover a
// finding in a component after the fixpoint has converged, but the blocked
// parent that was collected before that discovery must be revisited within the
// same bounded recovery loop. Advancing the materialization epoch after a
// changed recovery round makes exactly those earlier results stale, so the root
// reflects the finding its own component discovered even though the analysis
// stays exhausted and the root remains blocked and never settled as stable.
//
// Without the advancement the component holds `E` after the recovery sweep
// while the root still reports nothing: the export's returned Used set (what
// the rule reports) misses the dependency, and the caller sees a stable
// surface that a later analysis would have reported. The test consumes no
// additional guard work: recovery recollects represented components through
// ordinary reads, it never authorizes a new guarded read.
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
		if m.Name == "a" {
			value = m
		}
		if m.Name == "z" {
			fnSymbol = m
		}
	}
	if value == nil || fnSymbol == nil {
		t.Fatal("expected members a and z")
	}
	fnType := c.GetTypeOfSymbol(fnSymbol)
	signature := c.GetSignaturesOfType(fnType, checker.SignatureKindCall)[0]
	if c.GetResolvedReturnTypeOfSignatureIfMaterialized(signature) != nil ||
		c.GetResolvedTypeOfSymbolIfMaterialized(value) != nil {
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
		// A cycle-cut signature contributes no findings until it is recollected,
		// which is what makes the recovery sweep the only round that can find it.
		// The result is stored directly, exactly like an independent seeded
		// analysis; settlement must reconcile it into its visit order.
		analysis.sessionSignatures[apiStabilitySignatureKey{signature: signature}] = cycleCutSurface()
		analysis.symbolSurface(sym)
		if c.GetResolvedTypeOfSymbolIfMaterialized(value) != nil {
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

// TestApiStabilityDeclaredPersistsAndCompleteSurfacesReuse pins the
// current cache policy (updated from the original local-only policy,
// which the latest shared-surface design supersedes): declared symbol/signature
// stability persists per checker and is shared across analyses and TypeParser
// instances; a complete, settled, context-free concrete type or signature
// surface is published to the per-checker shared caches and reused by later
// exports, files and parsers; symbol results and every incomplete or
// substitution-context result stay local to the session that computed them.
// Declared lookups never trigger a computed analysis.
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

	// A fresh session starts with no local computed state: symbol results and
	// unpublishable results are never carried by an earlier session.
	session := tp.NewApiStabilitySession()
	if len(session.analysis.sessionSymbols) != 0 || len(session.analysis.sessionTypes) != 0 ||
		len(session.analysis.sessionSignatures) != 0 || len(session.analysis.sessionComponents) != 0 {
		t.Fatal("a fresh session must start with no local computed state")
	}

	used := session.UsedBySymbol(leaky)
	if used.Incomplete || !containsDependency(used, "E") {
		t.Fatalf("Leaky should be a complete surface exposing E, got %v (incomplete=%v)", apiStabilityDependencyNames(used), used.Incomplete)
	}

	// Repeating the same component inside the export reuses the session memo:
	// the second query consumes no analysis work and returns the same surface.
	work := session.analysis.work
	repeat := session.UsedBySymbol(leaky)
	if repeat.Incomplete || !containsDependency(repeat, "E") {
		t.Fatalf("repeated session query lost E, got %v (incomplete=%v)", apiStabilityDependencyNames(repeat), repeat.Incomplete)
	}
	if session.analysis.work != work {
		t.Fatal("repeated session query recomputed a completed surface")
	}

	// The complete concrete type surface was published: a new export still
	// starts with an empty symbol memo, yet composes the same result and a
	// direct type query consumes no analysis work at all.
	declared := c.GetDeclaredTypeOfSymbol(leaky)
	if apiStabilitySharedTypeEntry(tp, declared, apiStabilityExpand) == nil {
		t.Fatal("the complete Leaky type surface should be published for reuse")
	}
	next := tp.NewApiStabilitySession()
	if len(next.analysis.sessionSymbols) != 0 {
		t.Fatal("a new export must not observe an earlier export's symbol memo")
	}
	nextUsed := next.UsedBySymbol(alsoLeaky)
	if nextUsed.Incomplete || !containsDependency(nextUsed, "E") {
		t.Fatalf("AlsoLeaky should be a complete surface exposing E, got %v (incomplete=%v)", apiStabilityDependencyNames(nextUsed), nextUsed.Incomplete)
	}
	direct := newApiStabilityAnalysis(tp)
	direct.typeSurface(declared, apiStabilityExpand, apiStabilitySubstitution{})
	if direct.work != 0 {
		t.Fatalf("the published type snapshot consumed %d work", direct.work)
	}

	// Declared stability persists per checker and is shared across parsers;
	// the declared read never triggers a computed analysis or a publication.
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

	// Reused snapshots still produce the same surface content.
	fresh := NewTypeParser(c.Program(), c).NewApiStabilitySession()
	again := fresh.UsedBySymbol(leaky)
	if again.Incomplete || len(again.Dependencies) != len(used.Dependencies) {
		t.Fatalf("a later session disagrees with the first: first=%v again=%v", apiStabilityDependencyNames(used), apiStabilityDependencyNames(again))
	}
}

// TestApiStabilityIncompleteIsNotStable pins that a surface the checker
// has not materialized reports Incomplete from the public result, publishes
// nothing, and that a later session over the materialized checker recomputes a
// complete result: an incomplete result is never carried over as a finished
// surface.
func TestApiStabilityIncompleteIsNotStable(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
export interface WellKnownOnly { plain: number; [Symbol.iterator](): E }
`,
	})
	defer done()

	symbol := apiStabilityTestExport(t, c, files["/.src/test.ts"], "WellKnownOnly")
	declared := c.GetDeclaredTypeOfSymbol(symbol)
	cold := tp.ApiStabilityUsedBySymbol(symbol)
	if !cold.Incomplete {
		t.Fatal("an unmaterialized computed member must report an incomplete result")
	}
	if apiStabilitySharedTypeEntry(tp, declared, apiStabilityExpand) != nil {
		t.Fatal("an incomplete result must never be published")
	}

	primeApiStabilityChecker(t, c)
	warm := NewTypeParser(c.Program(), c).ApiStabilityUsedBySymbol(symbol)
	if warm.Incomplete || !containsDependency(warm, "E") {
		t.Fatalf("the materialized retry should settle complete with E, got %v (incomplete=%v)", apiStabilityDependencyNames(warm), warm.Incomplete)
	}
	if apiStabilitySharedTypeEntry(tp, declared, apiStabilityExpand) == nil {
		t.Fatal("the materialized retry should publish its complete surface")
	}
}
