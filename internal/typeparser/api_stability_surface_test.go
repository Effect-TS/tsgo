package typeparser

import (
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

// assertApiStabilitySurfaceExcludesOwnTag asserts that a surface result carries
// none of the root's own findings. name is the display name the root's own tag
// would report under; other names are legitimate child dependencies.
func assertApiStabilitySurfaceExcludesOwnTag(t *testing.T, used ApiStabilityUsed, name string) {
	t.Helper()
	for _, dependency := range used.Dependencies {
		if ApiStabilityDependencyName(dependency) == name {
			t.Fatalf("root tag %q leaked into its own surface result: %v", name, apiStabilityDependencyNames(used))
		}
	}
}

// TestApiStabilityRootTagIsExcludedFromResult pins the
// declared-versus-surface split for every root kind: declared stability
// is the component's own tag, while the surface result is the minimum
// guaranteed stability of the component's exposed children and excludes the
// root's own tag. The declared accessors keep returning the own tag with its
// declaration provenance, and the traversal still descends through a tagged
// root, so a tagged root's children keep being reported.
func TestApiStabilityRootTagIsExcludedFromResult(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
export interface E { value: string }

/** @stability unstable */
export interface TaggedRoot { value: string }

/** @stability experimental */
export interface TaggedExperimentalRoot { value: string }

/** @stability stable */
export interface ExplicitStableRoot { value: string }

/** @stability unstable */
export interface TaggedRootWithChild { member: E }

/** @stability unstable */
export function taggedFn(x: string): string

/** @stability unstable */
export function taggedLeaky(): E

/** @stability unstable */
export type TaggedAlias = { value: string }

/** @stability unstable */
export type TaggedAliasLeaky = { value: E }
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	testFile := files["/.src/test.ts"]
	session := tp.NewApiStabilitySession()

	// The declared accessors keep the own tag and its provenance for every root
	// kind. This read never triggers the computed surface.
	taggedRoot := apiStabilityTestExport(t, c, testFile, "TaggedRoot")
	taggedRootDeclared := tp.DeclaredApiStabilityOfSymbol(taggedRoot)
	if taggedRootDeclared.Level != ApiStabilityUnstable || taggedRootDeclared.Declaration == nil {
		t.Fatalf("TaggedRoot declared = %+v, want unstable with provenance", taggedRootDeclared)
	}
	experimentalDeclared := tp.DeclaredApiStabilityOfSymbol(apiStabilityTestExport(t, c, testFile, "TaggedExperimentalRoot"))
	if experimentalDeclared.Level != ApiStabilityExperimental || experimentalDeclared.Declaration == nil {
		t.Fatalf("TaggedExperimentalRoot declared = %+v, want experimental with provenance", experimentalDeclared)
	}
	explicitStableDeclared := tp.DeclaredApiStabilityOfSymbol(apiStabilityTestExport(t, c, testFile, "ExplicitStableRoot"))
	if explicitStableDeclared.Level != ApiStabilityStable || explicitStableDeclared.Declaration == nil {
		t.Fatalf("an explicit stable root must stay distinct from an absent tag: %+v", explicitStableDeclared)
	}

	tagOnly := []struct {
		name      string
		inspector func() ApiStabilityUsed
	}{
		{"TaggedRoot", func() ApiStabilityUsed { return session.UsedBySymbol(taggedRoot) }},
		{"TaggedRoot type", func() ApiStabilityUsed { return session.UsedByType(c.GetDeclaredTypeOfSymbol(taggedRoot)) }},
		{
			"taggedFn", func() ApiStabilityUsed {
				return session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "taggedFn"))
			},
		},
		{"TaggedAlias", func() ApiStabilityUsed {
			return session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "TaggedAlias"))
		}},
		{"TaggedExperimentalRoot", func() ApiStabilityUsed {
			return session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "TaggedExperimentalRoot"))
		}},
		{"ExplicitStableRoot", func() ApiStabilityUsed {
			return session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "ExplicitStableRoot"))
		}},
	}
	for _, test := range tagOnly {
		used := test.inspector()
		if used.Incomplete {
			t.Fatalf("%s: a leaf surface with only an own tag must settle complete", test.name)
		}
		if used.Minimum != ApiStabilityStable || len(used.Dependencies) != 0 {
			t.Fatalf("%s: own tag must be excluded, got min=%v deps=%v", test.name, used.Minimum, apiStabilityDependencyNames(used))
		}
	}

	// A tagged function's own declaration tag is recorded as a signature
	// finding; it is still the root's own tag and stays excluded.
	taggedFn := apiStabilityTestExport(t, c, testFile, "taggedFn")
	fnDeclared := tp.DeclaredApiStabilityOfSignature(apiStabilityTestSignatures(t, c, taggedFn)[0])
	if fnDeclared.Level != ApiStabilityUnstable || fnDeclared.Declaration == nil {
		t.Fatalf("taggedFn signature declared = %+v, want unstable with provenance", fnDeclared)
	}
	fnUsed := session.UsedBySignature(apiStabilityTestSignatures(t, c, taggedFn)[0])
	if fnUsed.Incomplete || fnUsed.Minimum != ApiStabilityStable || len(fnUsed.Dependencies) != 0 {
		t.Fatalf("taggedFn signature own tag must be excluded, got min=%v deps=%v", fnUsed.Minimum, apiStabilityDependencyNames(fnUsed))
	}

	// The tagged root's children are still checked: the traversal never stops
	// at the root tag.
	withChild := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "TaggedRootWithChild"))
	if withChild.Incomplete || withChild.Minimum != ApiStabilityExperimental || !containsDependency(withChild, "E") {
		t.Fatalf("TaggedRootWithChild should expose E, got min=%v deps=%v", withChild.Minimum, apiStabilityDependencyNames(withChild))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, withChild, "TaggedRootWithChild")

	leaky := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "taggedLeaky"))
	if leaky.Incomplete || leaky.Minimum != ApiStabilityExperimental || !containsDependency(leaky, "E") {
		t.Fatalf("taggedLeaky should expose E, got min=%v deps=%v", leaky.Minimum, apiStabilityDependencyNames(leaky))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, leaky, "taggedLeaky")

	aliasLeaky := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "TaggedAliasLeaky"))
	if aliasLeaky.Incomplete || aliasLeaky.Minimum != ApiStabilityExperimental || !containsDependency(aliasLeaky, "E") {
		t.Fatalf("TaggedAliasLeaky should expose E, got min=%v deps=%v", aliasLeaky.Minimum, apiStabilityDependencyNames(aliasLeaky))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, aliasLeaky, "TaggedAliasLeaky")

	// A represented type keeps its own symbol's tag excluded while its children
	// still contribute.
	aliasType := session.UsedByType(c.GetDeclaredTypeOfSymbol(apiStabilityTestExport(t, c, testFile, "TaggedAliasLeaky")))
	if aliasType.Incomplete || aliasType.Minimum != ApiStabilityExperimental || !containsDependency(aliasType, "E") {
		t.Fatalf("TaggedAliasLeaky type should expose E, got min=%v deps=%v", aliasType.Minimum, apiStabilityDependencyNames(aliasType))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, aliasType, "TaggedAliasLeaky")
}

// TestApiStabilityRootExclusionIsAResultConversion proves
// the exclusion belongs to the result conversion and not to the stored
// surface: the stored surface always keeps its own tag, so the same concrete
// type reached as a root excludes it while the same type reached as a child of
// another root contributes it. The assertions run in the same session in both
// query orders.
func TestApiStabilityRootExclusionIsAResultConversion(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
export interface Shared { value: string }
export interface Holder { shared: Shared }
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	testFile := files["/.src/test.ts"]
	shared := apiStabilityTestExport(t, c, testFile, "Shared")
	holder := apiStabilityTestExport(t, c, testFile, "Holder")

	checkShared := func(t *testing.T, session *ApiStabilitySession, label string) {
		t.Helper()
		used := session.UsedBySymbol(shared)
		if used.Incomplete || used.Minimum != ApiStabilityStable || len(used.Dependencies) != 0 {
			t.Fatalf("%s: Shared as a root must exclude its own tag, got min=%v deps=%v", label, used.Minimum, apiStabilityDependencyNames(used))
		}
		surface, ok := session.analysis.sessionSymbols[shared]
		if !ok || len(surface.findings) == 0 {
			t.Fatalf("%s: the stored surface must keep the own tag for child reuse", label)
		}
	}

	checkHolder := func(t *testing.T, session *ApiStabilitySession, label string) {
		t.Helper()
		used := session.UsedBySymbol(holder)
		if used.Incomplete || used.Minimum != ApiStabilityExperimental || !containsDependency(used, "Shared") {
			t.Fatalf("%s: Holder should expose Shared as a child, got min=%v deps=%v", label, used.Minimum, apiStabilityDependencyNames(used))
		}
		surface, ok := session.analysis.sessionSymbols[holder]
		if !ok || len(surface.findings) == 0 {
			t.Fatalf("%s: Holder's stored surface must contain the child tag", label)
		}
	}

	// Root first, then child: the cached root surface still contributes when
	// reached as a child.
	childLast := tp.NewApiStabilitySession()
	checkShared(t, childLast, "shared-first")
	checkHolder(t, childLast, "shared-first")
	checkShared(t, childLast, "shared-first-repeat")

	// Child first, then root: the cached child does not leak into the root
	// result.
	rootLast := tp.NewApiStabilitySession()
	checkHolder(t, rootLast, "child-first")
	checkShared(t, rootLast, "child-first")
	checkHolder(t, rootLast, "child-first-repeat")
}

// TestApiStabilityAliasRootVersusConcreteIdentity pins the
// alias treatment: a tagged alias root excludes its own tag but the concrete
// underlying identity it resolves to stays an exposed child. A non-generic
// alias to an interface is represented by the compiler as the interface type
// itself, so the type-level query excludes the interface's own tag, exactly
// like querying the interface symbol directly.
func TestApiStabilityAliasRootVersusConcreteIdentity(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability unstable */
export interface Concrete { value: string }
export type AliasToConcrete = Concrete
/** @stability unstable */
export type TaggedAliasToConcrete = Concrete
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	testFile := files["/.src/test.ts"]
	session := tp.NewApiStabilitySession()

	concrete := apiStabilityTestExport(t, c, testFile, "Concrete")
	concreteUsed := session.UsedBySymbol(concrete)
	if concreteUsed.Incomplete || concreteUsed.Minimum != ApiStabilityStable || len(concreteUsed.Dependencies) != 0 {
		t.Fatalf("Concrete as a root must exclude its own tag, got min=%v deps=%v", concreteUsed.Minimum, apiStabilityDependencyNames(concreteUsed))
	}
	concreteType := session.UsedByType(c.GetDeclaredTypeOfSymbol(concrete))
	if concreteType.Incomplete || concreteType.Minimum != ApiStabilityStable || len(concreteType.Dependencies) != 0 {
		t.Fatalf("Concrete's type must exclude its own tag, got min=%v deps=%v", concreteType.Minimum, apiStabilityDependencyNames(concreteType))
	}

	// An untagged alias exposes the concrete identity as a child: the alias's
	// own tag is absent, so nothing is excluded beyond the alias itself.
	plain := apiStabilityTestExport(t, c, testFile, "AliasToConcrete")
	plainUsed := session.UsedBySymbol(plain)
	if plainUsed.Incomplete || plainUsed.Minimum != ApiStabilityUnstable || !containsDependency(plainUsed, "Concrete") {
		t.Fatalf("AliasToConcrete should expose Concrete as a child, got min=%v deps=%v", plainUsed.Minimum, apiStabilityDependencyNames(plainUsed))
	}

	// A tagged alias excludes its own tag and keeps the concrete identity as a
	// child.
	taggedAlias := apiStabilityTestExport(t, c, testFile, "TaggedAliasToConcrete")
	taggedAliasUsed := session.UsedBySymbol(taggedAlias)
	if taggedAliasUsed.Incomplete || taggedAliasUsed.Minimum != ApiStabilityUnstable || !containsDependency(taggedAliasUsed, "Concrete") {
		t.Fatalf("TaggedAliasToConcrete should exclude only its own tag and expose Concrete, got min=%v deps=%v", taggedAliasUsed.Minimum, apiStabilityDependencyNames(taggedAliasUsed))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, taggedAliasUsed, "TaggedAliasToConcrete")

	// The compiler represents `TaggedAliasToConcrete` as `Concrete`'s own type,
	// so the type-level query sees `Concrete` as the root identity and excludes
	// it. The alias symbol is not part of the type object; callers that want
	// the alias surface query the alias symbol, which is what the rule does.
	taggedAliasType := session.UsedByType(c.GetDeclaredTypeOfSymbol(taggedAlias))
	if taggedAliasType.Incomplete || taggedAliasType.Minimum != ApiStabilityStable || len(taggedAliasType.Dependencies) != 0 {
		t.Fatalf("TaggedAliasToConcrete's represented type excludes Concrete's own tag, got min=%v deps=%v", taggedAliasType.Minimum, apiStabilityDependencyNames(taggedAliasType))
	}
}

// TestApiStabilityOverloadTagsStayDistinct pins the
// overload treatment: the root symbol's own tag is exactly the declared tag the
// symbol-level declared accessor reports, so a tagged overload of that same
// declaration is excluded from the symbol surface. A tagged overload carrying
// another declaration keeps contributing as an exposed child, and each
// signature's own tag stays excluded from that signature's surface.
func TestApiStabilityOverloadTagsStayDistinct(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability unstable */
export function mixed(x: string): string
/** @stability experimental */
export function mixed(x: number): number
export function mixed(x: string | number): string | number { return "" }

export function other(x: string): string
/** @stability experimental */
export function other(x: number): number
export function other(x: string | number): string | number { return "" }
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	testFile := files["/.src/test.ts"]
	session := tp.NewApiStabilitySession()

	mixed := apiStabilityTestExport(t, c, testFile, "mixed")
	declared := tp.DeclaredApiStabilityOfSymbol(mixed)
	if declared.Level != ApiStabilityUnstable {
		t.Fatalf("mixed declared = %v, want unstable (the first tagged own declaration)", declared.Level)
	}
	signatures := apiStabilityTestSignatures(t, c, mixed)
	if len(signatures) != 2 {
		t.Fatalf("expected two public overloads, got %d", len(signatures))
	}
	if tp.DeclaredApiStabilityOfSignature(signatures[0]).Level != ApiStabilityUnstable ||
		tp.DeclaredApiStabilityOfSignature(signatures[1]).Level != ApiStabilityExperimental {
		t.Fatalf("overload tags collapsed: %v/%v",
			tp.DeclaredApiStabilityOfSignature(signatures[0]).Level,
			tp.DeclaredApiStabilityOfSignature(signatures[1]).Level)
	}

	// The unstable overload is the declared own tag and is excluded; the
	// experimental overload is another declaration and contributes as a child.
	mixedUsed := session.UsedBySymbol(mixed)
	if mixedUsed.Incomplete || mixedUsed.Minimum != ApiStabilityExperimental {
		t.Fatalf("mixed surface = %v/%v, want experimental from the other overload", mixedUsed.Minimum, apiStabilityDependencyNames(mixedUsed))
	}
	foundExperimental := false
	for _, dependency := range mixedUsed.Dependencies {
		if dependency.Signature != nil && dependency.Level == ApiStabilityExperimental {
			foundExperimental = true
		}
		if dependency.Signature != nil && rawSignature(dependency.Signature) == rawSignature(signatures[0]) {
			t.Fatalf("the declared own overload leaked into the surface: %v", apiStabilityDependencyNames(mixedUsed))
		}
	}
	if !foundExperimental {
		t.Fatalf("the experimental overload should stay an exposed child, got %v", apiStabilityDependencyNames(mixedUsed))
	}

	// Each overload's own tag is excluded from its own signature surface; the
	// stable parameter/return types expose nothing.
	for index, signature := range signatures {
		used := session.UsedBySignature(signature)
		if used.Incomplete || used.Minimum != ApiStabilityStable || len(used.Dependencies) != 0 {
			t.Fatalf("overload %d own tag must be excluded, got min=%v deps=%v", index, used.Minimum, apiStabilityDependencyNames(used))
		}
	}

	// When the symbol-level declared tag is the only tagged own declaration,
	// the whole symbol surface has no child findings.
	other := apiStabilityTestExport(t, c, testFile, "other")
	if declared := tp.DeclaredApiStabilityOfSymbol(other); declared.Level != ApiStabilityExperimental {
		t.Fatalf("other declared = %v, want experimental", declared.Level)
	}
	otherUsed := session.UsedBySymbol(other)
	if otherUsed.Incomplete || otherUsed.Minimum != ApiStabilityStable || len(otherUsed.Dependencies) != 0 {
		t.Fatalf("other's only tag is its own declared tag, got min=%v deps=%v", otherUsed.Minimum, apiStabilityDependencyNames(otherUsed))
	}
}

// TestApiStabilityExplicitStableBoundaries pins the child
// rules: an absent tag and an explicit stable tag are both effective stable at
// the export ceiling while staying distinct in the declared lookup, a named
// reference reached inside a surface stays shallow (no recursive member
// explosion), an explicit tag is a boundary for the component's internal
// surface, and independently exposed generic arguments are never hidden.
func TestApiStabilityExplicitStableBoundaries(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
export interface E { value: string }

/** @stability stable */
export interface StableBox<T> { value: T }
export interface PlainBox<T> { value: T }
export interface RootWithStableBox { box: StableBox<E> }
export interface RootWithPlainBox { box: PlainBox<E> }

/** @stability stable */
export interface StableHidden { value: E }
export interface PlainHidden { value: E }
export interface RootWithStableHidden { box: StableHidden }
export interface RootWithPlainHidden { box: PlainHidden }
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	testFile := files["/.src/test.ts"]
	session := tp.NewApiStabilitySession()

	// Explicit stable stays distinct from an absent tag in the declared lookup.
	stableBox := apiStabilityTestExport(t, c, testFile, "StableBox")
	stableDeclared := tp.DeclaredApiStabilityOfSymbol(stableBox)
	if stableDeclared.Level != ApiStabilityStable || stableDeclared.Declaration == nil {
		t.Fatalf("explicit stable child lost its provenance: %+v", stableDeclared)
	}
	plainBox := apiStabilityTestExport(t, c, testFile, "PlainBox")
	plainDeclared := tp.DeclaredApiStabilityOfSymbol(plainBox)
	if plainDeclared.Level != ApiStabilityStable || plainDeclared.Declaration != nil {
		t.Fatalf("absent tag should be stable without provenance: %+v", plainDeclared)
	}

	// A tagged stable child must not hide an independently exposed generic
	// argument.
	stableBoxRoot := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "RootWithStableBox"))
	if stableBoxRoot.Incomplete || stableBoxRoot.Minimum != ApiStabilityExperimental || !containsDependency(stableBoxRoot, "E") {
		t.Fatalf("RootWithStableBox should expose the StableBox type argument E, got min=%v deps=%v", stableBoxRoot.Minimum, apiStabilityDependencyNames(stableBoxRoot))
	}
	plainBoxRoot := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "RootWithPlainBox"))
	if plainBoxRoot.Incomplete || plainBoxRoot.Minimum != ApiStabilityExperimental || !containsDependency(plainBoxRoot, "E") {
		t.Fatalf("RootWithPlainBox should expose the PlainBox type argument E, got min=%v deps=%v", plainBoxRoot.Minimum, apiStabilityDependencyNames(plainBoxRoot))
	}

	// A directly exported component is expanded: its own members are part of
	// its root surface.
	stableHidden := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "StableHidden"))
	if stableHidden.Incomplete || stableHidden.Minimum != ApiStabilityExperimental || !containsDependency(stableHidden, "E") {
		t.Fatalf("StableHidden as a root should expand and expose E, got min=%v deps=%v", stableHidden.Minimum, apiStabilityDependencyNames(stableHidden))
	}

	// On the member route both the tagged and the untagged component stay
	// quiet: a plain member type already uses the shallow named-reference
	// boundary, so its own members are never expanded. The explicit tag is a
	// real boundary on the routes the shallow boundary would still inspect
	// (call/construct signatures and heritage); those differentials are pinned
	// in TestApiStabilityChildBoundaryDifferentials.
	stableHiddenRoot := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "RootWithStableHidden"))
	if stableHiddenRoot.Incomplete || len(stableHiddenRoot.Dependencies) != 0 {
		t.Fatalf("RootWithStableHidden must not expand the nested component, got min=%v deps=%v", stableHiddenRoot.Minimum, apiStabilityDependencyNames(stableHiddenRoot))
	}
	plainHiddenRoot := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "RootWithPlainHidden"))
	if plainHiddenRoot.Incomplete || len(plainHiddenRoot.Dependencies) != 0 {
		t.Fatalf("RootWithPlainHidden must not expand the nested component, got min=%v deps=%v", plainHiddenRoot.Minimum, apiStabilityDependencyNames(plainHiddenRoot))
	}
}

// TestApiStabilityChildrenStillChecked pins that the
// root-excluding split keeps the currently represented obligations checked:
// generic parameters, constraints, inferred anonymous nested returns and
// signature components still expose their dependencies under a tagged root.
func TestApiStabilityChildrenStillChecked(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
declare function make<T>(): { nested: { fn: (x: T) => T } }

/** @stability unstable */
export const taggedInferred = make<E>()
export const cleanInferred = make<string>()

/** @stability unstable */
export declare function taggedGeneric<T extends E>(x: T): T

/** @stability unstable */
export interface TaggedConstrained<T extends E> { value: T }
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	testFile := files["/.src/test.ts"]
	session := tp.NewApiStabilitySession()

	inferred := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "taggedInferred"))
	if inferred.Incomplete || inferred.Minimum != ApiStabilityExperimental || !containsDependency(inferred, "E") {
		t.Fatalf("an inferred nested anonymous return should still expose E, got min=%v deps=%v", inferred.Minimum, apiStabilityDependencyNames(inferred))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, inferred, "taggedInferred")

	clean := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "cleanInferred"))
	if clean.Incomplete || len(clean.Dependencies) != 0 {
		t.Fatalf("cleanInferred must stay clean, got min=%v deps=%v", clean.Minimum, apiStabilityDependencyNames(clean))
	}

	generic := apiStabilityTestExport(t, c, testFile, "taggedGeneric")
	genericUsed := session.UsedBySymbol(generic)
	if genericUsed.Incomplete || genericUsed.Minimum != ApiStabilityExperimental || !containsDependency(genericUsed, "E") {
		t.Fatalf("a tagged generic function should still expose its constraint, got min=%v deps=%v", genericUsed.Minimum, apiStabilityDependencyNames(genericUsed))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, genericUsed, "taggedGeneric")
	signatureUsed := session.UsedBySignature(apiStabilityTestSignatures(t, c, generic)[0])
	if signatureUsed.Incomplete || signatureUsed.Minimum != ApiStabilityExperimental || !containsDependency(signatureUsed, "E") {
		t.Fatalf("a tagged signature should still expose its constraint, got min=%v deps=%v", signatureUsed.Minimum, apiStabilityDependencyNames(signatureUsed))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, signatureUsed, "taggedGeneric")

	constrained := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "TaggedConstrained"))
	if constrained.Incomplete || constrained.Minimum != ApiStabilityExperimental || !containsDependency(constrained, "E") {
		t.Fatalf("a tagged generic interface should still expose its constraint, got min=%v deps=%v", constrained.Minimum, apiStabilityDependencyNames(constrained))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, constrained, "TaggedConstrained")
}

// TestApiStabilityIncompleteAndCyclesKeepExclusion pins
// that an incomplete or cycle-settled result still excludes the root's own tag
// and keeps the generality of the result: incomplete is never treated as
// stable, and a cycle still propagates its leak without reintroducing the root
// tag.
func TestApiStabilityIncompleteAndCyclesKeepExclusion(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
export interface E { value: string }

/** @stability unstable */
export interface Node { next: Node; payload: E }

/** @stability unstable */
export interface Cold { plain: number; [Symbol.iterator](): E }
`,
	})
	defer done()

	testFile := files["/.src/test.ts"]

	// Cold: the computed member is not materialized, so the surface reports
	// incomplete. The root's own tag is excluded even from the incomplete
	// result.
	cold := apiStabilityTestExport(t, c, testFile, "Cold")
	coldUsed := tp.ApiStabilityUsedBySymbol(cold)
	if !coldUsed.Incomplete {
		t.Fatal("an unmaterialized computed member must report an incomplete result")
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, coldUsed, "Cold")

	primeApiStabilityChecker(t, c)

	warmCold := NewTypeParser(c.Program(), c).ApiStabilityUsedBySymbol(cold)
	if warmCold.Incomplete || !containsDependency(warmCold, "E") {
		t.Fatalf("the materialized retry should settle complete with E, got min=%v deps=%v", warmCold.Minimum, apiStabilityDependencyNames(warmCold))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, warmCold, "Cold")

	session := tp.NewApiStabilitySession()
	node := apiStabilityTestExport(t, c, testFile, "Node")
	nodeUsed := session.UsedBySymbol(node)
	if nodeUsed.Incomplete || nodeUsed.Minimum != ApiStabilityExperimental || !containsDependency(nodeUsed, "E") {
		t.Fatalf("the cyclic root should expose E, got min=%v deps=%v", nodeUsed.Minimum, apiStabilityDependencyNames(nodeUsed))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, nodeUsed, "Node")
	if surface := session.analysis.sessionSymbols[node]; !surface.complete || surface.blocked {
		t.Fatal("a settled cyclic surface should be a complete session result")
	}
	// The stored cyclic surface keeps the own tag for child reuse.
	if len(session.analysis.sessionSymbols[node].findings) == 0 {
		t.Fatal("the stored cyclic surface must keep the own tag")
	}
}

// TestApiStabilityDeclaredReadsStayIndependent pins that
// the split keeps declared and computed state independent for every new root
// accessor: reading a declared level never populates a session or triggers a
// surface, and a surface query reuses the same declared cache.
func TestApiStabilityDeclaredReadsStayIndependent(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability unstable */
export function taggedFn(x: string): string
/** @stability unstable */
export interface TaggedIface { value: string }
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	testFile := files["/.src/test.ts"]
	taggedFn := apiStabilityTestExport(t, c, testFile, "taggedFn")
	taggedIface := apiStabilityTestExport(t, c, testFile, "TaggedIface")
	signature := apiStabilityTestSignatures(t, c, taggedFn)[0]

	before := c.TotalInstantiationCount
	_ = tp.DeclaredApiStabilityOfSymbol(taggedFn)
	_ = tp.DeclaredApiStabilityOfSymbol(taggedIface)
	_ = tp.DeclaredApiStabilityOfSignature(signature)
	if delta := c.TotalInstantiationCount - before; delta != 0 {
		t.Fatalf("declared reads instantiated %d types, want 0", delta)
	}

	session := tp.NewApiStabilitySession()
	if len(session.analysis.sessionSymbols) != 0 || len(session.analysis.sessionTypes) != 0 ||
		len(session.analysis.sessionSignatures) != 0 || len(session.analysis.sessionComponents) != 0 {
		t.Fatal("declared reads left computed analysis state")
	}
	if !tp.links.ApiStabilityDeclaredSymbol.Has(taggedFn) || !tp.links.ApiStabilityDeclaredSymbol.Has(taggedIface) {
		t.Fatal("declared symbol lookups should be cached")
	}
	if !tp.links.ApiStabilityDeclaredSignature.Has(rawSignature(signature)) {
		t.Fatal("declared signature lookup should be cached")
	}

	// The surface query builds its root identity from the declared cache and
	// still returns the root-excluding result.
	used := session.UsedBySymbol(taggedFn)
	if used.Incomplete || used.Minimum != ApiStabilityStable || len(used.Dependencies) != 0 {
		t.Fatalf("taggedFn surface should be empty, got min=%v deps=%v", used.Minimum, apiStabilityDependencyNames(used))
	}
}

// TestApiStabilityChildBoundaryDifferentials pins the
// explicitly tagged child boundary on every surface edge: a child carrying an
// explicit declared tag contributes its declared level and its internals are
// not expanded, while an untagged child keeps composing within the current
// traversal boundaries. Independently represented generic arguments stay
// exposed on tagged children, a tagged component still expands when it is the
// queried root, and a member the compiler flattened into a derived member table
// from a tagged base is attributed to that base rather than expanded as the
// derived component's own member.
func TestApiStabilityChildBoundaryDifferentials(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
export interface E { value: string }

/** @stability stable */
export interface TaggedCallable { (): E }
export interface PlainCallable { (): E }
export interface CallableHolder { tagged: TaggedCallable; plain: PlainCallable }

/** @stability unstable */
export interface UnstableCallable { (): E }
export interface UnstableCallableHolder { callable: UnstableCallable }

export interface AnonymousWrapper { inner: { callable: TaggedCallable } }

/** @stability stable */
export interface TaggedHidden { secret: E }
export interface PlainHidden { secret: E }
export interface HiddenHolder { tagged: TaggedHidden; plain: PlainHidden }

/** @stability stable */
export interface TaggedBase { secret: E }
export interface PlainBase { secret: E }
export interface TaggedHeritage extends TaggedBase {}
export interface PlainHeritage extends PlainBase {}

/** @stability unstable */
export interface UnstableBase { secret: E }
export interface UnstableHeritage extends UnstableBase {}

/** @stability stable */
export interface TaggedBox<T> { value: T; extra: E }
export interface TaggedBoxStableArgument extends TaggedBox<string> {}
export interface TaggedBoxExposedArgument extends TaggedBox<E> {}

/** @stability stable */
export interface TaggedChain<T> { value: T }
export interface UntaggedMid<U> extends TaggedChain<U> {}
export interface DeepChain extends UntaggedMid<E> {}
export interface DeepChainRef<T> extends UntaggedMid<T> {}
export declare const deepChainRef: DeepChainRef<E>

/** @stability stable */
export type TaggedAliasBox<T> = { value: T }
export interface TaggedAliasBoxHolder { box: TaggedAliasBox<E> }

export interface MemberTagBoundary {
  /** @stability stable */
  anonymous: { inner: E }
  /** @stability stable */
  (): E
}
export interface MemberTagControl {
  anonymous: { inner: E }
  (): E
}

export interface IndexTagBoundary {
  /** @stability stable */
  [key: string]: E
}
export interface IndexTagControl {
  [key: string]: E
}

/** @stability unstable */
export interface SharedBoundary { secret: E }
export interface SharedBoundaryHolder { child: SharedBoundary }

export interface HeritageOverride extends TaggedBase {
  secret: E
}

/** @stability stable */
export interface TaggedIndexedBox<T> { value: T }
export interface BlockedHeritage<T, K extends keyof T> extends TaggedIndexedBox<T[K]> {}
export interface BlockedMember<T, K extends keyof T> { box: TaggedIndexedBox<T[K]> }
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	testFile := files["/.src/test.ts"]
	session := tp.NewApiStabilitySession()
	surface := func(name string) ApiStabilityUsed {
		t.Helper()
		return session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, name))
	}

	// Member route: the untagged callable's directly exposed call signature is
	// inspected, while the explicitly tagged callable stops before it.
	callableHolder := surface("CallableHolder")
	if callableHolder.Incomplete || callableHolder.Minimum != ApiStabilityExperimental || !containsDependency(callableHolder, "E") {
		t.Fatalf("CallableHolder should expose E through its untagged callable, got min=%v deps=%v", callableHolder.Minimum, apiStabilityDependencyNames(callableHolder))
	}

	anonymousWrapper := surface("AnonymousWrapper")
	if anonymousWrapper.Incomplete || containsDependency(anonymousWrapper, "E") {
		t.Fatalf("a tagged callable must stay bounded through an anonymous wrapper, got min=%v deps=%v", anonymousWrapper.Minimum, apiStabilityDependencyNames(anonymousWrapper))
	}

	hiddenHolder := surface("HiddenHolder")
	if hiddenHolder.Incomplete || len(hiddenHolder.Dependencies) != 0 {
		t.Fatalf("a tagged hidden member and an untagged shallow member must both stay quiet, got min=%v deps=%v", hiddenHolder.Minimum, apiStabilityDependencyNames(hiddenHolder))
	}

	// The tagged hidden component itself still expands as a queried root.
	taggedHidden := surface("TaggedHidden")
	if taggedHidden.Incomplete || taggedHidden.Minimum != ApiStabilityExperimental || !containsDependency(taggedHidden, "E") {
		t.Fatalf("TaggedHidden as a root should expand and expose E, got min=%v deps=%v", taggedHidden.Minimum, apiStabilityDependencyNames(taggedHidden))
	}

	// Severity: an unstable tagged child contributes unstable, never its
	// experimental internal.
	unstableCallable := surface("UnstableCallableHolder")
	if unstableCallable.Incomplete || unstableCallable.Minimum != ApiStabilityUnstable || containsDependency(unstableCallable, "E") {
		t.Fatalf("UnstableCallableHolder should expose only the unstable child tag, got min=%v deps=%v", unstableCallable.Minimum, apiStabilityDependencyNames(unstableCallable))
	}
	if !containsDependency(unstableCallable, "UnstableCallable") {
		t.Fatalf("UnstableCallableHolder should expose UnstableCallable, got %v", apiStabilityDependencyNames(unstableCallable))
	}
	unstableCallableRoot := surface("UnstableCallable")
	if unstableCallableRoot.Incomplete || unstableCallableRoot.Minimum != ApiStabilityExperimental || !containsDependency(unstableCallableRoot, "E") {
		t.Fatalf("UnstableCallable as a root should expand and expose E, got min=%v deps=%v", unstableCallableRoot.Minimum, apiStabilityDependencyNames(unstableCallableRoot))
	}

	// Heritage route: a tagged base stops the expand; an untagged base keeps it.
	taggedHeritage := surface("TaggedHeritage")
	if taggedHeritage.Incomplete || len(taggedHeritage.Dependencies) != 0 {
		t.Fatalf("TaggedHeritage must not expose the tagged base internals, got min=%v deps=%v", taggedHeritage.Minimum, apiStabilityDependencyNames(taggedHeritage))
	}
	plainHeritage := surface("PlainHeritage")
	if plainHeritage.Incomplete || plainHeritage.Minimum != ApiStabilityExperimental || !containsDependency(plainHeritage, "E") {
		t.Fatalf("PlainHeritage should expand its untagged base and expose E, got min=%v deps=%v", plainHeritage.Minimum, apiStabilityDependencyNames(plainHeritage))
	}
	unstableHeritage := surface("UnstableHeritage")
	if unstableHeritage.Incomplete || unstableHeritage.Minimum != ApiStabilityUnstable || containsDependency(unstableHeritage, "E") {
		t.Fatalf("UnstableHeritage should expose only the unstable base tag, got min=%v deps=%v", unstableHeritage.Minimum, apiStabilityDependencyNames(unstableHeritage))
	}
	if !containsDependency(unstableHeritage, "UnstableBase") {
		t.Fatalf("UnstableHeritage should expose UnstableBase, got %v", apiStabilityDependencyNames(unstableHeritage))
	}

	// Heritage generic arguments: a stable argument contributes nothing, an
	// independently represented argument stays exposed.
	stableArgument := surface("TaggedBoxStableArgument")
	if stableArgument.Incomplete || len(stableArgument.Dependencies) != 0 {
		t.Fatalf("TaggedBox<string> must not expose the tagged base internals, got min=%v deps=%v", stableArgument.Minimum, apiStabilityDependencyNames(stableArgument))
	}
	exposedArgument := surface("TaggedBoxExposedArgument")
	if exposedArgument.Incomplete || exposedArgument.Minimum != ApiStabilityExperimental || !containsDependency(exposedArgument, "E") {
		t.Fatalf("TaggedBox<E> must keep the independently represented argument exposed, got min=%v deps=%v", exposedArgument.Minimum, apiStabilityDependencyNames(exposedArgument))
	}

	// Deep heritage: the untagged intermediate keeps composing, so the tagged
	// ancestor's argument is still reached with the composed substitution.
	deepChain := surface("DeepChain")
	if deepChain.Incomplete || deepChain.Minimum != ApiStabilityExperimental || !containsDependency(deepChain, "E") {
		t.Fatalf("DeepChain should expose the tagged ancestor argument through the untagged intermediate, got min=%v deps=%v", deepChain.Minimum, apiStabilityDependencyNames(deepChain))
	}

	// The same deep chain as an expanded reference root: the flattened member
	// table attributes the tagged ancestor and still exposes its argument.
	deepRefIdent := findIdentifierByText(t, testFile, "DeepChainRef", 1)
	if deepRefIdent == nil || deepRefIdent.Parent == nil {
		t.Fatal("DeepChainRef annotation not found")
	}
	deepRefType := tp.GetTypeAtLocation(deepRefIdent.Parent)
	deepRef := session.UsedByType(deepRefType)
	if deepRef.Incomplete || deepRef.Minimum != ApiStabilityExperimental || !containsDependency(deepRef, "E") {
		t.Fatalf("the DeepChainRef<E> reference root should expose E, got min=%v deps=%v", deepRef.Minimum, apiStabilityDependencyNames(deepRef))
	}

	// Alias application: the alias tag is the boundary and its argument stays.
	aliasBox := surface("TaggedAliasBoxHolder")
	if aliasBox.Incomplete || aliasBox.Minimum != ApiStabilityExperimental || !containsDependency(aliasBox, "E") {
		t.Fatalf("a tagged alias application must keep its argument exposed, got min=%v deps=%v", aliasBox.Minimum, apiStabilityDependencyNames(aliasBox))
	}

	// Member tag: an explicitly tagged member bounds its anonymous type, a tag
	// on an anonymous member's own call signature bounds that signature, and an
	// untagged control keeps the anonymous walk.
	memberTagBoundary := surface("MemberTagBoundary")
	if memberTagBoundary.Incomplete || len(memberTagBoundary.Dependencies) != 0 {
		t.Fatalf("tagged member and tagged call signature must both stay bounded, got min=%v deps=%v", memberTagBoundary.Minimum, apiStabilityDependencyNames(memberTagBoundary))
	}
	memberTagControl := surface("MemberTagControl")
	if memberTagControl.Incomplete || memberTagControl.Minimum != ApiStabilityExperimental || !containsDependency(memberTagControl, "E") {
		t.Fatalf("MemberTagControl should keep the anonymous walk and expose E, got min=%v deps=%v", memberTagControl.Minimum, apiStabilityDependencyNames(memberTagControl))
	}

	// Index signature tag: the explicit tag bounds the key and value internals.
	indexTagBoundary := surface("IndexTagBoundary")
	if indexTagBoundary.Incomplete || containsDependency(indexTagBoundary, "E") {
		t.Fatalf("a tagged index signature must not expose its value internals, got min=%v deps=%v", indexTagBoundary.Minimum, apiStabilityDependencyNames(indexTagBoundary))
	}
	indexTagControl := surface("IndexTagControl")
	if indexTagControl.Incomplete || indexTagControl.Minimum != ApiStabilityExperimental || !containsDependency(indexTagControl, "E") {
		t.Fatalf("IndexTagControl should expose the index value type, got min=%v deps=%v", indexTagControl.Minimum, apiStabilityDependencyNames(indexTagControl))
	}

	// The same tagged component as parent-child and as root: the parent sees
	// the child's declared level, the child's own root audit still expands.
	sharedHolder := surface("SharedBoundaryHolder")
	if sharedHolder.Incomplete || sharedHolder.Minimum != ApiStabilityUnstable || containsDependency(sharedHolder, "E") {
		t.Fatalf("SharedBoundaryHolder should expose only the child tag, got min=%v deps=%v", sharedHolder.Minimum, apiStabilityDependencyNames(sharedHolder))
	}
	if !containsDependency(sharedHolder, "SharedBoundary") {
		t.Fatalf("SharedBoundaryHolder should expose SharedBoundary, got %v", apiStabilityDependencyNames(sharedHolder))
	}
	sharedRoot := surface("SharedBoundary")
	if sharedRoot.Incomplete || sharedRoot.Minimum != ApiStabilityExperimental || !containsDependency(sharedRoot, "E") {
		t.Fatalf("SharedBoundary as a root should still expand and expose E, got min=%v deps=%v", sharedRoot.Minimum, apiStabilityDependencyNames(sharedRoot))
	}

	// A same-named own override on an untagged derived declaration is the
	// derived component's own member and keeps being inspected even though the
	// base is tagged.
	heritageOverride := surface("HeritageOverride")
	if heritageOverride.Incomplete || heritageOverride.Minimum != ApiStabilityExperimental || !containsDependency(heritageOverride, "E") {
		t.Fatalf("an own override must not be suppressed by a tagged base of the same name, got min=%v deps=%v", heritageOverride.Minimum, apiStabilityDependencyNames(heritageOverride))
	}

	// A blocked required argument keeps the parent incomplete: the boundary
	// contributes its tag plus exposed arguments, and an argument that cannot be
	// established is never dropped to make the parent complete.
	blockedHeritage := surface("BlockedHeritage")
	if !blockedHeritage.Incomplete {
		t.Fatalf("a tagged base whose required argument cannot be established must stay incomplete, got min=%v deps=%v", blockedHeritage.Minimum, apiStabilityDependencyNames(blockedHeritage))
	}
	blockedMember := surface("BlockedMember")
	if !blockedMember.Incomplete {
		t.Fatalf("a tagged member whose required argument cannot be established must stay incomplete, got min=%v deps=%v", blockedMember.Minimum, apiStabilityDependencyNames(blockedMember))
	}
}

// TestApiStabilityFlattenedInheritanceBoundaries pins the
// fix2 correction: the compiler flattens inherited call, construct and index
// signatures of a derived structured type while keeping each declaration's
// base provenance, so an explicitly tagged base bounds those internals exactly
// like it bounds flattened members. Every affected resolved read is covered:
// the declaration root, the instantiated reference root, the exported value of
// the derived generic type, the class static side (including the inherited
// constructor and an own constructor override), an untagged intermediate chain
// ending in a tagged callable ancestor, and the anonymous/function signature
// reads. Owner-declared signatures and index signatures stay walked, the tagged
// base keeps reporting its own internals as a root, independently represented
// arguments of a tagged ancestor stay exposed, and untagged controls keep
// composing. Cold results must never report the bounded internals either.
func TestApiStabilityFlattenedInheritanceBoundaries(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
export interface E { value: string }
/** @stability experimental */
export interface FxCallE { value: string }
/** @stability experimental */
export interface FxCtorE { value: string }
/** @stability experimental */
export interface FxIdxE { value: string }
/** @stability experimental */
export interface FxMethE { value: string }
/** @stability experimental */
export interface FxPropE { value: string }
/** @stability experimental */
export interface FxArgE { value: string }

/** @stability unstable */
export interface FxTaggedCallableBase { (): FxCallE }
export interface FxDerivedCallable extends FxTaggedCallableBase {}
export interface FxDerivedCallableGeneric<T> extends FxTaggedCallableBase { own: T }
export declare const fxCallRef: FxDerivedCallableGeneric<string>
interface FxPlainCallableBase { (): FxCallE }
export interface FxPlainDerivedCallableGeneric<T> extends FxPlainCallableBase { own: T }
export declare const fxPlainCallRef: FxPlainDerivedCallableGeneric<string>

/** @stability unstable */
export interface FxTaggedCombo {
  (): FxCallE
  new (): FxCtorE
  [key: string]: FxIdxE
  m(): FxMethE
  p: FxPropE
}
export interface FxDerivedCombo<T> extends FxTaggedCombo { own: T }
export declare const fxComboRef: FxDerivedCombo<string>

/** @stability unstable */
export interface FxTaggedIndexBase { [key: string]: FxIdxE }
export interface FxDerivedIndex<T> extends FxTaggedIndexBase { own: T }
export declare const fxIndexRef: FxDerivedIndex<string>
interface FxPlainIndexBase { [key: string]: FxIdxE }
export interface FxPlainDerivedIndex<T> extends FxPlainIndexBase { own: T }
export declare const fxPlainIndexRef: FxPlainDerivedIndex<string>

/** @stability unstable */
export interface FxTaggedDeepCallable { (): FxCallE }
export interface FxMidCallable<T> extends FxTaggedDeepCallable { mid: T }
export interface FxDeepCallable<T> extends FxMidCallable<T> {}
export declare const fxDeepCallRef: FxDeepCallable<string>

/** @stability unstable */
export class FxTaggedClassBase {
  constructor(x: FxCtorE)
  static s: FxPropE
  m(): FxMethE
}
export class FxDerivedClass extends FxTaggedClassBase {}
export class FxDerivedOwnCtorClass extends FxTaggedClassBase {
  constructor(y: E) { super(y as unknown as FxCtorE) }
}
/** @stability unstable */
export class FxTaggedNoCtorClass { m(): FxMethE }
export class FxDerivedNoCtorClass extends FxTaggedNoCtorClass {}
class FxPlainClassBase {
  constructor(x: FxCtorE)
  static s: FxPropE
  m(): FxMethE
}
export class FxPlainDerivedClass extends FxPlainClassBase {}

/** @stability stable */
export interface FxStableCallableBase { (): FxCallE }
export interface FxDerivedStableCallable extends FxStableCallableBase {}

/** @stability unstable */
export interface FxOwnOverrideBase { (): FxCallE; [key: string]: FxIdxE }
export interface FxOwnOverrideDerived extends FxOwnOverrideBase {
  (): E
  [key: string]: E
}

/** @stability unstable */
export interface FxTaggedCallableBox<T> { (): FxPropE; value: T }
export interface FxDerivedCallableBox<T> extends FxTaggedCallableBox<T> {}
export declare const fxCallableBoxRef: FxDerivedCallableBox<FxArgE>

/** @stability unstable */
export interface FxTaggedFirstCall { (): FxCallE }
interface FxPlainSecond { own: string }
export interface FxMixedCallInheritance extends FxTaggedFirstCall, FxPlainSecond {}

/** @stability unstable */
export interface FxTaggedBlockedBox<T> { value: T }
export interface FxBlockedDerived<T, K extends keyof T> extends FxTaggedBlockedBox<T[K]> {}
`,
	})
	defer done()

	testFile := files["/.src/test.ts"]
	assertQuiet := func(label string, used ApiStabilityUsed, dependency string) {
		t.Helper()
		if containsDependency(used, dependency) {
			t.Fatalf("%s must not expose %s, got incomplete=%v min=%v deps=%v", label, dependency, used.Incomplete, used.Minimum, apiStabilityDependencyNames(used))
		}
	}
	assertExposed := func(label string, used ApiStabilityUsed, dependency string) {
		t.Helper()
		if used.Incomplete || !containsDependency(used, dependency) {
			t.Fatalf("%s should expose %s, got incomplete=%v min=%v deps=%v", label, dependency, used.Incomplete, used.Minimum, apiStabilityDependencyNames(used))
		}
	}

	// Cold: the flattened signatures and index infos are not materialized yet.
	// Every route must stay quiet about the tagged base internals, with no
	// false positive of any kind.
	cold := tp.NewApiStabilitySession()
	coldCallRefType := c.GetTypeOfSymbol(apiStabilityTestExport(t, c, testFile, "fxCallRef"))
	coldIndexRefType := c.GetTypeOfSymbol(apiStabilityTestExport(t, c, testFile, "fxIndexRef"))
	coldComboRefType := c.GetTypeOfSymbol(apiStabilityTestExport(t, c, testFile, "fxComboRef"))
	coldDeepRefType := c.GetTypeOfSymbol(apiStabilityTestExport(t, c, testFile, "fxDeepCallRef"))
	coldDerivedClassStatic := c.GetTypeOfSymbol(apiStabilityTestExport(t, c, testFile, "FxDerivedClass"))
	if coldCallRefType == nil || coldIndexRefType == nil || coldComboRefType == nil || coldDeepRefType == nil || coldDerivedClassStatic == nil {
		t.Fatal("cold reference types must be representable")
	}
	for _, test := range []struct {
		label string
		used  ApiStabilityUsed
	}{
		{"cold FxDerivedCallable declaration root", cold.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "FxDerivedCallable"))},
		{"cold FxDerivedCallableGeneric declaration root", cold.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "FxDerivedCallableGeneric"))},
		{"cold FxDerivedIndex declaration root", cold.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "FxDerivedIndex"))},
		{"cold FxDerivedClass declaration root", cold.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "FxDerivedClass"))},
		{"cold fxCallRef reference root", cold.UsedByType(coldCallRefType)},
		{"cold fxIndexRef reference root", cold.UsedByType(coldIndexRefType)},
		{"cold fxComboRef reference root", cold.UsedByType(coldComboRefType)},
		{"cold fxDeepCallRef reference root", cold.UsedByType(coldDeepRefType)},
		{"cold fxDerivedClass static type root", cold.UsedByType(coldDerivedClassStatic)},
	} {
		for _, dependency := range []string{"FxCallE", "FxCtorE", "FxIdxE", "FxMethE", "FxPropE"} {
			assertQuiet(test.label, test.used, dependency)
		}
	}

	primeApiStabilityChecker(t, c)
	warm := tp.NewApiStabilitySession()
	surface := func(name string) ApiStabilityUsed {
		t.Helper()
		return warm.UsedBySymbol(apiStabilityTestExport(t, c, testFile, name))
	}
	referenceSurface := func(name string) ApiStabilityUsed {
		t.Helper()
		represented := c.GetTypeOfSymbol(apiStabilityTestExport(t, c, testFile, name))
		if represented == nil {
			t.Fatalf("%s has no represented type", name)
		}
		return warm.UsedByType(represented)
	}

	// Inherited call signature: the derived declaration and the instantiated
	// reference (type and exported value roots) expose only the tagged base,
	// never the base's call signature internals. The untagged control composes.
	derivedCallable := surface("FxDerivedCallable")
	if derivedCallable.Incomplete || derivedCallable.Minimum != ApiStabilityUnstable || containsDependency(derivedCallable, "FxCallE") {
		t.Fatalf("FxDerivedCallable must expose only the tagged base, got min=%v deps=%v", derivedCallable.Minimum, apiStabilityDependencyNames(derivedCallable))
	}
	if !containsDependency(derivedCallable, "FxTaggedCallableBase") {
		t.Fatalf("FxDerivedCallable should expose FxTaggedCallableBase, got %v", apiStabilityDependencyNames(derivedCallable))
	}
	derivedCallableGeneric := surface("FxDerivedCallableGeneric")
	if derivedCallableGeneric.Incomplete || derivedCallableGeneric.Minimum != ApiStabilityUnstable || containsDependency(derivedCallableGeneric, "FxCallE") {
		t.Fatalf("FxDerivedCallableGeneric must expose only the tagged base, got min=%v deps=%v", derivedCallableGeneric.Minimum, apiStabilityDependencyNames(derivedCallableGeneric))
	}
	callRefType := referenceSurface("fxCallRef")
	if callRefType.Incomplete || callRefType.Minimum != ApiStabilityUnstable || containsDependency(callRefType, "FxCallE") {
		t.Fatalf("fxCallRef type root must expose only the tagged base, got min=%v deps=%v", callRefType.Minimum, apiStabilityDependencyNames(callRefType))
	}
	if !containsDependency(callRefType, "FxTaggedCallableBase") {
		t.Fatalf("fxCallRef type root should expose FxTaggedCallableBase, got %v", apiStabilityDependencyNames(callRefType))
	}
	callRefSymbol := surface("fxCallRef")
	if callRefSymbol.Incomplete || callRefSymbol.Minimum != ApiStabilityUnstable || containsDependency(callRefSymbol, "FxCallE") {
		t.Fatalf("fxCallRef value root must expose only the tagged base, got min=%v deps=%v", callRefSymbol.Minimum, apiStabilityDependencyNames(callRefSymbol))
	}
	assertExposed("fxPlainCallRef untagged control", referenceSurface("fxPlainCallRef"), "FxCallE")

	// Combined flattened signatures: call, construct and index infos are all
	// bounded while the already-gated members stay bounded too.
	for _, label := range []string{"combo type root", "combo value root"} {
		var used ApiStabilityUsed
		if label == "combo type root" {
			used = referenceSurface("fxComboRef")
		} else {
			used = surface("fxComboRef")
		}
		if used.Incomplete || used.Minimum != ApiStabilityUnstable || !containsDependency(used, "FxTaggedCombo") {
			t.Fatalf("%s must expose only FxTaggedCombo, got min=%v deps=%v", label, used.Minimum, apiStabilityDependencyNames(used))
		}
		for _, dependency := range []string{"FxCallE", "FxCtorE", "FxIdxE", "FxMethE", "FxPropE"} {
			assertQuiet(label, used, dependency)
		}
	}

	// Inherited index signature: declaration and reference roots stay quiet;
	// the untagged control still exposes the value type.
	derivedIndex := surface("FxDerivedIndex")
	if derivedIndex.Incomplete || derivedIndex.Minimum != ApiStabilityUnstable || containsDependency(derivedIndex, "FxIdxE") {
		t.Fatalf("FxDerivedIndex must expose only the tagged base, got min=%v deps=%v", derivedIndex.Minimum, apiStabilityDependencyNames(derivedIndex))
	}
	indexRefType := referenceSurface("fxIndexRef")
	if indexRefType.Incomplete || indexRefType.Minimum != ApiStabilityUnstable || containsDependency(indexRefType, "FxIdxE") {
		t.Fatalf("fxIndexRef type root must expose only the tagged base, got min=%v deps=%v", indexRefType.Minimum, apiStabilityDependencyNames(indexRefType))
	}
	assertQuiet("fxIndexRef value root", surface("fxIndexRef"), "FxIdxE")
	assertExposed("fxPlainIndexRef untagged control", referenceSurface("fxPlainIndexRef"), "FxIdxE")

	// Deep chain: an untagged intermediate still composes up to the tagged
	// callable ancestor, which bounds its flattened call signature.
	deepCallRef := referenceSurface("fxDeepCallRef")
	if deepCallRef.Incomplete || containsDependency(deepCallRef, "FxCallE") {
		t.Fatalf("fxDeepCallRef must not expose the tagged ancestor's call internals, got min=%v deps=%v", deepCallRef.Minimum, apiStabilityDependencyNames(deepCallRef))
	}
	if !containsDependency(deepCallRef, "FxTaggedDeepCallable") {
		t.Fatalf("fxDeepCallRef should expose FxTaggedDeepCallable, got %v", apiStabilityDependencyNames(deepCallRef))
	}

	// Class static side: the inherited constructor parameter is the tagged
	// base's internal; the inherited method and static are bounded as members.
	derivedClass := surface("FxDerivedClass")
	if derivedClass.Incomplete || derivedClass.Minimum != ApiStabilityUnstable {
		t.Fatalf("FxDerivedClass must expose the tagged base level, got min=%v deps=%v", derivedClass.Minimum, apiStabilityDependencyNames(derivedClass))
	}
	for _, dependency := range []string{"FxCtorE", "FxMethE", "FxPropE"} {
		assertQuiet("FxDerivedClass", derivedClass, dependency)
	}
	derivedClassStatic := warm.UsedByType(c.GetTypeOfSymbol(apiStabilityTestExport(t, c, testFile, "FxDerivedClass")))
	assertQuiet("FxDerivedClass static type root", derivedClassStatic, "FxCtorE")
	assertExposed("FxPlainDerivedClass untagged control", surface("FxPlainDerivedClass"), "FxCtorE")

	// An own constructor override is the derived component's own declaration:
	// its parameter stays exposed and the tagged base's constructor parameter
	// is never surfaced alongside it.
	ownCtor := surface("FxDerivedOwnCtorClass")
	assertExposed("FxDerivedOwnCtorClass own constructor", ownCtor, "E")
	assertQuiet("FxDerivedOwnCtorClass", ownCtor, "FxCtorE")

	// A tagged class without a constructor has no flattened constructor
	// declaration to attribute (the checker synthesizes a parameterless default
	// constructor). Nothing must leak, no base resolution may be forced, and
	// the derived component still records the tagged base's level.
	derivedNoCtor := surface("FxDerivedNoCtorClass")
	if derivedNoCtor.Incomplete || derivedNoCtor.Minimum != ApiStabilityUnstable || !containsDependency(derivedNoCtor, "FxTaggedNoCtorClass") {
		t.Fatalf("FxDerivedNoCtorClass must expose only the tagged base level, got incomplete=%v min=%v deps=%v", derivedNoCtor.Incomplete, derivedNoCtor.Minimum, apiStabilityDependencyNames(derivedNoCtor))
	}
	assertQuiet("FxDerivedNoCtorClass", derivedNoCtor, "FxMethE")
	assertQuiet("FxDerivedNoCtorClass static type root", warm.UsedByType(c.GetTypeOfSymbol(apiStabilityTestExport(t, c, testFile, "FxDerivedNoCtorClass"))), "FxMethE")
	assertQuiet("cold FxDerivedNoCtorClass", cold.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "FxDerivedNoCtorClass")), "FxMethE")

	// An explicit stable base is a boundary that contributes nothing above
	// stable: the derived surface settles complete and quiet.
	stableCallable := surface("FxDerivedStableCallable")
	if stableCallable.Incomplete || len(stableCallable.Dependencies) != 0 {
		t.Fatalf("FxDerivedStableCallable must stay complete and quiet, got min=%v deps=%v", stableCallable.Minimum, apiStabilityDependencyNames(stableCallable))
	}

	// Own call and index signatures of an untagged derived interface stay
	// walked even though the base declares tagged ones.
	ownOverride := surface("FxOwnOverrideDerived")
	assertExposed("FxOwnOverrideDerived own signatures", ownOverride, "E")
	assertQuiet("FxOwnOverrideDerived", ownOverride, "FxCallE")
	assertQuiet("FxOwnOverrideDerived", ownOverride, "FxIdxE")

	// Independently represented arguments of a tagged ancestor stay exposed
	// while its flattened call signature internals stay bounded.
	callableBoxRef := referenceSurface("fxCallableBoxRef")
	assertExposed("fxCallableBoxRef tagged ancestor argument", callableBoxRef, "FxArgE")
	assertQuiet("fxCallableBoxRef", callableBoxRef, "FxPropE")
	if !containsDependency(callableBoxRef, "FxTaggedCallableBox") {
		t.Fatalf("fxCallableBoxRef should expose FxTaggedCallableBox, got %v", apiStabilityDependencyNames(callableBoxRef))
	}

	// Mixed inheritance still attributes the call signature to the tagged base
	// and leaves the untagged base's own member composing.
	mixedCall := surface("FxMixedCallInheritance")
	assertQuiet("FxMixedCallInheritance", mixedCall, "FxCallE")
	if !containsDependency(mixedCall, "FxTaggedFirstCall") {
		t.Fatalf("FxMixedCallInheritance should expose FxTaggedFirstCall, got %v", apiStabilityDependencyNames(mixedCall))
	}

	// A tagged base whose required generic argument cannot be established
	// stays incomplete instead of dropping the argument to look stable.
	if blocked := surface("FxBlockedDerived"); !blocked.Incomplete {
		t.Fatalf("a tagged base with a blocked required argument must stay incomplete, got min=%v deps=%v", blocked.Minimum, apiStabilityDependencyNames(blocked))
	}

	// Every tagged component keeps reporting its own internals as a root.
	callableBaseRoot := surface("FxTaggedCallableBase")
	assertExposed("FxTaggedCallableBase root", callableBaseRoot, "FxCallE")
	comboBaseRoot := surface("FxTaggedCombo")
	for _, dependency := range []string{"FxCallE", "FxCtorE", "FxIdxE", "FxMethE", "FxPropE"} {
		assertExposed("FxTaggedCombo root", comboBaseRoot, dependency)
	}
	classBaseRoot := surface("FxTaggedClassBase")
	for _, dependency := range []string{"FxCtorE", "FxMethE", "FxPropE"} {
		assertExposed("FxTaggedClassBase root", classBaseRoot, dependency)
	}
	indexBaseRoot := surface("FxTaggedIndexBase")
	assertExposed("FxTaggedIndexBase root", indexBaseRoot, "FxIdxE")

	// The flattened-inheritance reads are ordinary lazy checker reads under the
	// existing guard: they must not produce compiler diagnostics (in particular
	// no TS2589) for these public shapes.
	if globals := c.GetGlobalDiagnostics(); len(globals) != 0 {
		t.Errorf("flattened inheritance queries produced global diagnostics: %v", globals)
	}
}

// apiStabilitySharedTypeEntry returns the published snapshot of a concrete
// type surface, or nil when nothing was published for the identity.
func apiStabilitySharedTypeEntry(tp *TypeParser, target *checker.Type, inspect apiStabilityInspection) *apiStabilitySharedSurface {
	if tp == nil || tp.links == nil || target == nil {
		return nil
	}
	return tp.links.ApiStabilitySurfaceType.TryGet(apiStabilitySurfaceTypeKey{t: target, inspect: inspect})
}

// apiStabilitySharedSignatureEntry returns the published snapshot of a
// concrete signature surface, or nil when nothing was published.
func apiStabilitySharedSignatureEntry(tp *TypeParser, signature *checker.Signature) *apiStabilitySharedSurface {
	if tp == nil || tp.links == nil || signature == nil {
		return nil
	}
	return tp.links.ApiStabilitySurfaceSignature.TryGet(signature)
}

// apiStabilitySharedEntryHasSymbol reports whether a published snapshot still
// carries a symbol finding.
func apiStabilitySharedEntryHasSymbol(entry *apiStabilitySharedSurface, symbol *ast.Symbol) bool {
	if entry == nil || symbol == nil {
		return false
	}
	_, ok := entry.findings[apiStabilityFindingKey{symbol: symbol}]
	return ok
}

// apiStabilitySharedEntryRemovedSymbol reports whether a published snapshot
// removed a symbol finding as the component's own declared tag.
func apiStabilitySharedEntryRemovedSymbol(entry *apiStabilitySharedSurface, symbol *ast.Symbol) bool {
	if entry == nil || symbol == nil {
		return false
	}
	for _, key := range entry.removed {
		if key.symbol == symbol {
			return true
		}
	}
	return false
}

// apiStabilitySharedEntryHasSignature reports whether a published snapshot
// still carries a raw signature finding.
func apiStabilitySharedEntryHasSignature(entry *apiStabilitySharedSurface, signature *checker.Signature) bool {
	if entry == nil || signature == nil {
		return false
	}
	raw := rawSignature(signature)
	for key := range entry.findings {
		if key.signature != nil && rawSignature(key.signature) == raw {
			return true
		}
	}
	return false
}

// apiStabilitySharedEntryRemovedSignature reports whether a published snapshot
// removed a raw signature finding as the component's own declared tag.
func apiStabilitySharedEntryRemovedSignature(entry *apiStabilitySharedSurface, signature *checker.Signature) bool {
	if entry == nil || signature == nil {
		return false
	}
	raw := rawSignature(signature)
	for _, key := range entry.removed {
		if key.signature != nil && rawSignature(key.signature) == raw {
			return true
		}
	}
	return false
}

// TestApiStabilitySnapshotsExcludeOwnTagAndCompose pins
// the shared-cache contract: a complete root surface is published as an
// immutable snapshot whose own declared-tag findings were removed and recorded,
// the root result still excludes them after a reuse, and the same components
// reached as tagged children contribute their declared level without leaking
// the snapshot internals.
func TestApiStabilitySnapshotsExcludeOwnTagAndCompose(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
export interface E { value: string }
/** @stability unstable */
export interface Tagged { leak: E; plain: number }
/** @stability unstable */
export declare function taggedFn(): E
export interface Holder { tagged: Tagged; fn: typeof taggedFn }
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	testFile := files["/.src/test.ts"]
	eSymbol := apiStabilityTestExport(t, c, testFile, "E")
	tagged := apiStabilityTestExport(t, c, testFile, "Tagged")
	taggedDecl := c.GetDeclaredTypeOfSymbol(tagged)
	taggedFn := apiStabilityTestExport(t, c, testFile, "taggedFn")
	signature := apiStabilityTestSignatures(t, c, taggedFn)[0]

	// A type root excludes its own tag and publishes a snapshot that moved
	// exactly that finding to removed while keeping the exposed internals.
	rootType := tp.NewApiStabilitySession().UsedByType(taggedDecl)
	if rootType.Incomplete || rootType.Minimum != ApiStabilityExperimental || !containsDependency(rootType, "E") {
		t.Fatalf("Tagged type root = %v/%v, want experimental E", rootType.Minimum, apiStabilityDependencyNames(rootType))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, rootType, "Tagged")
	typeEntry := apiStabilitySharedTypeEntry(tp, taggedDecl, apiStabilityExpand)
	if typeEntry == nil {
		t.Fatal("a complete root type surface must be published")
	}
	if apiStabilitySharedEntryHasSymbol(typeEntry, tagged) {
		t.Fatal("the published snapshot must not keep the component's own tag finding")
	}
	if !apiStabilitySharedEntryRemovedSymbol(typeEntry, tagged) {
		t.Fatal("the published snapshot must record the removed own-tag finding")
	}
	if !apiStabilitySharedEntryHasSymbol(typeEntry, eSymbol) {
		t.Fatal("the published snapshot must keep the exposed child findings")
	}

	// A signature root excludes its own tag and publishes a snapshot with the
	// raw signature finding removed.
	rootSignature := tp.NewApiStabilitySession().UsedBySignature(signature)
	if rootSignature.Incomplete || rootSignature.Minimum != ApiStabilityExperimental || !containsDependency(rootSignature, "E") {
		t.Fatalf("taggedFn signature root = %v/%v, want experimental E", rootSignature.Minimum, apiStabilityDependencyNames(rootSignature))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, rootSignature, "taggedFn")
	signatureEntry := apiStabilitySharedSignatureEntry(tp, signature)
	if signatureEntry == nil {
		t.Fatal("a complete root signature surface must be published")
	}
	if apiStabilitySharedEntryHasSignature(signatureEntry, signature) {
		t.Fatal("the published signature snapshot must not keep the own tag finding")
	}
	if !apiStabilitySharedEntryRemovedSignature(signatureEntry, signature) {
		t.Fatal("the published signature snapshot must record the removed own-tag finding")
	}

	// Reusing the snapshots still excludes the roots' own tags.
	reusedType := tp.NewApiStabilitySession().UsedByType(taggedDecl)
	if reusedType.Incomplete || reusedType.Minimum != ApiStabilityExperimental || len(reusedType.Dependencies) != len(rootType.Dependencies) {
		t.Fatalf("reused type root disagrees: %v/%v", reusedType.Minimum, apiStabilityDependencyNames(reusedType))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, reusedType, "Tagged")
	reusedSignature := tp.NewApiStabilitySession().UsedBySignature(signature)
	if reusedSignature.Incomplete || reusedSignature.Minimum != ApiStabilityExperimental || len(reusedSignature.Dependencies) != len(rootSignature.Dependencies) {
		t.Fatalf("reused signature root disagrees: %v/%v", reusedSignature.Minimum, apiStabilityDependencyNames(reusedSignature))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, reusedSignature, "taggedFn")

	// The same components as children contribute only their declared level:
	// the tagged child boundary is decided before any snapshot is consulted.
	holder := apiStabilityTestExport(t, c, testFile, "Holder")
	holderUsed := tp.NewApiStabilitySession().UsedBySymbol(holder)
	if holderUsed.Incomplete || holderUsed.Minimum != ApiStabilityUnstable {
		t.Fatalf("Holder = %v/%v, want unstable from its tagged children", holderUsed.Minimum, apiStabilityDependencyNames(holderUsed))
	}
	if !containsDependency(holderUsed, "Tagged") || !containsDependency(holderUsed, "taggedFn") {
		t.Fatalf("Holder should expose both tagged children, got %v", apiStabilityDependencyNames(holderUsed))
	}
	if containsDependency(holderUsed, "E") {
		t.Fatalf("a cached tagged child must not leak its internals, got %v", apiStabilityDependencyNames(holderUsed))
	}
}

// TestApiStabilitySnapshotsReuseAcrossExportsFilesAndParsers
// pins the reuse contract: a complete, settled, context-free type surface is
// consumed by a later export, a later source file and a later TypeParser over
// the same checker without recomputation, the compelled root result is
// unchanged, and the published snapshot stays intact.
func TestApiStabilitySnapshotsReuseAcrossExportsFilesAndParsers(t *testing.T) {
	t.Parallel()

	c, tp1, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/dep.ts": `/** @stability experimental */
export interface E { value: string }
/** @stability unstable */
export interface Shared { leak: E }
`,
		"/.src/consumer.ts": `import { Shared } from "./dep.js"
export interface Consumer { shared: Shared }
export interface AlsoConsumer { other: Shared }
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	shared := apiStabilityTestExport(t, c, files["/.src/dep.ts"], "Shared")
	sharedDecl := c.GetDeclaredTypeOfSymbol(shared)

	first := tp1.NewApiStabilitySession().UsedByType(sharedDecl)
	if first.Incomplete || first.Minimum != ApiStabilityExperimental || !containsDependency(first, "E") {
		t.Fatalf("Shared root = %v/%v, want experimental E", first.Minimum, apiStabilityDependencyNames(first))
	}
	entry := apiStabilitySharedTypeEntry(tp1, sharedDecl, apiStabilityExpand)
	if entry == nil {
		t.Fatal("the first export must publish the complete Shared surface")
	}
	published := len(entry.findings)

	// A later TypeParser over the same checker consumes the snapshot directly:
	// the root query is exact and consumes no analysis work.
	tp2 := NewTypeParser(c.Program(), c)
	hit := newApiStabilityAnalysis(tp2)
	hit.typeSurface(sharedDecl, apiStabilityExpand, apiStabilitySubstitution{})
	if hit.work != 0 {
		t.Fatalf("a published snapshot hit consumed %d work", hit.work)
	}
	second := tp2.NewApiStabilitySession().UsedByType(sharedDecl)
	if second.Minimum != first.Minimum || len(second.Dependencies) != len(first.Dependencies) {
		t.Fatalf("reused root disagrees: first=%v second=%v", apiStabilityDependencyNames(first), apiStabilityDependencyNames(second))
	}

	// A later source file reaches the same component as a shallow child and
	// publishes its own inspection-mode snapshot; a second export in that file
	// reuses it without recomputation.
	consumer := apiStabilityTestExport(t, c, files["/.src/consumer.ts"], "Consumer")
	consumerUsed := tp2.NewApiStabilitySession().UsedBySymbol(consumer)
	if consumerUsed.Incomplete || consumerUsed.Minimum != ApiStabilityUnstable || !containsDependency(consumerUsed, "Shared") {
		t.Fatalf("Consumer = %v/%v, want unstable Shared", consumerUsed.Minimum, apiStabilityDependencyNames(consumerUsed))
	}
	shallowEntry := apiStabilitySharedTypeEntry(tp2, sharedDecl, apiStabilityShallow)
	if shallowEntry == nil {
		t.Fatal("the shallow child surface must be published for the same concrete type")
	}
	alsoConsumer := apiStabilityTestExport(t, c, files["/.src/consumer.ts"], "AlsoConsumer")
	shallowHit := newApiStabilityAnalysis(tp2)
	shallowHit.typeSurface(sharedDecl, apiStabilityShallow, apiStabilitySubstitution{})
	if shallowHit.work != 0 {
		t.Fatalf("a published shallow snapshot hit consumed %d work", shallowHit.work)
	}
	alsoUsed := tp2.NewApiStabilitySession().UsedBySymbol(alsoConsumer)
	if alsoUsed.Minimum != consumerUsed.Minimum || len(alsoUsed.Dependencies) != len(consumerUsed.Dependencies) {
		t.Fatalf("second consumer disagrees: %v/%v", alsoUsed.Minimum, apiStabilityDependencyNames(alsoUsed))
	}

	// Neither reuse nor the root conversion mutated the published snapshots.
	after := apiStabilitySharedTypeEntry(tp2, sharedDecl, apiStabilityExpand)
	if after == nil || len(after.findings) != published {
		t.Fatalf("the published expand snapshot was mutated: before=%d after=%v", published, after)
	}
	if apiStabilitySharedEntryHasSymbol(after, shared) {
		t.Fatal("the published snapshot gained the own tag after reuse")
	}
}

// TestApiStabilityInspectionModesStaySeparate proves the
// shallow and expanded results of the same concrete type are separate entry
// slots in both computation orders, so a root snapshot can never leak internals
// through a shallow child path. The untagged component is the sharpest case:
// its expanded snapshot really does contain the member internals, while its
// shallow child surface must stay a signature-only declaration surface.
func TestApiStabilityInspectionModesStaySeparate(t *testing.T) {
	t.Parallel()

	for _, order := range []string{"expand-first", "shallow-first"} {
		t.Run(order, func(t *testing.T) {
			t.Parallel()
			c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
				"/.src/test.ts": `/** @stability experimental */
export interface E { value: string }
/** @stability unstable */
export interface Tagged { leak: E; plain: number }
export interface Child { tagged: Tagged }
export interface Plain { leak: E }
export interface PlainChild { plain: Plain }
`,
			})
			defer done()
			primeApiStabilityChecker(t, c)

			testFile := files["/.src/test.ts"]
			eSymbol := apiStabilityTestExport(t, c, testFile, "E")
			tagged := apiStabilityTestExport(t, c, testFile, "Tagged")
			taggedDecl := c.GetDeclaredTypeOfSymbol(tagged)
			child := apiStabilityTestExport(t, c, testFile, "Child")
			plain := apiStabilityTestExport(t, c, testFile, "Plain")
			plainDecl := c.GetDeclaredTypeOfSymbol(plain)
			plainChild := apiStabilityTestExport(t, c, testFile, "PlainChild")

			checkChildren := func(label string) {
				t.Helper()
				childUsed := tp.NewApiStabilitySession().UsedBySymbol(child)
				if childUsed.Incomplete || childUsed.Minimum != ApiStabilityUnstable || containsDependency(childUsed, "E") {
					t.Fatalf("%s: Child = %v/%v, want only the unstable child", label, childUsed.Minimum, apiStabilityDependencyNames(childUsed))
				}
				plainUsed := tp.NewApiStabilitySession().UsedBySymbol(plainChild)
				if plainUsed.Incomplete || len(plainUsed.Dependencies) != 0 {
					t.Fatalf("%s: PlainChild = %v/%v, want a quiet shallow surface", label, plainUsed.Minimum, apiStabilityDependencyNames(plainUsed))
				}
			}
			checkRoots := func(label string) {
				t.Helper()
				root := tp.NewApiStabilitySession().UsedByType(taggedDecl)
				if root.Incomplete || root.Minimum != ApiStabilityExperimental || !containsDependency(root, "E") {
					t.Fatalf("%s: Tagged root = %v/%v, want experimental E", label, root.Minimum, apiStabilityDependencyNames(root))
				}
				assertApiStabilitySurfaceExcludesOwnTag(t, root, "Tagged")
				plainRoot := tp.NewApiStabilitySession().UsedByType(plainDecl)
				if plainRoot.Incomplete || plainRoot.Minimum != ApiStabilityExperimental || !containsDependency(plainRoot, "E") {
					t.Fatalf("%s: Plain root = %v/%v, want experimental E", label, plainRoot.Minimum, apiStabilityDependencyNames(plainRoot))
				}
			}

			if order == "shallow-first" {
				checkChildren("shallow-first")
			}
			checkRoots(order)

			taggedExpand := apiStabilitySharedTypeEntry(tp, taggedDecl, apiStabilityExpand)
			plainExpand := apiStabilitySharedTypeEntry(tp, plainDecl, apiStabilityExpand)
			if taggedExpand == nil || plainExpand == nil {
				t.Fatal("both expanded root snapshots must be published")
			}
			if !apiStabilitySharedEntryHasSymbol(plainExpand, eSymbol) {
				t.Fatal("the plain expanded snapshot must keep the internals")
			}

			if order == "expand-first" {
				checkChildren("expand-first")
			}

			// Each shallow child computation published its own mode slot: it
			// contains no internals, and the tagged one records the removed own
			// tag.
			plainShallow := apiStabilitySharedTypeEntry(tp, plainDecl, apiStabilityShallow)
			if plainShallow == nil {
				t.Fatal("the plain shallow surface must be published separately")
			}
			if apiStabilitySharedEntryHasSymbol(plainShallow, eSymbol) {
				t.Fatal("the shallow snapshot must not contain expanded internals")
			}
			taggedShallow := apiStabilitySharedTypeEntry(tp, taggedDecl, apiStabilityShallow)
			if taggedShallow == nil {
				t.Fatal("the tagged shallow surface must be published separately")
			}
			if apiStabilitySharedEntryHasSymbol(taggedShallow, eSymbol) {
				t.Fatal("the tagged shallow snapshot must not contain expanded internals")
			}
			if !apiStabilitySharedEntryRemovedSymbol(taggedShallow, tagged) {
				t.Fatal("the tagged shallow snapshot must record the removed own tag")
			}
			// The expanded plain snapshot must still keep the internals after
			// the shallow child composition.
			if !apiStabilitySharedEntryHasSymbol(apiStabilitySharedTypeEntry(tp, plainDecl, apiStabilityExpand), eSymbol) {
				t.Fatal("the shallow composition contaminated the expanded snapshot")
			}
		})
	}
}

// TestApiStabilityConcreteInstantiationsAndCarriers proves
// concrete instantiations keep separate snapshots in both computation orders
// and that a surface computed under a substitution carrier is never published.
func TestApiStabilityConcreteInstantiationsAndCarriers(t *testing.T) {
	t.Parallel()

	for _, order := range []string{"leaky-first", "clean-first"} {
		t.Run(order, func(t *testing.T) {
			t.Parallel()
			leaky := "export declare const leaky: Box<E>"
			clean := "export declare const clean: Box<string>"
			declarations := leaky + "\n" + clean
			if order == "clean-first" {
				declarations = clean + "\n" + leaky
			}
			c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
				"/.src/test.ts": `/** @stability experimental */
export interface E { value: string }
export interface Box<T> { value: T }
` + declarations + "\n",
			})
			defer done()
			primeApiStabilityChecker(t, c)

			testFile := files["/.src/test.ts"]
			leakySymbol := apiStabilityTestExport(t, c, testFile, "leaky")
			cleanSymbol := apiStabilityTestExport(t, c, testFile, "clean")
			leakyType := c.GetTypeOfSymbol(leakySymbol)
			cleanType := c.GetTypeOfSymbol(cleanSymbol)
			eSymbol := apiStabilityTestExport(t, c, testFile, "E")

			leakyUsed := tp.NewApiStabilitySession().UsedBySymbol(leakySymbol)
			if leakyUsed.Incomplete || !containsDependency(leakyUsed, "E") {
				t.Fatalf("Box<E> should expose E, got %v", apiStabilityDependencyNames(leakyUsed))
			}
			cleanUsed := tp.NewApiStabilitySession().UsedBySymbol(cleanSymbol)
			if containsDependency(cleanUsed, "E") {
				t.Fatalf("Box<string> must not inherit E, got %v", apiStabilityDependencyNames(cleanUsed))
			}

			leakyEntry := apiStabilitySharedTypeEntry(tp, leakyType, apiStabilityExpand)
			cleanEntry := apiStabilitySharedTypeEntry(tp, cleanType, apiStabilityExpand)
			if leakyEntry == nil || cleanEntry == nil {
				t.Fatal("both concrete instantiations must publish their own snapshot")
			}
			if !apiStabilitySharedEntryHasSymbol(leakyEntry, eSymbol) {
				t.Fatal("the Box<E> snapshot must keep E")
			}
			if apiStabilitySharedEntryHasSymbol(cleanEntry, eSymbol) {
				t.Fatal("the Box<string> snapshot must not contain E")
			}

			// The raw declaration surface is only ever reached under a
			// reference substitution for these roots, so it must not publish a
			// context-free entry for either mode.
			box := apiStabilityTestExport(t, c, testFile, "Box")
			boxDecl := c.GetDeclaredTypeOfSymbol(box)
			if entry := apiStabilitySharedTypeEntry(tp, boxDecl, apiStabilityExpand); entry != nil {
				t.Fatal("a substituted raw declaration surface must stay analysis-local")
			}
			if entry := apiStabilitySharedTypeEntry(tp, boxDecl, apiStabilityShallow); entry != nil {
				t.Fatal("a substituted raw declaration surface must stay analysis-local")
			}
		})
	}

	// A raw signature analyzed under a concrete signature substitution stays
	// analysis-local: the concrete identity is in the key, but the carrier is
	// not persistent, so the raw signature slot must never receive the
	// substituted result.
	t.Run("substituted raw signature stays local", func(t *testing.T) {
		t.Parallel()
		c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
			"/.src/test.ts": `/** @stability experimental */
export interface E { value: string }
declare function make<T>(): (x: T) => T
export const leaky = make<E>()
`,
		})
		defer done()
		primeApiStabilityChecker(t, c)

		leaky := apiStabilityTestExport(t, c, files["/.src/test.ts"], "leaky")
		instantiated := apiStabilityTestSignatures(t, c, leaky)[0]
		raw := rawSignature(instantiated)
		if raw == instantiated {
			t.Fatal("expected an instantiated signature with a raw target")
		}

		analysis := newApiStabilityAnalysis(tp)
		subst := analysis.extendSignatureSubstitution(apiStabilitySubstitution{}, instantiated)
		inner := analysis.signatureSurface(raw, subst)
		analysis.settle()
		if apiStabilitySharedSignatureEntry(tp, raw) != nil {
			t.Fatal("a substituted raw signature surface must not be published")
		}
		stored := apiStabilityUsedOf(inner, nil)
		if stored.Incomplete || !containsDependency(stored, "E") {
			t.Fatalf("the substituted raw signature should expose E, got %v", apiStabilityDependencyNames(stored))
		}

		// The concrete signature is context-free and may be published; the raw
		// slot must still be empty afterwards.
		_ = tp.NewApiStabilitySession().UsedBySignature(instantiated)
		if apiStabilitySharedSignatureEntry(tp, raw) != nil {
			t.Fatal("the concrete signature published under the raw signature slot")
		}
		if apiStabilitySharedSignatureEntry(tp, instantiated) == nil {
			t.Fatal("the concrete context-free signature should be published")
		}
	})
}

// TestApiStabilityDeclaredReadsPublishNothing proves the
// declared accessors stay independent of the shared surface caches: reading a
// declared level neither computes nor publishes a surface.
func TestApiStabilityDeclaredReadsPublishNothing(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability unstable */
export interface Tagged { leak: string }
/** @stability unstable */
export declare function taggedFn(): string
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	testFile := files["/.src/test.ts"]
	tagged := apiStabilityTestExport(t, c, testFile, "Tagged")
	taggedFn := apiStabilityTestExport(t, c, testFile, "taggedFn")
	signature := apiStabilityTestSignatures(t, c, taggedFn)[0]

	before := c.TotalInstantiationCount
	declaredSymbol := tp.DeclaredApiStabilityOfSymbol(tagged)
	declaredFn := tp.DeclaredApiStabilityOfSymbol(taggedFn)
	declaredSignature := tp.DeclaredApiStabilityOfSignature(signature)
	if delta := c.TotalInstantiationCount - before; delta != 0 {
		t.Fatalf("declared reads instantiated %d types, want 0", delta)
	}
	if declaredSymbol.Level != ApiStabilityUnstable || declaredSignature.Level != ApiStabilityUnstable || declaredFn.Level != ApiStabilityUnstable {
		t.Fatalf("unexpected declared levels: %v/%v/%v", declaredSymbol.Level, declaredFn.Level, declaredSignature.Level)
	}
	if apiStabilitySharedTypeEntry(tp, c.GetDeclaredTypeOfSymbol(tagged), apiStabilityExpand) != nil {
		t.Fatal("a declared read published a type surface")
	}
	if apiStabilitySharedSignatureEntry(tp, signature) != nil {
		t.Fatal("a declared read published a signature surface")
	}
}

// TestApiStabilitySnapshotMutationIsIsolated pins the
// immutability requirement: consuming a snapshot copies it, so a caller
// mutating its local surface cannot change the published findings, and
// provenance recorded in the snapshot survives for diagnostics.
func TestApiStabilitySnapshotMutationIsIsolated(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
export interface E { value: string }
export interface Shared { leak: E }
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	testFile := files["/.src/test.ts"]
	eSymbol := apiStabilityTestExport(t, c, testFile, "E")
	shared := apiStabilityTestExport(t, c, testFile, "Shared")
	sharedDecl := c.GetDeclaredTypeOfSymbol(shared)

	first := tp.NewApiStabilitySession().UsedByType(sharedDecl)
	if first.Incomplete || !containsDependency(first, "E") {
		t.Fatalf("Shared root should expose E, got %v", apiStabilityDependencyNames(first))
	}
	entry := apiStabilitySharedTypeEntry(tp, sharedDecl, apiStabilityExpand)
	if entry == nil || !apiStabilitySharedEntryHasSymbol(entry, eSymbol) {
		t.Fatal("the published snapshot should contain the E finding")
	}
	published := len(entry.findings)

	// Consume the snapshot, then mutate the local copy destructively.
	session := tp.NewApiStabilitySession()
	used := session.UsedByType(sharedDecl)
	if used.Incomplete || !containsDependency(used, "E") {
		t.Fatalf("the reused root lost E, got %v", apiStabilityDependencyNames(used))
	}
	local := session.analysis.sessionTypes[apiStabilityTypeKey{t: sharedDecl, inspect: apiStabilityExpand}]
	delete(local.findings, apiStabilityFindingKey{symbol: eSymbol})
	local.findings[apiStabilityFindingKey{declaration: testFile.AsNode()}] = apiStabilityFinding{level: ApiStabilityExperimental}

	after := apiStabilitySharedTypeEntry(tp, sharedDecl, apiStabilityExpand)
	if after == nil || len(after.findings) != published {
		t.Fatalf("the published snapshot changed after local mutation: before=%d after=%d", published, len(after.findings))
	}
	if !apiStabilitySharedEntryHasSymbol(after, eSymbol) {
		t.Fatal("local mutation removed a finding from the published snapshot")
	}

	// Provenance for a snapshot-sourced finding is retained for diagnostics.
	foundProvenance := false
	for _, dependency := range first.Dependencies {
		if ApiStabilityDependencyName(dependency) == "E" && dependency.Declaration != nil {
			foundProvenance = true
		}
	}
	if !foundProvenance {
		t.Fatal("a snapshot finding lost its declaration provenance")
	}
}

// TestApiStabilityIncompleteAndBlockedAreNeverPublished
// proves only complete, settled results are published: a cold blocked surface
// and a budget-exhausted read publish nothing, and a later materialized retry
// settles complete and publishes a snapshot a new session can reuse.
func TestApiStabilityIncompleteAndBlockedAreNeverPublished(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
export interface E { value: string }
/** @stability unstable */
export interface Cold { plain: number; [Symbol.iterator](): E }
`,
	})
	defer done()

	testFile := files["/.src/test.ts"]
	cold := apiStabilityTestExport(t, c, testFile, "Cold")
	coldDecl := c.GetDeclaredTypeOfSymbol(cold)

	coldUsed := tp.NewApiStabilitySession().UsedBySymbol(cold)
	if !coldUsed.Incomplete {
		t.Fatal("an unmaterialized computed member must report an incomplete result")
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, coldUsed, "Cold")
	if apiStabilitySharedTypeEntry(tp, coldDecl, apiStabilityExpand) != nil {
		t.Fatal("an incomplete cold surface must not be published")
	}

	// A budget-exhausted read is blocked and publishes nothing either.
	exhausted := newApiStabilityAnalysis(tp)
	exhausted.work = apiStabilityMaxWork
	blocked := exhausted.typeSurface(coldDecl, apiStabilityExpand, apiStabilitySubstitution{})
	exhausted.settle()
	if !blocked.blocked {
		t.Fatal("an exhausted read must be blocked")
	}
	if apiStabilitySharedTypeEntry(tp, coldDecl, apiStabilityExpand) != nil {
		t.Fatal("a budget-blocked surface must not be published")
	}

	// After the checker materializes the computed member, the retry settles
	// complete and publishes the snapshot; a new session reuses it.
	primeApiStabilityChecker(t, c)
	warm := tp.NewApiStabilitySession().UsedBySymbol(cold)
	if warm.Incomplete || !containsDependency(warm, "E") {
		t.Fatalf("the materialized retry should expose E, got %v", apiStabilityDependencyNames(warm))
	}
	if apiStabilitySharedTypeEntry(tp, coldDecl, apiStabilityExpand) == nil {
		t.Fatal("the complete retry must publish its snapshot")
	}
	reuse := newApiStabilityAnalysis(tp)
	reuse.typeSurface(coldDecl, apiStabilityExpand, apiStabilitySubstitution{})
	if reuse.work != 0 {
		t.Fatalf("the published retry snapshot consumed %d work", reuse.work)
	}
	reused := tp.NewApiStabilitySession().UsedBySymbol(cold)
	if reused.Incomplete || !containsDependency(reused, "E") || len(reused.Dependencies) != len(warm.Dependencies) {
		t.Fatalf("reused surface disagrees: %v/%v", reused.Minimum, apiStabilityDependencyNames(reused))
	}
}

// TestApiStabilityCyclesPublishOnlySettledComplete pins
// the cycle policy: a cold mutual cycle with an unmaterialized computed member
// is incomplete and never published, the materialized retry settles complete
// and publishes, and the published snapshots keep the cycle diagnostic in both
// query orders.
func TestApiStabilityCyclesPublishOnlySettledComplete(t *testing.T) {
	t.Parallel()

	fixture := map[string]string{
		"/.src/test.ts": `/** @stability experimental */
interface E { value: string }
export interface ColdA { b: ColdB }
export interface ColdB { a: ColdA; payload: E; [Symbol.iterator](): E }
`,
	}
	for _, order := range []string{"b-first", "a-first"} {
		t.Run(order, func(t *testing.T) {
			t.Parallel()
			c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, fixture)
			defer done()

			testFile := files["/.src/test.ts"]
			a := apiStabilityTestExport(t, c, testFile, "ColdA")
			b := apiStabilityTestExport(t, c, testFile, "ColdB")
			bDecl := c.GetDeclaredTypeOfSymbol(b)

			var coldA, coldB ApiStabilityUsed
			if order == "b-first" {
				coldB = tp.NewApiStabilitySession().UsedBySymbol(b)
				coldA = tp.NewApiStabilitySession().UsedBySymbol(a)
			} else {
				coldA = tp.NewApiStabilitySession().UsedBySymbol(a)
				coldB = tp.NewApiStabilitySession().UsedBySymbol(b)
			}
			if !coldB.Incomplete {
				t.Fatal("the cold computed member must keep ColdB incomplete")
			}
			if apiStabilitySharedTypeEntry(tp, bDecl, apiStabilityExpand) != nil {
				t.Fatal("an incomplete cold cycle surface must not be published")
			}
			if coldA.Incomplete || containsDependency(coldA, "E") {
				t.Fatalf("ColdA = %v/%v, want a complete shallow surface", coldA.Minimum, apiStabilityDependencyNames(coldA))
			}

			primeApiStabilityChecker(t, c)
			warmB := tp.NewApiStabilitySession().UsedBySymbol(b)
			if warmB.Incomplete || warmB.Minimum != ApiStabilityExperimental || !containsDependency(warmB, "E") {
				t.Fatalf("warm ColdB = %v/%v, want experimental E", warmB.Minimum, apiStabilityDependencyNames(warmB))
			}
			if apiStabilitySharedTypeEntry(tp, bDecl, apiStabilityExpand) == nil {
				t.Fatal("the warm complete cycle must publish its snapshot")
			}
			warmA := tp.NewApiStabilitySession().UsedBySymbol(a)
			if warmA.Incomplete || containsDependency(warmA, "E") {
				t.Fatalf("warm ColdA = %v/%v, want only the shallow surface", warmA.Minimum, apiStabilityDependencyNames(warmA))
			}
			reuse := newApiStabilityAnalysis(tp)
			reuse.typeSurface(bDecl, apiStabilityExpand, apiStabilitySubstitution{})
			if reuse.work != 0 {
				t.Fatalf("the published cycle snapshot consumed %d work", reuse.work)
			}
		})
	}
}

// TestApiStabilityOverloadTagsSurviveSnapshotReuse pins
// the overload split through the shared caches: the root symbol's declared own
// overload stays excluded and its sibling experimental overload stays an
// exposed child when both symbol and signature surfaces are reused in a later
// session, and each overload's own tag stays excluded from its own reused
// signature root.
func TestApiStabilityOverloadTagsSurviveSnapshotReuse(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability unstable */
export function mixed(x: string): string
/** @stability experimental */
export function mixed(x: number): number
export function mixed(x: string | number): string | number { return "" }
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	mixed := apiStabilityTestExport(t, c, files["/.src/test.ts"], "mixed")
	signatures := apiStabilityTestSignatures(t, c, mixed)
	if len(signatures) != 2 {
		t.Fatalf("expected two public overloads, got %d", len(signatures))
	}

	first := tp.NewApiStabilitySession().UsedBySymbol(mixed)
	if first.Incomplete || first.Minimum != ApiStabilityExperimental {
		t.Fatalf("mixed = %v/%v, want experimental from the sibling overload", first.Minimum, apiStabilityDependencyNames(first))
	}
	for _, dependency := range first.Dependencies {
		if dependency.Signature != nil && rawSignature(dependency.Signature) == rawSignature(signatures[0]) {
			t.Fatalf("the declared own overload leaked into the surface: %v", apiStabilityDependencyNames(first))
		}
	}
	if apiStabilitySharedSignatureEntry(tp, signatures[0]) == nil || apiStabilitySharedSignatureEntry(tp, signatures[1]) == nil {
		t.Fatal("both overload signature surfaces should be published")
	}
	// Each signature snapshot excludes only its own tag; the sibling overload
	// surfaces again when composed into the symbol root.
	for index, signature := range signatures {
		if !apiStabilitySharedEntryRemovedSignature(apiStabilitySharedSignatureEntry(tp, signature), signature) {
			t.Fatalf("overload %d own tag must be removed from its snapshot", index)
		}
	}

	// A later session composes the snapshots and keeps the same split.
	reused := tp.NewApiStabilitySession().UsedBySymbol(mixed)
	if reused.Incomplete || reused.Minimum != first.Minimum || len(reused.Dependencies) != len(first.Dependencies) {
		t.Fatalf("reused mixed = %v/%v, want %v", reused.Minimum, apiStabilityDependencyNames(reused), apiStabilityDependencyNames(first))
	}
	for index, signature := range signatures {
		used := tp.NewApiStabilitySession().UsedBySignature(signature)
		if used.Incomplete || used.Minimum != ApiStabilityStable || len(used.Dependencies) != 0 {
			t.Fatalf("reused overload %d own tag must be excluded, got min=%v deps=%v", index, used.Minimum, apiStabilityDependencyNames(used))
		}
	}
}

// TestApiStabilitySharedCachesAreCheckerLocal proves the
// published snapshots live on the per-checker EffectLinks: a second checker
// never observes the first checker's entries for the same source shape.
func TestApiStabilitySharedCachesAreCheckerLocal(t *testing.T) {
	t.Parallel()

	source := `/** @stability experimental */
export interface E { value: string }
/** @stability unstable */
export interface Shared { leak: E }
export interface Holder { child: Shared }
`
	c1, tp1, files1, done1 := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done1()
	c2, tp2, files2, done2 := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done2()
	primeApiStabilityChecker(t, c1)
	primeApiStabilityChecker(t, c2)

	if tp1.links == tp2.links {
		t.Fatal("different checkers must not share EffectLinks")
	}

	shared1 := apiStabilityTestExport(t, c1, files1, "Shared")
	decl1 := c1.GetDeclaredTypeOfSymbol(shared1)
	holder1 := apiStabilityTestExport(t, c1, files1, "Holder")
	if used := tp1.NewApiStabilitySession().UsedBySymbol(holder1); !containsDependency(used, "Shared") {
		t.Fatalf("first checker should expose Shared, got %v", apiStabilityDependencyNames(used))
	}
	if apiStabilitySharedTypeEntry(tp1, decl1, apiStabilityShallow) == nil {
		t.Fatal("the first checker should publish its shallow snapshot")
	}

	shared2 := apiStabilityTestExport(t, c2, files2, "Shared")
	decl2 := c2.GetDeclaredTypeOfSymbol(shared2)
	if shared1 == shared2 || decl1 == decl2 {
		t.Fatal("checkers should produce distinct identities")
	}
	if apiStabilitySharedTypeEntry(tp2, decl1, apiStabilityShallow) != nil {
		t.Fatal("the second checker observed the first checker's snapshot")
	}
	if apiStabilitySharedTypeEntry(tp2, decl2, apiStabilityShallow) != nil {
		t.Fatal("the second checker published nothing before its own query")
	}
	holder2 := apiStabilityTestExport(t, c2, files2, "Holder")
	if used := tp2.NewApiStabilitySession().UsedBySymbol(holder2); !containsDependency(used, "Shared") {
		t.Fatalf("second checker should expose Shared independently, got %v", apiStabilityDependencyNames(used))
	}
	if apiStabilitySharedTypeEntry(tp1, decl2, apiStabilityShallow) != nil {
		t.Fatal("the first checker observed the second checker's snapshot")
	}
}

// TestApiStabilityColdCompleteAgreesWithPureWarm pins the
// cold-complete publication policy: a surface that settles complete on an
// unprimed checker may be published, and reusing it after the checker
// materializes the program must agree with a pure warm recomputation on a
// fresh checker. The cold result is never trusted because of an allocation
// epoch: it is only published once the session established it complete.
func TestApiStabilityColdCompleteAgreesWithPureWarm(t *testing.T) {
	t.Parallel()

	source := `/** @stability experimental */
interface E { value: string }
type Deep<T> = T extends string ? Deep<T> : T
interface Callable<T> { (x: T): void }
export declare const f: Callable<Deep<E>>
`
	cold, tpCold, coldFiles, coldDone := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{"/.src/test.ts": source})
	defer coldDone()
	coldSymbol := apiStabilityTestExport(t, cold, coldFiles["/.src/test.ts"], "f")
	coldUsed := tpCold.NewApiStabilitySession().UsedBySymbol(coldSymbol)
	if coldUsed.Incomplete || !containsDependency(coldUsed, "E") {
		t.Fatalf("the cold surface should lazily materialize E, got %v", apiStabilityDependencyNames(coldUsed))
	}
	coldType := cold.GetTypeOfSymbol(coldSymbol)
	if apiStabilitySharedTypeEntry(tpCold, coldType, apiStabilityExpand) == nil {
		t.Fatal("the cold complete surface should be published")
	}

	// The same checker materialized: the reused snapshot must agree with a
	// pure warm recomputation on a fresh checker.
	primeApiStabilityChecker(t, cold)
	reused := NewTypeParser(cold.Program(), cold).NewApiStabilitySession().UsedBySymbol(coldSymbol)
	if reused.Incomplete {
		t.Fatal("the reused snapshot must stay complete")
	}

	warm, tpWarm, warmFiles, warmDone := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{"/.src/test.ts": source})
	defer warmDone()
	primeApiStabilityChecker(t, warm)
	warmSymbol := apiStabilityTestExport(t, warm, warmFiles["/.src/test.ts"], "f")
	pure := tpWarm.NewApiStabilitySession().UsedBySymbol(warmSymbol)
	if pure.Incomplete {
		t.Fatal("the pure warm recomputation must be complete")
	}

	if reused.Minimum != pure.Minimum {
		t.Fatalf("reused minimum %v disagrees with pure warm %v", reused.Minimum, pure.Minimum)
	}
	reusedNames, pureNames := apiStabilityDependencyNames(reused), apiStabilityDependencyNames(pure)
	if len(reusedNames) != len(pureNames) {
		t.Fatalf("reused dependencies %v disagree with pure warm %v", reusedNames, pureNames)
	}
	for index := range reusedNames {
		if reusedNames[index] != pureNames[index] {
			t.Fatalf("reused dependencies %v disagree with pure warm %v", reusedNames, pureNames)
		}
	}
}

// TestApiStabilitySymbolLessTypesReuse pins the snapshot
// contract for represented types without an identity symbol: a union publishes
// its component findings with nothing removed, and reusing it across sessions
// returns the same root-excluding result.
func TestApiStabilitySymbolLessTypesReuse(t *testing.T) {
	t.Parallel()

	c, tp, files, done := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{
		"/.src/test.ts": `/** @stability experimental */
export interface E { value: string }
export interface Plain { value: string }
export declare const choose: E | Plain
`,
	})
	defer done()
	primeApiStabilityChecker(t, c)

	choose := apiStabilityTestExport(t, c, files["/.src/test.ts"], "choose")
	union := c.GetTypeOfSymbol(choose)
	if union.Flags()&checker.TypeFlagsUnionOrIntersection == 0 {
		t.Fatalf("expected a union type, got flags %v", union.Flags())
	}

	first := tp.NewApiStabilitySession().UsedByType(union)
	if first.Incomplete || first.Minimum != ApiStabilityExperimental || !containsDependency(first, "E") {
		t.Fatalf("union root = %v/%v, want experimental E", first.Minimum, apiStabilityDependencyNames(first))
	}
	entry := apiStabilitySharedTypeEntry(tp, union, apiStabilityExpand)
	if entry == nil {
		t.Fatal("a symbol-less complete surface must be published")
	}
	if len(entry.removed) != 0 {
		t.Fatalf("a symbol-less type has no own tag to remove, got %v", entry.removed)
	}
	if !apiStabilitySharedEntryHasSymbol(entry, apiStabilityTestExport(t, c, files["/.src/test.ts"], "E")) {
		t.Fatal("the union snapshot must keep its component finding")
	}

	reused := tp.NewApiStabilitySession().UsedByType(union)
	if reused.Incomplete || reused.Minimum != first.Minimum || len(reused.Dependencies) != len(first.Dependencies) {
		t.Fatalf("reused union = %v/%v, want %v", reused.Minimum, apiStabilityDependencyNames(reused), apiStabilityDependencyNames(first))
	}
	direct := newApiStabilityAnalysis(tp)
	direct.typeSurface(union, apiStabilityExpand, apiStabilitySubstitution{})
	if direct.work != 0 {
		t.Fatalf("the symbol-less snapshot hit consumed %d work", direct.work)
	}
}
