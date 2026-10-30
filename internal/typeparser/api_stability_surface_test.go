package typeparser

import (
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

func assertApiStabilitySurfaceExcludesOwnTag(t *testing.T, used ApiStabilityUsed, name string) {
	t.Helper()
	for _, dependency := range used.dependencies {
		if ApiStabilityDependencyName(dependency) == name {
			t.Fatalf("root tag %q leaked into its own surface result: %v", name, apiStabilityDependencyNames(used))
		}
	}
}

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

	taggedRoot := apiStabilityTestExport(t, c, testFile, "TaggedRoot")
	taggedRootDeclared := tp.DeclaredApiStabilityOfSymbol(taggedRoot)
	if taggedRootDeclared.level != ApiStabilityUnstable || taggedRootDeclared.declaration == nil {
		t.Fatalf("TaggedRoot declared = %+v, want unstable with provenance", taggedRootDeclared)
	}
	experimentalDeclared := tp.DeclaredApiStabilityOfSymbol(apiStabilityTestExport(t, c, testFile, "TaggedExperimentalRoot"))
	if experimentalDeclared.level != ApiStabilityExperimental || experimentalDeclared.declaration == nil {
		t.Fatalf("TaggedExperimentalRoot declared = %+v, want experimental with provenance", experimentalDeclared)
	}
	explicitStableDeclared := tp.DeclaredApiStabilityOfSymbol(apiStabilityTestExport(t, c, testFile, "ExplicitStableRoot"))
	if explicitStableDeclared.level != ApiStabilityStable || explicitStableDeclared.declaration == nil {
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
		if used.incomplete {
			t.Fatalf("%s: a leaf surface with only an own tag must settle complete", test.name)
		}
		if used.minimum != ApiStabilityStable || len(used.dependencies) != 0 {
			t.Fatalf("%s: own tag must be excluded, got min=%v deps=%v", test.name, used.minimum, apiStabilityDependencyNames(used))
		}
	}

	taggedFn := apiStabilityTestExport(t, c, testFile, "taggedFn")
	fnDeclared := tp.DeclaredApiStabilityOfSignature(apiStabilityTestSignatures(t, c, taggedFn)[0])
	if fnDeclared.level != ApiStabilityUnstable || fnDeclared.declaration == nil {
		t.Fatalf("taggedFn signature declared = %+v, want unstable with provenance", fnDeclared)
	}
	fnUsed := session.UsedBySignature(apiStabilityTestSignatures(t, c, taggedFn)[0])
	if fnUsed.incomplete || fnUsed.minimum != ApiStabilityStable || len(fnUsed.dependencies) != 0 {
		t.Fatalf("taggedFn signature own tag must be excluded, got min=%v deps=%v", fnUsed.minimum, apiStabilityDependencyNames(fnUsed))
	}

	withChild := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "TaggedRootWithChild"))
	if withChild.incomplete || withChild.minimum != ApiStabilityExperimental || !containsDependency(withChild, "E") {
		t.Fatalf("TaggedRootWithChild should expose E, got min=%v deps=%v", withChild.minimum, apiStabilityDependencyNames(withChild))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, withChild, "TaggedRootWithChild")

	leaky := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "taggedLeaky"))
	if leaky.incomplete || leaky.minimum != ApiStabilityExperimental || !containsDependency(leaky, "E") {
		t.Fatalf("taggedLeaky should expose E, got min=%v deps=%v", leaky.minimum, apiStabilityDependencyNames(leaky))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, leaky, "taggedLeaky")

	aliasLeaky := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "TaggedAliasLeaky"))
	if aliasLeaky.incomplete || aliasLeaky.minimum != ApiStabilityExperimental || !containsDependency(aliasLeaky, "E") {
		t.Fatalf("TaggedAliasLeaky should expose E, got min=%v deps=%v", aliasLeaky.minimum, apiStabilityDependencyNames(aliasLeaky))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, aliasLeaky, "TaggedAliasLeaky")

	aliasType := session.UsedByType(c.GetDeclaredTypeOfSymbol(apiStabilityTestExport(t, c, testFile, "TaggedAliasLeaky")))
	if aliasType.incomplete || aliasType.minimum != ApiStabilityExperimental || !containsDependency(aliasType, "E") {
		t.Fatalf("TaggedAliasLeaky type should expose E, got min=%v deps=%v", aliasType.minimum, apiStabilityDependencyNames(aliasType))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, aliasType, "TaggedAliasLeaky")
}

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
		if used.incomplete || used.minimum != ApiStabilityStable || len(used.dependencies) != 0 {
			t.Fatalf("%s: Shared as a root must exclude its own tag, got min=%v deps=%v", label, used.minimum, apiStabilityDependencyNames(used))
		}
		surface, ok := session.analysis.sessionSymbols[shared]
		if !ok || len(surface.findings) == 0 {
			t.Fatalf("%s: the stored surface must keep the own tag for child reuse", label)
		}
	}

	checkHolder := func(t *testing.T, session *ApiStabilitySession, label string) {
		t.Helper()
		used := session.UsedBySymbol(holder)
		if used.incomplete || used.minimum != ApiStabilityExperimental || !containsDependency(used, "Shared") {
			t.Fatalf("%s: Holder should expose Shared as a child, got min=%v deps=%v", label, used.minimum, apiStabilityDependencyNames(used))
		}
		surface, ok := session.analysis.sessionSymbols[holder]
		if !ok || len(surface.findings) == 0 {
			t.Fatalf("%s: Holder's stored surface must contain the child tag", label)
		}
	}

	childLast := tp.NewApiStabilitySession()
	checkShared(t, childLast, "shared-first")
	checkHolder(t, childLast, "shared-first")
	checkShared(t, childLast, "shared-first-repeat")

	rootLast := tp.NewApiStabilitySession()
	checkHolder(t, rootLast, "child-first")
	checkShared(t, rootLast, "child-first")
	checkHolder(t, rootLast, "child-first-repeat")
}

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
	if concreteUsed.incomplete || concreteUsed.minimum != ApiStabilityStable || len(concreteUsed.dependencies) != 0 {
		t.Fatalf("Concrete as a root must exclude its own tag, got min=%v deps=%v", concreteUsed.minimum, apiStabilityDependencyNames(concreteUsed))
	}
	concreteType := session.UsedByType(c.GetDeclaredTypeOfSymbol(concrete))
	if concreteType.incomplete || concreteType.minimum != ApiStabilityStable || len(concreteType.dependencies) != 0 {
		t.Fatalf("Concrete's type must exclude its own tag, got min=%v deps=%v", concreteType.minimum, apiStabilityDependencyNames(concreteType))
	}

	plain := apiStabilityTestExport(t, c, testFile, "AliasToConcrete")
	plainUsed := session.UsedBySymbol(plain)
	if plainUsed.incomplete || plainUsed.minimum != ApiStabilityUnstable || !containsDependency(plainUsed, "Concrete") {
		t.Fatalf("AliasToConcrete should expose Concrete as a child, got min=%v deps=%v", plainUsed.minimum, apiStabilityDependencyNames(plainUsed))
	}

	taggedAlias := apiStabilityTestExport(t, c, testFile, "TaggedAliasToConcrete")
	taggedAliasUsed := session.UsedBySymbol(taggedAlias)
	if taggedAliasUsed.incomplete || taggedAliasUsed.minimum != ApiStabilityUnstable || !containsDependency(taggedAliasUsed, "Concrete") {
		t.Fatalf("TaggedAliasToConcrete should exclude only its own tag and expose Concrete, got min=%v deps=%v", taggedAliasUsed.minimum, apiStabilityDependencyNames(taggedAliasUsed))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, taggedAliasUsed, "TaggedAliasToConcrete")

	// The compiler represents `TaggedAliasToConcrete` as `Concrete`'s own type,
	// so the type-level query sees `Concrete` as the root identity and excludes
	// it. The alias symbol is not part of the type object.
	taggedAliasType := session.UsedByType(c.GetDeclaredTypeOfSymbol(taggedAlias))
	if taggedAliasType.incomplete || taggedAliasType.minimum != ApiStabilityStable || len(taggedAliasType.dependencies) != 0 {
		t.Fatalf("TaggedAliasToConcrete's represented type excludes Concrete's own tag, got min=%v deps=%v", taggedAliasType.minimum, apiStabilityDependencyNames(taggedAliasType))
	}
}

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
	if declared.level != ApiStabilityUnstable {
		t.Fatalf("mixed declared = %v, want unstable (the first tagged own declaration)", declared.level)
	}
	signatures := apiStabilityTestSignatures(t, c, mixed)
	if len(signatures) != 2 {
		t.Fatalf("expected two public overloads, got %d", len(signatures))
	}
	if tp.DeclaredApiStabilityOfSignature(signatures[0]).level != ApiStabilityUnstable ||
		tp.DeclaredApiStabilityOfSignature(signatures[1]).level != ApiStabilityExperimental {
		t.Fatalf("overload tags collapsed: %v/%v",
			tp.DeclaredApiStabilityOfSignature(signatures[0]).level,
			tp.DeclaredApiStabilityOfSignature(signatures[1]).level)
	}

	mixedUsed := session.UsedBySymbol(mixed)
	if mixedUsed.incomplete || mixedUsed.minimum != ApiStabilityExperimental {
		t.Fatalf("mixed surface = %v/%v, want experimental from the other overload", mixedUsed.minimum, apiStabilityDependencyNames(mixedUsed))
	}
	foundExperimental := false
	for _, dependency := range mixedUsed.dependencies {
		if dependency.signature != nil && dependency.level == ApiStabilityExperimental {
			foundExperimental = true
		}
		if dependency.signature != nil && rawSignature(dependency.signature) == rawSignature(signatures[0]) {
			t.Fatalf("the declared own overload leaked into the surface: %v", apiStabilityDependencyNames(mixedUsed))
		}
	}
	if !foundExperimental {
		t.Fatalf("the experimental overload should stay an exposed child, got %v", apiStabilityDependencyNames(mixedUsed))
	}

	for index, signature := range signatures {
		used := session.UsedBySignature(signature)
		if used.incomplete || used.minimum != ApiStabilityStable || len(used.dependencies) != 0 {
			t.Fatalf("overload %d own tag must be excluded, got min=%v deps=%v", index, used.minimum, apiStabilityDependencyNames(used))
		}
	}

	other := apiStabilityTestExport(t, c, testFile, "other")
	if declared := tp.DeclaredApiStabilityOfSymbol(other); declared.level != ApiStabilityExperimental {
		t.Fatalf("other declared = %v, want experimental", declared.level)
	}
	otherUsed := session.UsedBySymbol(other)
	if otherUsed.incomplete || otherUsed.minimum != ApiStabilityStable || len(otherUsed.dependencies) != 0 {
		t.Fatalf("other's only tag is its own declared tag, got min=%v deps=%v", otherUsed.minimum, apiStabilityDependencyNames(otherUsed))
	}
}

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

	stableBox := apiStabilityTestExport(t, c, testFile, "StableBox")
	stableDeclared := tp.DeclaredApiStabilityOfSymbol(stableBox)
	if stableDeclared.level != ApiStabilityStable || stableDeclared.declaration == nil {
		t.Fatalf("explicit stable child lost its provenance: %+v", stableDeclared)
	}
	plainBox := apiStabilityTestExport(t, c, testFile, "PlainBox")
	plainDeclared := tp.DeclaredApiStabilityOfSymbol(plainBox)
	if plainDeclared.level != ApiStabilityStable || plainDeclared.declaration != nil {
		t.Fatalf("absent tag should be stable without provenance: %+v", plainDeclared)
	}

	stableBoxRoot := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "RootWithStableBox"))
	if stableBoxRoot.incomplete || stableBoxRoot.minimum != ApiStabilityExperimental || !containsDependency(stableBoxRoot, "E") {
		t.Fatalf("RootWithStableBox should expose the StableBox type argument E, got min=%v deps=%v", stableBoxRoot.minimum, apiStabilityDependencyNames(stableBoxRoot))
	}
	plainBoxRoot := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "RootWithPlainBox"))
	if plainBoxRoot.incomplete || plainBoxRoot.minimum != ApiStabilityExperimental || !containsDependency(plainBoxRoot, "E") {
		t.Fatalf("RootWithPlainBox should expose the PlainBox type argument E, got min=%v deps=%v", plainBoxRoot.minimum, apiStabilityDependencyNames(plainBoxRoot))
	}

	stableHidden := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "StableHidden"))
	if stableHidden.incomplete || stableHidden.minimum != ApiStabilityExperimental || !containsDependency(stableHidden, "E") {
		t.Fatalf("StableHidden as a root should expand and expose E, got min=%v deps=%v", stableHidden.minimum, apiStabilityDependencyNames(stableHidden))
	}

	stableHiddenRoot := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "RootWithStableHidden"))
	if stableHiddenRoot.incomplete || len(stableHiddenRoot.dependencies) != 0 {
		t.Fatalf("RootWithStableHidden must not expand the nested component, got min=%v deps=%v", stableHiddenRoot.minimum, apiStabilityDependencyNames(stableHiddenRoot))
	}
	plainHiddenRoot := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "RootWithPlainHidden"))
	if plainHiddenRoot.incomplete || len(plainHiddenRoot.dependencies) != 0 {
		t.Fatalf("RootWithPlainHidden must not expand the nested component, got min=%v deps=%v", plainHiddenRoot.minimum, apiStabilityDependencyNames(plainHiddenRoot))
	}
}

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
	if inferred.incomplete || inferred.minimum != ApiStabilityExperimental || !containsDependency(inferred, "E") {
		t.Fatalf("an inferred nested anonymous return should still expose E, got min=%v deps=%v", inferred.minimum, apiStabilityDependencyNames(inferred))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, inferred, "taggedInferred")

	clean := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "cleanInferred"))
	if clean.incomplete || len(clean.dependencies) != 0 {
		t.Fatalf("cleanInferred must stay clean, got min=%v deps=%v", clean.minimum, apiStabilityDependencyNames(clean))
	}

	generic := apiStabilityTestExport(t, c, testFile, "taggedGeneric")
	genericUsed := session.UsedBySymbol(generic)
	if genericUsed.incomplete || genericUsed.minimum != ApiStabilityExperimental || !containsDependency(genericUsed, "E") {
		t.Fatalf("a tagged generic function should still expose its constraint, got min=%v deps=%v", genericUsed.minimum, apiStabilityDependencyNames(genericUsed))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, genericUsed, "taggedGeneric")
	signatureUsed := session.UsedBySignature(apiStabilityTestSignatures(t, c, generic)[0])
	if signatureUsed.incomplete || signatureUsed.minimum != ApiStabilityExperimental || !containsDependency(signatureUsed, "E") {
		t.Fatalf("a tagged signature should still expose its constraint, got min=%v deps=%v", signatureUsed.minimum, apiStabilityDependencyNames(signatureUsed))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, signatureUsed, "taggedGeneric")

	constrained := session.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "TaggedConstrained"))
	if constrained.incomplete || constrained.minimum != ApiStabilityExperimental || !containsDependency(constrained, "E") {
		t.Fatalf("a tagged generic interface should still expose its constraint, got min=%v deps=%v", constrained.minimum, apiStabilityDependencyNames(constrained))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, constrained, "TaggedConstrained")
}

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

	callableHolder := surface("CallableHolder")
	if callableHolder.incomplete || callableHolder.minimum != ApiStabilityExperimental || !containsDependency(callableHolder, "E") {
		t.Fatalf("CallableHolder should expose E through its untagged callable, got min=%v deps=%v", callableHolder.minimum, apiStabilityDependencyNames(callableHolder))
	}

	anonymousWrapper := surface("AnonymousWrapper")
	if anonymousWrapper.incomplete || containsDependency(anonymousWrapper, "E") {
		t.Fatalf("a tagged callable must stay bounded through an anonymous wrapper, got min=%v deps=%v", anonymousWrapper.minimum, apiStabilityDependencyNames(anonymousWrapper))
	}

	hiddenHolder := surface("HiddenHolder")
	if hiddenHolder.incomplete || len(hiddenHolder.dependencies) != 0 {
		t.Fatalf("a tagged hidden member and an untagged shallow member must both stay quiet, got min=%v deps=%v", hiddenHolder.minimum, apiStabilityDependencyNames(hiddenHolder))
	}

	taggedHidden := surface("TaggedHidden")
	if taggedHidden.incomplete || taggedHidden.minimum != ApiStabilityExperimental || !containsDependency(taggedHidden, "E") {
		t.Fatalf("TaggedHidden as a root should expand and expose E, got min=%v deps=%v", taggedHidden.minimum, apiStabilityDependencyNames(taggedHidden))
	}

	unstableCallable := surface("UnstableCallableHolder")
	if unstableCallable.incomplete || unstableCallable.minimum != ApiStabilityUnstable || containsDependency(unstableCallable, "E") {
		t.Fatalf("UnstableCallableHolder should expose only the unstable child tag, got min=%v deps=%v", unstableCallable.minimum, apiStabilityDependencyNames(unstableCallable))
	}
	if !containsDependency(unstableCallable, "UnstableCallable") {
		t.Fatalf("UnstableCallableHolder should expose UnstableCallable, got %v", apiStabilityDependencyNames(unstableCallable))
	}
	unstableCallableRoot := surface("UnstableCallable")
	if unstableCallableRoot.incomplete || unstableCallableRoot.minimum != ApiStabilityExperimental || !containsDependency(unstableCallableRoot, "E") {
		t.Fatalf("UnstableCallable as a root should expand and expose E, got min=%v deps=%v", unstableCallableRoot.minimum, apiStabilityDependencyNames(unstableCallableRoot))
	}

	taggedHeritage := surface("TaggedHeritage")
	if taggedHeritage.incomplete || len(taggedHeritage.dependencies) != 0 {
		t.Fatalf("TaggedHeritage must not expose the tagged base internals, got min=%v deps=%v", taggedHeritage.minimum, apiStabilityDependencyNames(taggedHeritage))
	}
	plainHeritage := surface("PlainHeritage")
	if plainHeritage.incomplete || plainHeritage.minimum != ApiStabilityExperimental || !containsDependency(plainHeritage, "E") {
		t.Fatalf("PlainHeritage should expand its untagged base and expose E, got min=%v deps=%v", plainHeritage.minimum, apiStabilityDependencyNames(plainHeritage))
	}
	unstableHeritage := surface("UnstableHeritage")
	if unstableHeritage.incomplete || unstableHeritage.minimum != ApiStabilityUnstable || containsDependency(unstableHeritage, "E") {
		t.Fatalf("UnstableHeritage should expose only the unstable base tag, got min=%v deps=%v", unstableHeritage.minimum, apiStabilityDependencyNames(unstableHeritage))
	}
	if !containsDependency(unstableHeritage, "UnstableBase") {
		t.Fatalf("UnstableHeritage should expose UnstableBase, got %v", apiStabilityDependencyNames(unstableHeritage))
	}

	stableArgument := surface("TaggedBoxStableArgument")
	if stableArgument.incomplete || len(stableArgument.dependencies) != 0 {
		t.Fatalf("TaggedBox<string> must not expose the tagged base internals, got min=%v deps=%v", stableArgument.minimum, apiStabilityDependencyNames(stableArgument))
	}
	exposedArgument := surface("TaggedBoxExposedArgument")
	if exposedArgument.incomplete || exposedArgument.minimum != ApiStabilityExperimental || !containsDependency(exposedArgument, "E") {
		t.Fatalf("TaggedBox<E> must keep the independently represented argument exposed, got min=%v deps=%v", exposedArgument.minimum, apiStabilityDependencyNames(exposedArgument))
	}

	deepChain := surface("DeepChain")
	if deepChain.incomplete || deepChain.minimum != ApiStabilityExperimental || !containsDependency(deepChain, "E") {
		t.Fatalf("DeepChain should expose the tagged ancestor argument through the untagged intermediate, got min=%v deps=%v", deepChain.minimum, apiStabilityDependencyNames(deepChain))
	}

	deepRefIdent := findIdentifierByText(t, testFile, "DeepChainRef", 1)
	if deepRefIdent == nil || deepRefIdent.Parent == nil {
		t.Fatal("DeepChainRef annotation not found")
	}
	deepRefType := tp.GetTypeAtLocation(deepRefIdent.Parent)
	deepRef := session.UsedByType(deepRefType)
	if deepRef.incomplete || deepRef.minimum != ApiStabilityExperimental || !containsDependency(deepRef, "E") {
		t.Fatalf("the DeepChainRef<E> reference root should expose E, got min=%v deps=%v", deepRef.minimum, apiStabilityDependencyNames(deepRef))
	}

	aliasBox := surface("TaggedAliasBoxHolder")
	if aliasBox.incomplete || aliasBox.minimum != ApiStabilityExperimental || !containsDependency(aliasBox, "E") {
		t.Fatalf("a tagged alias application must keep its argument exposed, got min=%v deps=%v", aliasBox.minimum, apiStabilityDependencyNames(aliasBox))
	}

	memberTagBoundary := surface("MemberTagBoundary")
	if memberTagBoundary.incomplete || len(memberTagBoundary.dependencies) != 0 {
		t.Fatalf("tagged member and tagged call signature must both stay bounded, got min=%v deps=%v", memberTagBoundary.minimum, apiStabilityDependencyNames(memberTagBoundary))
	}
	memberTagControl := surface("MemberTagControl")
	if memberTagControl.incomplete || memberTagControl.minimum != ApiStabilityExperimental || !containsDependency(memberTagControl, "E") {
		t.Fatalf("MemberTagControl should keep the anonymous walk and expose E, got min=%v deps=%v", memberTagControl.minimum, apiStabilityDependencyNames(memberTagControl))
	}

	indexTagBoundary := surface("IndexTagBoundary")
	if indexTagBoundary.incomplete || containsDependency(indexTagBoundary, "E") {
		t.Fatalf("a tagged index signature must not expose its value internals, got min=%v deps=%v", indexTagBoundary.minimum, apiStabilityDependencyNames(indexTagBoundary))
	}
	indexTagControl := surface("IndexTagControl")
	if indexTagControl.incomplete || indexTagControl.minimum != ApiStabilityExperimental || !containsDependency(indexTagControl, "E") {
		t.Fatalf("IndexTagControl should expose the index value type, got min=%v deps=%v", indexTagControl.minimum, apiStabilityDependencyNames(indexTagControl))
	}

	sharedHolder := surface("SharedBoundaryHolder")
	if sharedHolder.incomplete || sharedHolder.minimum != ApiStabilityUnstable || containsDependency(sharedHolder, "E") {
		t.Fatalf("SharedBoundaryHolder should expose only the child tag, got min=%v deps=%v", sharedHolder.minimum, apiStabilityDependencyNames(sharedHolder))
	}
	if !containsDependency(sharedHolder, "SharedBoundary") {
		t.Fatalf("SharedBoundaryHolder should expose SharedBoundary, got %v", apiStabilityDependencyNames(sharedHolder))
	}
	sharedRoot := surface("SharedBoundary")
	if sharedRoot.incomplete || sharedRoot.minimum != ApiStabilityExperimental || !containsDependency(sharedRoot, "E") {
		t.Fatalf("SharedBoundary as a root should still expand and expose E, got min=%v deps=%v", sharedRoot.minimum, apiStabilityDependencyNames(sharedRoot))
	}

	heritageOverride := surface("HeritageOverride")
	if heritageOverride.incomplete || heritageOverride.minimum != ApiStabilityExperimental || !containsDependency(heritageOverride, "E") {
		t.Fatalf("an own override must not be suppressed by a tagged base of the same name, got min=%v deps=%v", heritageOverride.minimum, apiStabilityDependencyNames(heritageOverride))
	}

	blockedHeritage := surface("BlockedHeritage")
	if !blockedHeritage.incomplete {
		t.Fatalf("a tagged base whose required argument cannot be established must stay incomplete, got min=%v deps=%v", blockedHeritage.minimum, apiStabilityDependencyNames(blockedHeritage))
	}
	blockedMember := surface("BlockedMember")
	if !blockedMember.incomplete {
		t.Fatalf("a tagged member whose required argument cannot be established must stay incomplete, got min=%v deps=%v", blockedMember.minimum, apiStabilityDependencyNames(blockedMember))
	}
}

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
			t.Fatalf("%s must not expose %s, got incomplete=%v min=%v deps=%v", label, dependency, used.incomplete, used.minimum, apiStabilityDependencyNames(used))
		}
	}
	assertExposed := func(label string, used ApiStabilityUsed, dependency string) {
		t.Helper()
		if used.incomplete || !containsDependency(used, dependency) {
			t.Fatalf("%s should expose %s, got incomplete=%v min=%v deps=%v", label, dependency, used.incomplete, used.minimum, apiStabilityDependencyNames(used))
		}
	}

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

	derivedCallable := surface("FxDerivedCallable")
	if derivedCallable.incomplete || derivedCallable.minimum != ApiStabilityUnstable || containsDependency(derivedCallable, "FxCallE") {
		t.Fatalf("FxDerivedCallable must expose only the tagged base, got min=%v deps=%v", derivedCallable.minimum, apiStabilityDependencyNames(derivedCallable))
	}
	if !containsDependency(derivedCallable, "FxTaggedCallableBase") {
		t.Fatalf("FxDerivedCallable should expose FxTaggedCallableBase, got %v", apiStabilityDependencyNames(derivedCallable))
	}
	derivedCallableGeneric := surface("FxDerivedCallableGeneric")
	if derivedCallableGeneric.incomplete || derivedCallableGeneric.minimum != ApiStabilityUnstable || containsDependency(derivedCallableGeneric, "FxCallE") {
		t.Fatalf("FxDerivedCallableGeneric must expose only the tagged base, got min=%v deps=%v", derivedCallableGeneric.minimum, apiStabilityDependencyNames(derivedCallableGeneric))
	}
	callRefType := referenceSurface("fxCallRef")
	if callRefType.incomplete || callRefType.minimum != ApiStabilityUnstable || containsDependency(callRefType, "FxCallE") {
		t.Fatalf("fxCallRef type root must expose only the tagged base, got min=%v deps=%v", callRefType.minimum, apiStabilityDependencyNames(callRefType))
	}
	if !containsDependency(callRefType, "FxTaggedCallableBase") {
		t.Fatalf("fxCallRef type root should expose FxTaggedCallableBase, got %v", apiStabilityDependencyNames(callRefType))
	}
	callRefSymbol := surface("fxCallRef")
	if callRefSymbol.incomplete || callRefSymbol.minimum != ApiStabilityUnstable || containsDependency(callRefSymbol, "FxCallE") {
		t.Fatalf("fxCallRef value root must expose only the tagged base, got min=%v deps=%v", callRefSymbol.minimum, apiStabilityDependencyNames(callRefSymbol))
	}
	assertExposed("fxPlainCallRef untagged control", referenceSurface("fxPlainCallRef"), "FxCallE")

	for _, label := range []string{"combo type root", "combo value root"} {
		var used ApiStabilityUsed
		if label == "combo type root" {
			used = referenceSurface("fxComboRef")
		} else {
			used = surface("fxComboRef")
		}
		if used.incomplete || used.minimum != ApiStabilityUnstable || !containsDependency(used, "FxTaggedCombo") {
			t.Fatalf("%s must expose only FxTaggedCombo, got min=%v deps=%v", label, used.minimum, apiStabilityDependencyNames(used))
		}
		for _, dependency := range []string{"FxCallE", "FxCtorE", "FxIdxE", "FxMethE", "FxPropE"} {
			assertQuiet(label, used, dependency)
		}
	}

	derivedIndex := surface("FxDerivedIndex")
	if derivedIndex.incomplete || derivedIndex.minimum != ApiStabilityUnstable || containsDependency(derivedIndex, "FxIdxE") {
		t.Fatalf("FxDerivedIndex must expose only the tagged base, got min=%v deps=%v", derivedIndex.minimum, apiStabilityDependencyNames(derivedIndex))
	}
	indexRefType := referenceSurface("fxIndexRef")
	if indexRefType.incomplete || indexRefType.minimum != ApiStabilityUnstable || containsDependency(indexRefType, "FxIdxE") {
		t.Fatalf("fxIndexRef type root must expose only the tagged base, got min=%v deps=%v", indexRefType.minimum, apiStabilityDependencyNames(indexRefType))
	}
	assertQuiet("fxIndexRef value root", surface("fxIndexRef"), "FxIdxE")
	assertExposed("fxPlainIndexRef untagged control", referenceSurface("fxPlainIndexRef"), "FxIdxE")

	deepCallRef := referenceSurface("fxDeepCallRef")
	if deepCallRef.incomplete || containsDependency(deepCallRef, "FxCallE") {
		t.Fatalf("fxDeepCallRef must not expose the tagged ancestor's call internals, got min=%v deps=%v", deepCallRef.minimum, apiStabilityDependencyNames(deepCallRef))
	}
	if !containsDependency(deepCallRef, "FxTaggedDeepCallable") {
		t.Fatalf("fxDeepCallRef should expose FxTaggedDeepCallable, got %v", apiStabilityDependencyNames(deepCallRef))
	}

	derivedClass := surface("FxDerivedClass")
	if derivedClass.incomplete || derivedClass.minimum != ApiStabilityUnstable {
		t.Fatalf("FxDerivedClass must expose the tagged base level, got min=%v deps=%v", derivedClass.minimum, apiStabilityDependencyNames(derivedClass))
	}
	for _, dependency := range []string{"FxCtorE", "FxMethE", "FxPropE"} {
		assertQuiet("FxDerivedClass", derivedClass, dependency)
	}
	derivedClassStatic := warm.UsedByType(c.GetTypeOfSymbol(apiStabilityTestExport(t, c, testFile, "FxDerivedClass")))
	assertQuiet("FxDerivedClass static type root", derivedClassStatic, "FxCtorE")
	assertExposed("FxPlainDerivedClass untagged control", surface("FxPlainDerivedClass"), "FxCtorE")

	ownCtor := surface("FxDerivedOwnCtorClass")
	assertExposed("FxDerivedOwnCtorClass own constructor", ownCtor, "E")
	assertQuiet("FxDerivedOwnCtorClass", ownCtor, "FxCtorE")

	derivedNoCtor := surface("FxDerivedNoCtorClass")
	if derivedNoCtor.incomplete || derivedNoCtor.minimum != ApiStabilityUnstable || !containsDependency(derivedNoCtor, "FxTaggedNoCtorClass") {
		t.Fatalf("FxDerivedNoCtorClass must expose only the tagged base level, got incomplete=%v min=%v deps=%v", derivedNoCtor.incomplete, derivedNoCtor.minimum, apiStabilityDependencyNames(derivedNoCtor))
	}
	assertQuiet("FxDerivedNoCtorClass", derivedNoCtor, "FxMethE")
	assertQuiet("FxDerivedNoCtorClass static type root", warm.UsedByType(c.GetTypeOfSymbol(apiStabilityTestExport(t, c, testFile, "FxDerivedNoCtorClass"))), "FxMethE")
	assertQuiet("cold FxDerivedNoCtorClass", cold.UsedBySymbol(apiStabilityTestExport(t, c, testFile, "FxDerivedNoCtorClass")), "FxMethE")

	stableCallable := surface("FxDerivedStableCallable")
	if stableCallable.incomplete || len(stableCallable.dependencies) != 0 {
		t.Fatalf("FxDerivedStableCallable must stay complete and quiet, got min=%v deps=%v", stableCallable.minimum, apiStabilityDependencyNames(stableCallable))
	}

	ownOverride := surface("FxOwnOverrideDerived")
	assertExposed("FxOwnOverrideDerived own signatures", ownOverride, "E")
	assertQuiet("FxOwnOverrideDerived", ownOverride, "FxCallE")
	assertQuiet("FxOwnOverrideDerived", ownOverride, "FxIdxE")

	callableBoxRef := referenceSurface("fxCallableBoxRef")
	assertExposed("fxCallableBoxRef tagged ancestor argument", callableBoxRef, "FxArgE")
	assertQuiet("fxCallableBoxRef", callableBoxRef, "FxPropE")
	if !containsDependency(callableBoxRef, "FxTaggedCallableBox") {
		t.Fatalf("fxCallableBoxRef should expose FxTaggedCallableBox, got %v", apiStabilityDependencyNames(callableBoxRef))
	}

	mixedCall := surface("FxMixedCallInheritance")
	assertQuiet("FxMixedCallInheritance", mixedCall, "FxCallE")
	if !containsDependency(mixedCall, "FxTaggedFirstCall") {
		t.Fatalf("FxMixedCallInheritance should expose FxTaggedFirstCall, got %v", apiStabilityDependencyNames(mixedCall))
	}

	if blocked := surface("FxBlockedDerived"); !blocked.incomplete {
		t.Fatalf("a tagged base with a blocked required argument must stay incomplete, got min=%v deps=%v", blocked.minimum, apiStabilityDependencyNames(blocked))
	}

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

	if globals := c.GetGlobalDiagnostics(); len(globals) != 0 {
		t.Errorf("flattened inheritance queries produced global diagnostics: %v", globals)
	}
}

func apiStabilitySharedTypeEntry(tp *TypeParser, target *checker.Type, inspect apiStabilityInspection) *apiStabilitySharedSurface {
	if tp == nil || tp.links == nil || target == nil {
		return nil
	}
	return tp.links.ApiStabilitySurfaceType.TryGet(apiStabilitySurfaceTypeKey{t: target, inspect: inspect})
}

func apiStabilitySharedSignatureEntry(tp *TypeParser, signature *checker.Signature) *apiStabilitySharedSurface {
	if tp == nil || tp.links == nil || signature == nil {
		return nil
	}
	return tp.links.ApiStabilitySurfaceSignature.TryGet(signature)
}

func apiStabilitySharedEntryHasSymbol(entry *apiStabilitySharedSurface, symbol *ast.Symbol) bool {
	if entry == nil || symbol == nil {
		return false
	}
	_, ok := entry.findings[apiStabilityFindingKey{symbol: symbol}]
	return ok
}

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

	rootType := tp.NewApiStabilitySession().UsedByType(taggedDecl)
	if rootType.incomplete || rootType.minimum != ApiStabilityExperimental || !containsDependency(rootType, "E") {
		t.Fatalf("Tagged type root = %v/%v, want experimental E", rootType.minimum, apiStabilityDependencyNames(rootType))
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

	rootSignature := tp.NewApiStabilitySession().UsedBySignature(signature)
	if rootSignature.incomplete || rootSignature.minimum != ApiStabilityExperimental || !containsDependency(rootSignature, "E") {
		t.Fatalf("taggedFn signature root = %v/%v, want experimental E", rootSignature.minimum, apiStabilityDependencyNames(rootSignature))
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

	reusedType := tp.NewApiStabilitySession().UsedByType(taggedDecl)
	if reusedType.incomplete || reusedType.minimum != ApiStabilityExperimental || len(reusedType.dependencies) != len(rootType.dependencies) {
		t.Fatalf("reused type root disagrees: %v/%v", reusedType.minimum, apiStabilityDependencyNames(reusedType))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, reusedType, "Tagged")
	reusedSignature := tp.NewApiStabilitySession().UsedBySignature(signature)
	if reusedSignature.incomplete || reusedSignature.minimum != ApiStabilityExperimental || len(reusedSignature.dependencies) != len(rootSignature.dependencies) {
		t.Fatalf("reused signature root disagrees: %v/%v", reusedSignature.minimum, apiStabilityDependencyNames(reusedSignature))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, reusedSignature, "taggedFn")

	holder := apiStabilityTestExport(t, c, testFile, "Holder")
	holderUsed := tp.NewApiStabilitySession().UsedBySymbol(holder)
	if holderUsed.incomplete || holderUsed.minimum != ApiStabilityUnstable {
		t.Fatalf("Holder = %v/%v, want unstable from its tagged children", holderUsed.minimum, apiStabilityDependencyNames(holderUsed))
	}
	if !containsDependency(holderUsed, "Tagged") || !containsDependency(holderUsed, "taggedFn") {
		t.Fatalf("Holder should expose both tagged children, got %v", apiStabilityDependencyNames(holderUsed))
	}
	if containsDependency(holderUsed, "E") {
		t.Fatalf("a cached tagged child must not leak its internals, got %v", apiStabilityDependencyNames(holderUsed))
	}
}

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
	if first.incomplete || first.minimum != ApiStabilityExperimental || !containsDependency(first, "E") {
		t.Fatalf("Shared root = %v/%v, want experimental E", first.minimum, apiStabilityDependencyNames(first))
	}
	entry := apiStabilitySharedTypeEntry(tp1, sharedDecl, apiStabilityExpand)
	if entry == nil {
		t.Fatal("the first export must publish the complete Shared surface")
	}
	published := len(entry.findings)

	tp2 := NewTypeParser(c.Program(), c)
	hit := newApiStabilityAnalysis(tp2)
	hit.typeSurface(sharedDecl, apiStabilityExpand, apiStabilitySubstitution{})
	if hit.work != 0 {
		t.Fatalf("a published snapshot hit consumed %d work", hit.work)
	}
	second := tp2.NewApiStabilitySession().UsedByType(sharedDecl)
	if second.minimum != first.minimum || len(second.dependencies) != len(first.dependencies) {
		t.Fatalf("reused root disagrees: first=%v second=%v", apiStabilityDependencyNames(first), apiStabilityDependencyNames(second))
	}

	consumer := apiStabilityTestExport(t, c, files["/.src/consumer.ts"], "Consumer")
	consumerUsed := tp2.NewApiStabilitySession().UsedBySymbol(consumer)
	if consumerUsed.incomplete || consumerUsed.minimum != ApiStabilityUnstable || !containsDependency(consumerUsed, "Shared") {
		t.Fatalf("Consumer = %v/%v, want unstable Shared", consumerUsed.minimum, apiStabilityDependencyNames(consumerUsed))
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
	if alsoUsed.minimum != consumerUsed.minimum || len(alsoUsed.dependencies) != len(consumerUsed.dependencies) {
		t.Fatalf("second consumer disagrees: %v/%v", alsoUsed.minimum, apiStabilityDependencyNames(alsoUsed))
	}

	after := apiStabilitySharedTypeEntry(tp2, sharedDecl, apiStabilityExpand)
	if after == nil || len(after.findings) != published {
		t.Fatalf("the published expand snapshot was mutated: before=%d after=%v", published, after)
	}
	if apiStabilitySharedEntryHasSymbol(after, shared) {
		t.Fatal("the published snapshot gained the own tag after reuse")
	}
}

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
				if childUsed.incomplete || childUsed.minimum != ApiStabilityUnstable || containsDependency(childUsed, "E") {
					t.Fatalf("%s: Child = %v/%v, want only the unstable child", label, childUsed.minimum, apiStabilityDependencyNames(childUsed))
				}
				plainUsed := tp.NewApiStabilitySession().UsedBySymbol(plainChild)
				if plainUsed.incomplete || len(plainUsed.dependencies) != 0 {
					t.Fatalf("%s: PlainChild = %v/%v, want a quiet shallow surface", label, plainUsed.minimum, apiStabilityDependencyNames(plainUsed))
				}
			}
			checkRoots := func(label string) {
				t.Helper()
				root := tp.NewApiStabilitySession().UsedByType(taggedDecl)
				if root.incomplete || root.minimum != ApiStabilityExperimental || !containsDependency(root, "E") {
					t.Fatalf("%s: Tagged root = %v/%v, want experimental E", label, root.minimum, apiStabilityDependencyNames(root))
				}
				assertApiStabilitySurfaceExcludesOwnTag(t, root, "Tagged")
				plainRoot := tp.NewApiStabilitySession().UsedByType(plainDecl)
				if plainRoot.incomplete || plainRoot.minimum != ApiStabilityExperimental || !containsDependency(plainRoot, "E") {
					t.Fatalf("%s: Plain root = %v/%v, want experimental E", label, plainRoot.minimum, apiStabilityDependencyNames(plainRoot))
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
			if !apiStabilitySharedEntryHasSymbol(apiStabilitySharedTypeEntry(tp, plainDecl, apiStabilityExpand), eSymbol) {
				t.Fatal("the shallow composition contaminated the expanded snapshot")
			}
		})
	}
}

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
			if leakyUsed.incomplete || !containsDependency(leakyUsed, "E") {
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
		if stored.incomplete || !containsDependency(stored, "E") {
			t.Fatalf("the substituted raw signature should expose E, got %v", apiStabilityDependencyNames(stored))
		}

		_ = tp.NewApiStabilitySession().UsedBySignature(instantiated)
		if apiStabilitySharedSignatureEntry(tp, raw) != nil {
			t.Fatal("the concrete signature published under the raw signature slot")
		}
		if apiStabilitySharedSignatureEntry(tp, instantiated) == nil {
			t.Fatal("the concrete context-free signature should be published")
		}
	})
}

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
	if first.incomplete || !containsDependency(first, "E") {
		t.Fatalf("Shared root should expose E, got %v", apiStabilityDependencyNames(first))
	}
	entry := apiStabilitySharedTypeEntry(tp, sharedDecl, apiStabilityExpand)
	if entry == nil || !apiStabilitySharedEntryHasSymbol(entry, eSymbol) {
		t.Fatal("the published snapshot should contain the E finding")
	}
	published := len(entry.findings)

	session := tp.NewApiStabilitySession()
	used := session.UsedByType(sharedDecl)
	if used.incomplete || !containsDependency(used, "E") {
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

	foundProvenance := false
	for _, dependency := range first.dependencies {
		if ApiStabilityDependencyName(dependency) == "E" && dependency.declaration != nil {
			foundProvenance = true
		}
	}
	if !foundProvenance {
		t.Fatal("a snapshot finding lost its declaration provenance")
	}
}

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
	if !coldUsed.incomplete {
		t.Fatal("an unmaterialized computed member must report an incomplete result")
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, coldUsed, "Cold")
	if apiStabilitySharedTypeEntry(tp, coldDecl, apiStabilityExpand) != nil {
		t.Fatal("an incomplete cold surface must not be published")
	}

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

	primeApiStabilityChecker(t, c)
	warm := tp.NewApiStabilitySession().UsedBySymbol(cold)
	if warm.incomplete || !containsDependency(warm, "E") {
		t.Fatalf("the materialized retry should expose E, got %v", apiStabilityDependencyNames(warm))
	}
	assertApiStabilitySurfaceExcludesOwnTag(t, warm, "Cold")
	if apiStabilitySharedTypeEntry(tp, coldDecl, apiStabilityExpand) == nil {
		t.Fatal("the complete retry must publish its snapshot")
	}
	reuse := newApiStabilityAnalysis(tp)
	reuse.typeSurface(coldDecl, apiStabilityExpand, apiStabilitySubstitution{})
	if reuse.work != 0 {
		t.Fatalf("the published retry snapshot consumed %d work", reuse.work)
	}
	reused := tp.NewApiStabilitySession().UsedBySymbol(cold)
	if reused.incomplete || !containsDependency(reused, "E") || len(reused.dependencies) != len(warm.dependencies) {
		t.Fatalf("reused surface disagrees: %v/%v", reused.minimum, apiStabilityDependencyNames(reused))
	}
}

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
			if !coldB.incomplete {
				t.Fatal("the cold computed member must keep ColdB incomplete")
			}
			if apiStabilitySharedTypeEntry(tp, bDecl, apiStabilityExpand) != nil {
				t.Fatal("an incomplete cold cycle surface must not be published")
			}
			if coldA.incomplete || containsDependency(coldA, "E") {
				t.Fatalf("ColdA = %v/%v, want a complete shallow surface", coldA.minimum, apiStabilityDependencyNames(coldA))
			}

			primeApiStabilityChecker(t, c)
			warmB := tp.NewApiStabilitySession().UsedBySymbol(b)
			if warmB.incomplete || warmB.minimum != ApiStabilityExperimental || !containsDependency(warmB, "E") {
				t.Fatalf("warm ColdB = %v/%v, want experimental E", warmB.minimum, apiStabilityDependencyNames(warmB))
			}
			if apiStabilitySharedTypeEntry(tp, bDecl, apiStabilityExpand) == nil {
				t.Fatal("the warm complete cycle must publish its snapshot")
			}
			warmA := tp.NewApiStabilitySession().UsedBySymbol(a)
			if warmA.incomplete || containsDependency(warmA, "E") {
				t.Fatalf("warm ColdA = %v/%v, want only the shallow surface", warmA.minimum, apiStabilityDependencyNames(warmA))
			}
			reuse := newApiStabilityAnalysis(tp)
			reuse.typeSurface(bDecl, apiStabilityExpand, apiStabilitySubstitution{})
			if reuse.work != 0 {
				t.Fatalf("the published cycle snapshot consumed %d work", reuse.work)
			}
		})
	}
}

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
	if first.incomplete || first.minimum != ApiStabilityExperimental {
		t.Fatalf("mixed = %v/%v, want experimental from the sibling overload", first.minimum, apiStabilityDependencyNames(first))
	}
	for _, dependency := range first.dependencies {
		if dependency.signature != nil && rawSignature(dependency.signature) == rawSignature(signatures[0]) {
			t.Fatalf("the declared own overload leaked into the surface: %v", apiStabilityDependencyNames(first))
		}
	}
	if apiStabilitySharedSignatureEntry(tp, signatures[0]) == nil || apiStabilitySharedSignatureEntry(tp, signatures[1]) == nil {
		t.Fatal("both overload signature surfaces should be published")
	}
	for index, signature := range signatures {
		if !apiStabilitySharedEntryRemovedSignature(apiStabilitySharedSignatureEntry(tp, signature), signature) {
			t.Fatalf("overload %d own tag must be removed from its snapshot", index)
		}
	}

	reused := tp.NewApiStabilitySession().UsedBySymbol(mixed)
	if reused.incomplete || reused.minimum != first.minimum || len(reused.dependencies) != len(first.dependencies) {
		t.Fatalf("reused mixed = %v/%v, want %v", reused.minimum, apiStabilityDependencyNames(reused), apiStabilityDependencyNames(first))
	}
	for index, signature := range signatures {
		used := tp.NewApiStabilitySession().UsedBySignature(signature)
		if used.incomplete || used.minimum != ApiStabilityStable || len(used.dependencies) != 0 {
			t.Fatalf("reused overload %d own tag must be excluded, got min=%v deps=%v", index, used.minimum, apiStabilityDependencyNames(used))
		}
	}
}

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
	if coldUsed.incomplete || !containsDependency(coldUsed, "E") {
		t.Fatalf("the cold surface should lazily materialize E, got %v", apiStabilityDependencyNames(coldUsed))
	}
	coldType := cold.GetTypeOfSymbol(coldSymbol)
	if apiStabilitySharedTypeEntry(tpCold, coldType, apiStabilityExpand) == nil {
		t.Fatal("the cold complete surface should be published")
	}

	primeApiStabilityChecker(t, cold)
	reused := NewTypeParser(cold.Program(), cold).NewApiStabilitySession().UsedBySymbol(coldSymbol)
	if reused.incomplete {
		t.Fatal("the reused snapshot must stay complete")
	}

	warm, tpWarm, warmFiles, warmDone := compileAndGetCheckerAndSourceFilesInternal(t, map[string]string{"/.src/test.ts": source})
	defer warmDone()
	primeApiStabilityChecker(t, warm)
	warmSymbol := apiStabilityTestExport(t, warm, warmFiles["/.src/test.ts"], "f")
	pure := tpWarm.NewApiStabilitySession().UsedBySymbol(warmSymbol)
	if pure.incomplete {
		t.Fatal("the pure warm recomputation must be complete")
	}

	if reused.minimum != pure.minimum {
		t.Fatalf("reused minimum %v disagrees with pure warm %v", reused.minimum, pure.minimum)
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
	if first.incomplete || first.minimum != ApiStabilityExperimental || !containsDependency(first, "E") {
		t.Fatalf("union root = %v/%v, want experimental E", first.minimum, apiStabilityDependencyNames(first))
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
	if reused.incomplete || reused.minimum != first.minimum || len(reused.dependencies) != len(first.dependencies) {
		t.Fatalf("reused union = %v/%v, want %v", reused.minimum, apiStabilityDependencyNames(reused), apiStabilityDependencyNames(first))
	}
	direct := newApiStabilityAnalysis(tp)
	direct.typeSurface(union, apiStabilityExpand, apiStabilitySubstitution{})
	if direct.work != 0 {
		t.Fatalf("the symbol-less snapshot hit consumed %d work", direct.work)
	}
}
