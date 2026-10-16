package typeparser

import (
	"reflect"
	"slices"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
)

// apiStabilityAnalysisDiff names the fields of two analyses that differ,
// treating a nil and an empty map or slice as equal. Only one side may hold a
// non-empty collection, so it compares a used analysis against a fresh one. A
// field of a kind it does not know fails the test, so a new field forces a
// decision about reset.
func apiStabilityAnalysisDiff(t *testing.T, got, want *apiStabilityAnalysis) []string {
	t.Helper()
	var diff []string
	gotValue, wantValue := reflect.ValueOf(got).Elem(), reflect.ValueOf(want).Elem()
	for i := range gotValue.NumField() {
		name := gotValue.Type().Field(i).Name
		if !apiStabilityValuesEqual(t, name, gotValue.Field(i), wantValue.Field(i)) {
			diff = append(diff, name)
		}
	}
	return diff
}

func apiStabilityValuesEqual(t *testing.T, path string, got, want reflect.Value) bool {
	t.Helper()
	switch got.Kind() {
	case reflect.Map, reflect.Slice:
		if got.Len() != 0 && want.Len() != 0 {
			t.Fatalf("%s: both sides are non-empty", path)
		}
		return got.Len() == 0 && want.Len() == 0
	case reflect.Struct:
		for i := range got.NumField() {
			if !apiStabilityValuesEqual(t, path+"."+got.Type().Field(i).Name, got.Field(i), want.Field(i)) {
				return false
			}
		}
		return true
	case reflect.Pointer:
		return got.Pointer() == want.Pointer()
	case reflect.Int:
		return got.Int() == want.Int()
	default:
		t.Fatalf("%s: unsupported kind %s", path, got.Kind())
		return false
	}
}

const apiStabilityResetSource = `/** @stability experimental */
interface E { value: string }
/** @stability unstable */
interface U { value: string }
export interface Box<A> { value: A; map<B>(f: (a: A) => B): Box<B> }
export interface Node { next: Node; payload: E }
export type Pick2<A, K extends keyof A> = { [P in K]: A[P] }
export type IfE<A> = A extends E ? E : U
export declare const make: <A>(a: A) => Box<A>
export declare const boxed: Box<E>
export declare class Service extends Box<U> { run(): IfE<string> }
export interface Leaky { e: E }
export interface Other { u: U }
/** @stability unstable */
interface Reportable {
/** @stability experimental */ test?: string }
declare global { interface Error extends Reportable {} }
export function augmented(e: Error) {}
`

// TestApiStabilityResetMatchesFreshAnalysis proves a reset analysis is
// indistinguishable from a new one after a query that fills its tables.
func TestApiStabilityResetMatchesFreshAnalysis(t *testing.T) {
	t.Parallel()

	c, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, apiStabilityResetSource)
	defer done()
	primeApiStabilityChecker(t, c)

	session := tp.NewApiStabilitySession()
	for _, name := range []string{"Box", "Node", "Pick2", "IfE", "make", "boxed", "Service", "augmented"} {
		session.UsedBySymbol(apiStabilityTestExport(t, c, sf, name))
	}

	dirty := apiStabilityAnalysisDiff(t, session.analysis, newApiStabilityAnalysis(tp))
	for _, field := range []string{
		"sessionSymbols", "sessionTypes", "sessionComponents", "sessionSignatures",
		"typeFrames", "signatureFrames", "carriers",
		"settleSymbols", "settleTypes", "settleComponents", "settleSignatures",
		"signatureArgs", "signatureParameterCache", "safetySafeMemo", "typeNameSymbols",
		"optionalTaggedSymbols", "work", "safetyWork", "materializationReads", "materializationEpoch",
	} {
		if !slices.Contains(dirty, field) {
			t.Errorf("the queries left %s untouched, so the reset of it is not exercised (dirty: %v)", field, dirty)
		}
	}

	session.Reset()
	if diff := apiStabilityAnalysisDiff(t, session.analysis, newApiStabilityAnalysis(tp)); len(diff) != 0 {
		t.Fatalf("reset analysis differs from a fresh one in %v", diff)
	}
}

// TestApiStabilityResetHidesPreviousExport proves no state of one export
// reaches the next one: an exhausted budget and a stale result stored for the
// next root are both discarded by Reset.
func TestApiStabilityResetHidesPreviousExport(t *testing.T) {
	t.Parallel()

	c, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, apiStabilityResetSource)
	defer done()
	primeApiStabilityChecker(t, c)

	leaky := apiStabilityTestExport(t, c, sf, "Leaky")
	other := apiStabilityTestExport(t, c, sf, "Other")
	session := tp.NewApiStabilitySession()
	if got := apiStabilityDependencyNames(session.UsedBySymbol(leaky)); !slices.Equal(got, []string{"E"}) {
		t.Fatalf("Leaky = %v, want [E]", got)
	}

	session.analysis.work = apiStabilityMaxWork
	session.analysis.safetyWork = apiStabilitySafetyMaxWork + 1
	session.analysis.materializationEpoch = 99
	session.analysis.sessionSymbols[other] = newApiStabilitySurface()

	session.Reset()
	used := session.UsedBySymbol(other)
	if got := apiStabilityDependencyNames(used); used.incomplete || !slices.Equal(got, []string{"U"}) {
		t.Fatalf("Other after reset = %v (incomplete %v), want complete [U]", got, used.incomplete)
	}
	if _, ok := session.analysis.sessionSymbols[leaky]; ok {
		t.Fatal("the previous export's symbol result survived the reset")
	}
}

// TestApiStabilityResetSharedHitIsNotRepublished proves a later export reuses
// a published surface after a reset: the composed result is the same, the hit
// is recorded for settlement, it is not queued for publication again, and the
// snapshot keeps the findings it was published with.
func TestApiStabilityResetSharedHitIsNotRepublished(t *testing.T) {
	t.Parallel()

	c, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, `/** @stability experimental */
interface E { value: string }
/** @stability experimental */
export interface Tagged { value: string }
export interface Box<A> { value: A }
export interface First { box: Box<E>; tagged: Tagged }
export interface Second { box: Box<E>; tagged: Tagged }
`)
	defer done()
	primeApiStabilityChecker(t, c)

	first := apiStabilityTestExport(t, c, sf, "First")
	box := c.GetTypeOfSymbol(ast.TSGetSymbolMembers(first)["box"])
	tagged := c.GetDeclaredTypeOfSymbol(apiStabilityTestExport(t, c, sf, "Tagged"))
	session := tp.NewApiStabilitySession()
	if got := apiStabilityDependencyNames(session.UsedBySymbol(first)); !slices.Equal(got, []string{"E", "Tagged"}) {
		t.Fatalf("First = %v, want [E Tagged]", got)
	}
	boxEntry := apiStabilitySharedTypeEntry(tp, box, apiStabilityShallow)
	if boxEntry == nil || len(boxEntry.findings) != 1 {
		t.Fatalf("Box<E> snapshot = %v, want one retained finding", boxEntry)
	}
	taggedEntry := apiStabilitySharedTypeEntry(tp, tagged, apiStabilityShallow)
	if taggedEntry == nil || taggedEntry.findings != nil || len(taggedEntry.removed) != 1 {
		t.Fatalf("Tagged snapshot = %+v, want only its own tag, removed", taggedEntry)
	}

	session.Reset()
	boxKey := apiStabilityTypeKey{t: box, inspect: apiStabilityShallow}
	surface := session.analysis.typeSurface(box, apiStabilityShallow, apiStabilitySubstitution{})
	if len(surface.findings) != 1 || session.analysis.work != 0 {
		t.Fatalf("Box<E> hit = %d findings after %d work, want 1 finding and no work", len(surface.findings), session.analysis.work)
	}
	if !slices.Equal(session.analysis.settleTypes, []apiStabilityTypeKey{boxKey}) {
		t.Fatalf("settleTypes = %v, want the Box<E> hit", session.analysis.settleTypes)
	}
	if len(session.analysis.publishTypes) != 0 {
		t.Fatalf("the hit was queued for publication: %v", session.analysis.publishTypes)
	}

	session.Reset()
	if got := apiStabilityDependencyNames(session.UsedBySymbol(apiStabilityTestExport(t, c, sf, "Second"))); !slices.Equal(got, []string{"E", "Tagged"}) {
		t.Fatalf("Second after reset = %v, want [E Tagged]", got)
	}
	if after := apiStabilitySharedTypeEntry(tp, box, apiStabilityShallow); after != boxEntry || len(after.findings) != 1 {
		t.Fatal("reuse replaced or mutated the Box<E> snapshot")
	}
}
