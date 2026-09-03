package typeparser

import (
	"slices"
	"strings"
	"testing"

	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

func TestDerivedPipeableSignature_Provide(t *testing.T) {
	t.Parallel()

	source := `
import { Effect, Layer, ServiceMap } from "effect"

class MyService extends ServiceMap.Service<MyService>()("MyService", {
  make: Effect.succeed({ value: 1 })
}) {
  static Default = Layer.effect(this, this.make)
}

declare const program: Effect.Effect<number, never, "ProgramEnv">

const provided = Effect.provide(program, MyService.Default, { local: true })
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	call := findVariableInitializerCallByName(t, sf, "provided")
	logDerivedPipeableSignatureComparison(t, tp, call.AsNode())
	result := tp.DataFirstOrLastCall(call.AsNode())
	if result == nil {
		t.Fatal("expected provide call to normalize via derived signature comparison")
		return
	}
	if result.SubjectIndex != 0 {
		t.Fatalf("expected provide subject index 0, got %d", result.SubjectIndex)
	}
}

func TestDerivedPipeableSignature_LayerSucceed(t *testing.T) {
	t.Parallel()

	source := `
import { Layer, ServiceMap } from "effect"

class Service extends ServiceMap.Service<Service, {
  readonly value: 1
}>()("Service") {}

declare const make: { readonly value: 1 }

const live = Layer.succeed(Service, make)
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	call := findVariableInitializerCallByName(t, sf, "live")
	logDerivedPipeableSignatureComparison(t, tp, call.AsNode())
	result := tp.DataFirstOrLastCall(call.AsNode())
	if result == nil {
		t.Fatal("expected Layer.succeed call to normalize via derived signature comparison")
		return
	}
	if result.SubjectIndex != 1 {
		t.Fatalf("expected Layer.succeed subject index 1, got %d", result.SubjectIndex)
	}
}

func TestPipingFlows_DataFirstCalls(t *testing.T) {
	t.Parallel()

	source := `
import { Effect, Layer, ServiceMap } from "effect"

class MyService extends ServiceMap.Service<MyService>()("MyService", {
  make: Effect.succeed({ value: 1 as const })
}) {
  static Default = Layer.effect(this, this.make)
}

declare const program: Effect.Effect<number, never, "ProgramEnv">
declare const make: { readonly value: 1 }

export const provided = Effect.provide(program, MyService.Default, { local: true })
export const live = Layer.succeed(MyService, make)
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	flows := tp.PipingFlows(sf, false)
	providedCall := findVariableInitializerCallByName(t, sf, "provided").AsNode()
	liveCall := findVariableInitializerCallByName(t, sf, "live").AsNode()

	providedFlow := findFlowByNode(t, sf, flows, providedCall)
	if strings.TrimSpace(nodeText(sf, providedFlow.Subject.Node)) != "program" {
		t.Fatalf("provided subject = %q, want %q", strings.TrimSpace(nodeText(sf, providedFlow.Subject.Node)), "program")
	}
	assertSingleTransformation(t, sf, providedFlow, TransformationKindDataFirst, "Effect.provide", []string{"MyService.Default", "{ local: true }"})
	if got := stripWhitespace(ReconstructPipingFlow(sf, &providedFlow.Subject, ptrTransformations(providedFlow.Transformations))); got != stripWhitespace("Effect.provide(MyService.Default, { local: true })(program)") {
		t.Fatalf("provided reconstructed flow = %q", got)
	}

	liveFlow := findFlowByNode(t, sf, flows, liveCall)
	if strings.TrimSpace(nodeText(sf, liveFlow.Subject.Node)) != "make" {
		t.Fatalf("live subject = %q, want %q", strings.TrimSpace(nodeText(sf, liveFlow.Subject.Node)), "make")
	}
	assertSingleTransformation(t, sf, liveFlow, TransformationKindDataLast, "Layer.succeed", []string{"MyService"})
	if got := stripWhitespace(ReconstructPipingFlow(sf, &liveFlow.Subject, ptrTransformations(liveFlow.Transformations))); got != stripWhitespace("Layer.succeed(MyService)(make)") {
		t.Fatalf("live reconstructed flow = %q", got)
	}
}

func TestPipingFlows_TypeArguments(t *testing.T) {
	t.Parallel()

	source := `
import { pipe } from "effect"

declare function transform<A, B>(self: A, f: (value: A) => B): B
declare function transform<A, B>(f: (value: A) => B): (self: A) => B
declare function identity<A>(value: A): A

export const piped = pipe("value", transform<string, number>((value) => value.length))
export const dataFirst = transform<string, number>("value", (value) => value.length)
export const curried = transform<string, number>((value) => value.length)("value")
export const called = identity<string>("value")
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	for _, test := range []struct {
		name string
		want []string
	}{
		{name: "piped", want: []string{"string", "number"}},
		{name: "dataFirst", want: []string{"string", "number"}},
		{name: "curried", want: []string{"string", "number"}},
		{name: "called", want: []string{"string"}},
	} {
		call := findVariableInitializerCallByName(t, sf, test.name)
		flow := findFlowByNode(t, sf, tp.PipingFlows(sf, false), call.AsNode())
		if len(flow.Transformations) == 0 {
			t.Fatalf("%s transformation count = 0, want at least 1", test.name)
		}
		transformation := flow.Transformations[len(flow.Transformations)-1]
		if transformation.TypeArguments == nil {
			t.Fatalf("%s type arguments are nil", test.name)
		}
		got := make([]string, 0, len(transformation.TypeArguments.Nodes))
		for _, typeArgument := range transformation.TypeArguments.Nodes {
			got = append(got, strings.TrimSpace(nodeText(sf, typeArgument)))
		}
		if !slices.Equal(got, test.want) {
			t.Fatalf("%s type arguments = %v, want %v", test.name, got, test.want)
		}
	}
}

func TestPipingFlows_SkipsParentheses(t *testing.T) {
	t.Parallel()

	source := `
import { Effect, Option } from "effect"

export const some = Effect.succeed(((Option.some(1))))
export const none = Effect.succeed(((Option.none())))
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	someCall := findVariableInitializerCallByName(t, sf, "some")
	someFlow := findFlowByNode(t, sf, tp.PipingFlows(sf, false), someCall.AsNode())
	if got := strings.TrimSpace(nodeText(sf, someFlow.Subject.Node)); got != "1" {
		t.Fatalf("some subject = %q, want %q", got, "1")
	}
	if len(someFlow.Transformations) != 2 {
		t.Fatalf("some transformation count = %d, want 2", len(someFlow.Transformations))
	}
	if got := strings.TrimSpace(nodeText(sf, someFlow.Transformations[0].Callee)); got != "Option.some" {
		t.Fatalf("first some transformation = %q, want %q", got, "Option.some")
	}
	if got := strings.TrimSpace(nodeText(sf, someFlow.Transformations[1].Callee)); got != "Effect.succeed" {
		t.Fatalf("second some transformation = %q, want %q", got, "Effect.succeed")
	}

	noneCall := findVariableInitializerCallByName(t, sf, "none")
	noneFlow := findFlowByNode(t, sf, tp.PipingFlows(sf, false), noneCall.AsNode())
	if got := strings.TrimSpace(nodeText(sf, noneFlow.Subject.Node)); got != "Option.none()" {
		t.Fatalf("none subject = %q, want %q", got, "Option.none()")
	}
	assertSingleTransformation(t, sf, noneFlow, TransformationKindCall, "Effect.succeed", nil)
}

func TestLongestPipingFlowAt_ReturnsLongestNestedFlow(t *testing.T) {
	t.Parallel()

	source := `
import { Effect } from "effect"

declare const work: Effect.Effect<string>

export const dataFirst = Effect.raceFirst(
  Effect.flatMap(Effect.sleep("1 second"), () => Effect.fail("timeout")),
  work
)

export const pipeable = Effect.raceFirst(
  work,
  ((Effect.sleep("2 seconds").pipe(
    Effect.andThen(Effect.succeed("fallback"))
  )))
)
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	dataFirstRace := findVariableInitializerCallByName(t, sf, "dataFirst")
	if parsed := tp.DataFirstOrLastCall(dataFirstRace.AsNode()); parsed == nil || parsed.SubjectIndex != 0 {
		t.Fatal("expected data-first Effect.raceFirst with omitted options to normalize around self")
	}
	dataFirstTimer := dataFirstRace.Arguments.Nodes[0]
	dataFirstFlow := tp.LongestPipingFlowAt(dataFirstTimer, false)
	if dataFirstFlow == nil {
		t.Fatal("expected data-first timer sub-flow")
	}
	if dataFirstFlow.Node != dataFirstTimer {
		t.Fatal("data-first sub-flow should be rooted at the requested node")
	}
	if got := transformationCallees(sf, dataFirstFlow); !slices.Equal(got, []string{"Effect.sleep", "Effect.flatMap"}) {
		t.Fatalf("data-first transformations = %v", got)
	}

	pipeableRace := findVariableInitializerCallByName(t, sf, "pipeable")
	pipeableTimer := pipeableRace.Arguments.Nodes[1]
	pipeableFlow := tp.LongestPipingFlowAt(pipeableTimer, false)
	if pipeableFlow == nil {
		t.Fatal("expected parenthesized pipeable timer sub-flow")
	}
	if got := transformationCallees(sf, pipeableFlow); !slices.Equal(got, []string{"Effect.sleep", "Effect.andThen"}) {
		t.Fatalf("pipeable transformations = %v", got)
	}

	outerFlow := tp.LongestPipingFlowAt(dataFirstRace.AsNode(), false)
	if outerFlow == nil {
		t.Fatal("expected complete outer flow")
	}
	if got := transformationCallees(sf, outerFlow); !slices.Equal(got, []string{"Effect.sleep", "Effect.flatMap", "Effect.raceFirst"}) {
		t.Fatalf("outer transformations = %v", got)
	}

	prefix := outerFlow.CopyPrefix(2)
	if prefix == nil {
		t.Fatal("expected a two-transformation prefix")
	}
	if prefix.Subject != outerFlow.Subject {
		t.Fatal("prefix should preserve the flow subject")
	}
	if len(prefix.Transformations) != 2 {
		t.Fatalf("prefix transformation count = %d, want 2", len(prefix.Transformations))
	}
	if got := strings.TrimSpace(nodeText(sf, prefix.Transformations[1].Callee)); got != "Effect.flatMap" {
		t.Fatalf("last prefix transformation = %q, want Effect.flatMap", got)
	}
	originalKind := outerFlow.Transformations[0].Kind
	prefix.Transformations[0].Kind = TransformationKind("test")
	if outerFlow.Transformations[0].Kind != originalKind {
		t.Fatal("prefix transformations should have an independent backing slice")
	}
	if empty := outerFlow.CopyPrefix(0); empty == nil || len(empty.Transformations) != 0 {
		t.Fatal("expected an empty prefix rooted at the flow subject")
	}
	if outerFlow.CopyPrefix(-1) != nil || outerFlow.CopyPrefix(len(outerFlow.Transformations)+1) != nil {
		t.Fatal("out-of-bounds prefixes should return nil")
	}
	var nilFlow *PipingFlow
	if nilFlow.CopyPrefix(0) != nil {
		t.Fatal("a nil complete flow should return a nil prefix")
	}
}

func transformationCallees(sf *ast.SourceFile, flow *PipingFlow) []string {
	result := make([]string, 0, len(flow.Transformations))
	for i := range flow.Transformations {
		result = append(result, strings.TrimSpace(nodeText(sf, flow.Transformations[i].Callee)))
	}
	return result
}

func TestDataFirstOrLastCall_OptionMatchV4(t *testing.T) {
	t.Parallel()

	source := `
import { Effect, Option } from "effect"

declare const value: Option.Option<number>

export const result = Option.match(value, {
  onNone: () => Effect.fail("missing"),
  onSome: Effect.succeed
})
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	call := findVariableInitializerCallByName(t, sf, "result")
	result := tp.DataFirstOrLastCall(call.AsNode())
	if result == nil {
		t.Fatal("expected Option.match data-first call to normalize")
	}
	if result.SubjectIndex != 0 {
		t.Fatalf("subject index = %d, want 0", result.SubjectIndex)
	}
	if got := strings.TrimSpace(nodeText(sf, result.Subject)); got != "value" {
		t.Fatalf("subject = %q, want %q", got, "value")
	}
	if got := strings.TrimSpace(nodeText(sf, result.Callee)); got != "Option.match" {
		t.Fatalf("callee = %q, want %q", got, "Option.match")
	}
	if len(result.Args) != 1 || !strings.HasPrefix(strings.TrimSpace(nodeText(sf, result.Args[0])), "{") {
		t.Fatalf("expected handlers object as sole transformation argument")
	}

	flow := findFlowByNode(t, sf, tp.PipingFlows(sf, false), call.AsNode())
	if got := strings.TrimSpace(nodeText(sf, flow.Subject.Node)); got != "value" {
		t.Fatalf("flow subject = %q, want %q", got, "value")
	}
	assertSingleTransformation(t, sf, flow, TransformationKindDataFirst, "Option.match", []string{`{
  onNone: () => Effect.fail("missing"),
  onSome: Effect.succeed
}`})
}

func TestDataFirstOrLastCall_AlphaEquivalentOverloads(t *testing.T) {
	t.Parallel()

	source := `
interface Box<A> { readonly value: A }
interface Pair<A, B> { readonly left: A; readonly right: B }

declare function reordered<A, B, C = B>(self: Box<A>, options: {
  readonly onEmpty: () => B
  readonly onValue: (value: A) => C
}): B | C
declare function reordered<B, A, C = B>(options: {
  readonly onValue: (value: A) => C
  readonly onEmpty: () => B
}): (self: Box<A>) => C | B

declare function moved<A>(self: Box<A>, index: number): A
declare function moved(index: number): <Value>(self: Box<Value>) => Value

declare function mapped<A extends object, N extends string, B>(self: A, name: N, value: B): {
  [K in N | keyof A]: K extends keyof A ? A[K] : B
}
declare function mapped<N extends string, A extends object, B>(name: N, value: B): (self: A) => {
  [K in N | keyof A]: K extends keyof A ? A[K] : B
}

declare function distinctRoles<A, B>(self: Box<A>, f: (value: A) => B): Pair<A, B>
declare function distinctRoles<A, B>(f: (value: B) => A): (self: Box<A>) => Pair<A, B>

declare function differentBinders<T>(self: Box<T>, f: (value: T) => T): T | T
declare function differentBinders<U, V>(f: (value: U) => V): (self: Box<U>) => U | V

namespace Left { export interface Same<A> { readonly value: A } }
namespace Right { export interface Same<A> { readonly value: A } }
declare function differentSymbols<A>(self: Left.Same<A>, size: number): A
declare function differentSymbols<A>(size: number): (self: Right.Same<A>) => A

declare function differentDefault<A, B = A>(self: Box<A>, f: (value: A) => B): B
declare function differentDefault<A, B = string>(f: (value: A) => B): (self: Box<A>) => B

declare function differentReadonly<A, B>(self: Box<A>, options: { readonly f: (value: A) => B }): B
declare function differentReadonly<A, B>(options: { f: (value: A) => B }): (self: Box<A>) => B

declare function optionalSubject<A>(self: Box<A>, size: number): A
declare function optionalSubject(size: number): <A>(self?: Box<A>) => A

declare const box: Box<number>
declare const left: Left.Same<number>
declare const object: { readonly a: number }

export const reorderedResult = reordered(box, {
  onEmpty: () => "empty",
  onValue: (value) => value
})
export const movedResult = moved(box, 0)
export const mappedResult = mapped(object, "b", true)
export const distinctRolesResult = distinctRoles(box, String)
export const differentBindersResult = differentBinders(box, (value) => value)
export const differentSymbolsResult = differentSymbols(left, 1)
export const differentDefaultResult = differentDefault(box, String)
export const differentReadonlyResult = differentReadonly(box, { f: String })
export const optionalSubjectResult = optionalSubject(box, 1)
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done()

	for _, name := range []string{"reorderedResult", "movedResult", "mappedResult"} {
		call := findVariableInitializerCallByName(t, sf, name)
		result := tp.DataFirstOrLastCall(call.AsNode())
		if result == nil {
			t.Errorf("expected %s to normalize", name)
			continue
		}
		if result.SubjectIndex != 0 {
			t.Errorf("%s subject index = %d, want 0", name, result.SubjectIndex)
		}
	}

	for _, name := range []string{
		"distinctRolesResult",
		"differentBindersResult",
		"differentSymbolsResult",
		"differentDefaultResult",
		"differentReadonlyResult",
		"optionalSubjectResult",
	} {
		call := findVariableInitializerCallByName(t, sf, name)
		if result := tp.DataFirstOrLastCall(call.AsNode()); result != nil {
			t.Errorf("expected %s not to normalize", name)
		}
	}
}

func TestDataFirstOrLastCall_CallableAlias(t *testing.T) {
	t.Parallel()

	source := `
type Pipeable = (self: string) => number

declare function operation(self: string, size: number): number
declare function operation(size: number): Pipeable

export const result = operation("value", 1)
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done()

	call := findVariableInitializerCallByName(t, sf, "result")
	result := tp.DataFirstOrLastCall(call.AsNode())
	if result == nil {
		t.Fatal("expected a pipeable overload returned through a callable alias to normalize")
	}
	if result.SubjectIndex != 0 {
		t.Fatalf("subject index = %d, want 0", result.SubjectIndex)
	}
}

func TestDataFirstOrLastCall_InferredParameterTypes(t *testing.T) {
	t.Parallel()

	source := `
declare function operation(self: string, size): number
declare function operation(size): (self: string) => number

export const result = operation("value", 1)
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done()

	call := findVariableInitializerCallByName(t, sf, "result")
	if result := tp.DataFirstOrLastCall(call.AsNode()); result == nil || result.SubjectIndex != 0 {
		t.Fatal("expected overloads with inferred any parameter types to normalize")
	}
}

func TestPipeableSignatureShapeWithoutExplicitDeclarations(t *testing.T) {
	t.Parallel()

	source := `
declare function operation(self: string, size: number): number
declare function operation(size: number): (self: string) => number

export const result = operation("value", 1)
`

	c, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done()
	call := findVariableInitializerCallByName(t, sf, "result")
	resolved := rawSignature(c.GetResolvedSignature(call.AsNode()))
	candidates := c.GetSignaturesOfType(tp.GetTypeAtLocation(call.Expression), checker.SignatureKindCall)
	if resolved == nil || len(candidates) != 2 {
		t.Fatal("expected both overload signatures")
	}
	pipeable := rawSignature(candidates[0])
	if len(pipeable.Parameters()) != 1 {
		pipeable = rawSignature(candidates[1])
	}

	syntheticDataFirst := checker.Checker_newCallSignature(
		c, resolved.TypeParameters(), resolved.ThisParameter(), resolved.Parameters(), c.GetReturnTypeOfSignature(resolved),
	)
	syntheticPipeable := checker.Checker_newCallSignature(
		c, pipeable.TypeParameters(), pipeable.ThisParameter(), pipeable.Parameters(), c.GetReturnTypeOfSignature(pipeable),
	)
	if !ast.NodeIsSynthesized(syntheticDataFirst.Declaration()) || !ast.NodeIsSynthesized(syntheticPipeable.Declaration()) {
		t.Fatal("expected signatures with no source declaration site")
	}
	if !MatchesPipeableSignature(c, syntheticDataFirst, syntheticPipeable, 0, nil) {
		t.Fatal("expected raw signature structures to match without declaration nodes")
	}
}

func TestPipeableSignatureShapeDoesNotInstantiateTypes(t *testing.T) {
	t.Parallel()

	source := `
interface Box<A> { readonly value: A }
declare function operation<A, B>(self: Box<A>, f: (value: A) => B): B | A
declare function operation<B, A>(f: (value: A) => B): (self: Box<A>) => A | B
declare const box: Box<number>
export const result = operation(box, String)
`

	c, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done()
	call := findVariableInitializerCallByName(t, sf, "result")
	resolved := c.GetResolvedSignature(call.AsNode())
	candidates := c.GetSignaturesOfType(tp.GetTypeAtLocation(call.Expression), checker.SignatureKindCall)
	var pipeable *checker.Signature
	for _, candidate := range candidates {
		raw := rawSignature(candidate)
		if raw != nil && len(raw.Parameters()) == 1 {
			pipeable = candidate
			break
		}
	}
	if resolved == nil || pipeable == nil {
		t.Fatal("expected data-first and pipeable signatures")
	}
	if !pipeableSignatureShapesMatch(c, resolved, pipeable, 0) {
		t.Fatal("expected signatures to be alpha-equivalent")
	}
	if !MatchesPipeableSignature(c, resolved, pipeable, 0, nil) {
		t.Fatal("expected full matcher to recognize the signatures")
	}
	before := c.TotalInstantiationCount
	for range 100 {
		if !comparePipeableSignatureTypes(c, rawSignature(resolved), rawSignature(pipeable), 0) {
			t.Fatal("expected repeated raw type comparison to match")
		}
		if !pipeableSignatureShapesMatch(c, resolved, pipeable, 0) {
			t.Fatal("expected cached signature comparison to match")
		}
		if !MatchesPipeableSignature(c, resolved, pipeable, 0, nil) {
			t.Fatal("expected repeated full signature comparison to match")
		}
	}
	if after := c.TotalInstantiationCount; after != before {
		t.Fatalf("type instantiation count changed from %d to %d", before, after)
	}
}

func TestPipeableSignatureShapeFailsClosedAtDepthBudget(t *testing.T) {
	t.Parallel()

	deepType := "number"
	for range pipeableShapeMaxDepth + 2 {
		deepType = "{ readonly value: " + deepType + " }"
	}
	source := `
declare function operation(self: ` + deepType + `, size: number): number
declare function operation(size: number): (self: ` + deepType + `) => number
declare const value: ` + deepType + `
export const result = operation(value, 1)
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done()
	call := findVariableInitializerCallByName(t, sf, "result")
	if result := tp.DataFirstOrLastCall(call.AsNode()); result != nil {
		t.Fatal("expected over-budget signature shape to fail closed")
	}
}

func TestDataFirstOrLastCall_RealAlphaEquivalentEffectApis(t *testing.T) {
	t.Parallel()

	source := `
import { Exit, Option, Result, UndefinedOr } from "effect"

declare const option: Option.Option<number>
declare const result: Result.Result<number, string>
declare const maybe: number | undefined
declare const exit: Exit.Exit<number, string>

export const optionGetOrElse = Option.getOrElse(option, () => "none")
export const resultMatch = Result.match(result, {
  onFailure: (error) => error.length,
  onSuccess: (value) => value
})
export const undefinedOrMap = UndefinedOr.map(maybe, (value) => String(value))
export const exitMatch = Exit.match(exit, {
  onFailure: (cause) => String(cause),
  onSuccess: (value) => value
})
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	for _, name := range []string{"optionGetOrElse", "resultMatch", "undefinedOrMap", "exitMatch"} {
		call := findVariableInitializerCallByName(t, sf, name)
		result := tp.DataFirstOrLastCall(call.AsNode())
		if result == nil {
			t.Errorf("expected %s to normalize", name)
			continue
		}
		if result.SubjectIndex != 0 {
			t.Errorf("%s subject index = %d, want 0", name, result.SubjectIndex)
		}
	}
}

func TestParseDataFirstCallAsPipeable_CatchAllV3(t *testing.T) {
	t.Parallel()

	source := `
// @effect-v3
import * as Effect from "effect/Effect"

export const shouldReportDataFirst = Effect.catchAll(
  Effect.never,
  () => Effect.log("error")
)
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV3Internal(t, source)
	defer done()

	call := findVariableInitializerCallByName(t, sf, "shouldReportDataFirst")
	result := tp.DataFirstOrLastCall(call.AsNode())
	if result == nil {
		t.Fatal("expected data-first catchAll to normalize")
		return
	}
	if strings.TrimSpace(nodeText(sf, result.Subject)) != "Effect.never" {
		t.Fatalf("subject = %q, want %q", strings.TrimSpace(nodeText(sf, result.Subject)), "Effect.never")
	}
	if strings.TrimSpace(nodeText(sf, result.Callee)) != "Effect.catchAll" {
		t.Fatalf("callee = %q, want %q", strings.TrimSpace(nodeText(sf, result.Callee)), "Effect.catchAll")
	}
	if result.SubjectIndex != 0 {
		t.Fatalf("subject index = %d, want 0", result.SubjectIndex)
	}
	if len(result.Args) != 1 || stripWhitespace(nodeText(sf, result.Args[0])) != stripWhitespace("() => Effect.log(\"error\")") {
		t.Fatalf("args = %q", strings.TrimSpace(nodeText(sf, result.Args[0])))
	}

	flows := tp.PipingFlows(sf, false)
	flow := findFlowByNode(t, sf, flows, call.AsNode())
	if strings.TrimSpace(nodeText(sf, flow.Subject.Node)) != "Effect.never" {
		t.Fatalf("flow subject = %q, want %q", strings.TrimSpace(nodeText(sf, flow.Subject.Node)), "Effect.never")
	}
	assertSingleTransformation(t, sf, flow, TransformationKindDataFirst, "Effect.catchAll", []string{"() => Effect.log(\"error\")"})
}

func TestDataFirstOrLastCallRejectsDifferentReturnOrigins(t *testing.T) {
	t.Parallel()

	source := `
type DataFirstResult<T> = { readonly dataFirst: T }
type DataLastResult<T> = { readonly dataLast: T }

declare function operation<T>(self: T, size: number): DataFirstResult<T>
declare function operation(size: number): <T>(self: T) => DataLastResult<T>

const result = operation("value", 2)
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done()

	call := findVariableInitializerCallByName(t, sf, "result")
	if result := tp.DataFirstOrLastCall(call.AsNode()); result != nil {
		t.Fatal("expected overloads with different return origins not to normalize")
	}
}

func TestDataFirstOrLastCallRejectsIncompatibleSubject(t *testing.T) {
	t.Parallel()

	source := `
type Result = { readonly value: string }

declare function operation(self: string, size: number): Result
declare function operation(size: number): (self: boolean) => Result

const result = operation("value", 2)
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done()

	call := findVariableInitializerCallByName(t, sf, "result")
	if result := tp.DataFirstOrLastCall(call.AsNode()); result != nil {
		t.Fatal("expected overloads with an incompatible subject not to normalize")
	}
}

func TestDataFirstOrLastCallRejectsAmbiguousSubjectIndex(t *testing.T) {
	t.Parallel()

	source := `
declare function operation<A>(first: A, second: A): A
declare function operation<A>(second: A): (first: A) => A

const result = operation(1, 2)
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileInternal(t, source)
	defer done()

	call := findVariableInitializerCallByName(t, sf, "result")
	if result := tp.DataFirstOrLastCall(call.AsNode()); result != nil {
		t.Fatal("expected an ambiguous subject index not to normalize")
	}
}

func logDerivedPipeableSignatureComparison(t *testing.T, tp *TypeParser, node *ast.Node) {
	t.Helper()
	if tp == nil || tp.checker == nil || node == nil {
		return
	}

	c := tp.checker
	call := node.AsCallExpression()
	if call == nil || len(call.Arguments.Nodes) < 2 {
		return
	}

	resolved := c.GetResolvedSignature(node)
	if resolved == nil || resolved.Declaration() == nil {
		t.Fatal("expected resolved data-first signature")
	}
	actualSymbol := checker.Checker_getSymbolOfDeclaration(c, resolved.Declaration())
	resolvedParamCount := len(resolved.Parameters())
	subjectIndexes := []int{0}
	if len(call.Arguments.Nodes) > 1 {
		last := len(call.Arguments.Nodes) - 1
		preferFirst := false
		if params := resolved.Parameters(); len(params) > 0 {
			preferFirst = isLikelySelfParameter(params[0])
		}
		if preferFirst {
			subjectIndexes = []int{0, last}
		} else {
			subjectIndexes = []int{last, 0}
		}
	}

	t.Logf("data-first resolved return type: %s", c.TypeToString(c.GetReturnTypeOfSignature(resolved)))
	for _, subjectIndex := range subjectIndexes {
		derived := DerivePipeableSignatureFromDataFirst(c, resolved, subjectIndex)
		if derived == nil {
			t.Logf("subjectIndex=%d derived=nil", subjectIndex)
			continue
		}
		derivedReturn := c.GetReturnTypeOfSignature(derived)
		innerSigs := c.GetSignaturesOfType(derivedReturn, checker.SignatureKindCall)
		innerReturns := make([]string, 0, len(innerSigs))
		for _, sig := range innerSigs {
			if sig != nil {
				innerReturns = append(innerReturns, c.TypeToString(c.GetReturnTypeOfSignature(sig)))
			}
		}
		t.Logf("subjectIndex=%d derived return=%s typeArgs=%d innerReturns=%v", subjectIndex, c.TypeToString(derivedReturn), len(derived.TypeParameters()), innerReturns)

		resolvedOuter, candidates := checker.GetResolvedSignatureForSignatureHelp(node, resolvedParamCount-1, c)
		_ = resolvedOuter
		for i, candidate := range candidates {
			if candidate == nil || candidate.Declaration() == nil {
				continue
			}
			candidateSymbol := checker.Checker_getSymbolOfDeclaration(c, candidate.Declaration())
			if candidateSymbol == nil || checker.Checker_getSymbolIfSameReference(c, actualSymbol, candidateSymbol) == nil {
				continue
			}
			candidateReturn := c.GetReturnTypeOfSignature(candidate)
			returned := c.GetSignaturesOfType(candidateReturn, checker.SignatureKindCall)
			returnedArity := make([]int, 0, len(returned))
			for _, rs := range returned {
				if rs != nil {
					returnedArity = append(returnedArity, len(rs.Parameters()))
				}
			}
			t.Logf(
				"derived=%s | candidate %d=%s | return=%s typeArgs=%d returnedArity=%v matches=%v",
				signatureString(c, derived),
				i,
				signatureString(c, candidate),
				c.TypeToString(candidateReturn),
				len(candidate.TypeParameters()),
				returnedArity,
				MatchesPipeableSignature(c, resolved, candidate, subjectIndex, nil),
			)
		}
	}
}

func signatureString(c *checker.Checker, sig *checker.Signature) string {
	if c == nil || sig == nil {
		return "<nil>"
	}
	return c.SignatureToStringEx(sig, nil, checker.TypeFormatFlagsWriteArrowStyleSignature, nil)
}

func findFlowByNode(t *testing.T, sf *ast.SourceFile, flows []*PipingFlow, node *ast.Node) *PipingFlow {
	t.Helper()
	for _, flow := range flows {
		if flow != nil && flow.Node == node {
			return flow
		}
	}
	t.Fatalf("flow for node %q not found", nodeText(sf, node))
	return nil
}

func assertSingleTransformation(t *testing.T, sf *ast.SourceFile, flow *PipingFlow, wantKind TransformationKind, wantCallee string, wantArgs []string) {
	t.Helper()
	if flow == nil {
		t.Fatal("flow is nil")
		return
	}
	if len(flow.Transformations) != 1 {
		t.Fatalf("transformation count = %d, want 1", len(flow.Transformations))
	}
	tr := flow.Transformations[0]
	if tr.Kind != wantKind {
		t.Fatalf("kind = %q, want %q", tr.Kind, wantKind)
	}
	if got := strings.TrimSpace(nodeText(sf, tr.Callee)); got != wantCallee {
		t.Fatalf("callee = %q, want %q", got, wantCallee)
	}
	if len(tr.Args) != len(wantArgs) {
		t.Fatalf("arg count = %d, want %d", len(tr.Args), len(wantArgs))
	}
	for i, arg := range tr.Args {
		if got := strings.TrimSpace(nodeText(sf, arg)); got != wantArgs[i] {
			t.Fatalf("arg[%d] = %q, want %q", i, got, wantArgs[i])
		}
	}
}

func ptrTransformations(in []PipingFlowTransformation) []*PipingFlowTransformation {
	result := make([]*PipingFlowTransformation, 0, len(in))
	for i := range in {
		result = append(result, &in[i])
	}
	return result
}

func stripWhitespace(s string) string {
	return strings.Join(strings.Fields(s), "")
}

func findVariableInitializerCallByName(t *testing.T, sf *ast.SourceFile, name string) *ast.CallExpression {
	t.Helper()

	var found *ast.CallExpression
	var visit func(*ast.Node)
	visit = func(node *ast.Node) {
		if node == nil || found != nil {
			return
		}
		if node.Kind == ast.KindVariableDeclaration {
			decl := node.AsVariableDeclaration()
			if decl != nil && decl.Name() != nil && decl.Name().Kind == ast.KindIdentifier {
				if ident := decl.Name().AsIdentifier(); ident != nil && ident.Text == name && decl.Initializer != nil && decl.Initializer.Kind == ast.KindCallExpression {
					found = decl.Initializer.AsCallExpression()
					return
				}
			}
		}
		node.ForEachChild(func(child *ast.Node) bool {
			visit(child)
			return false
		})
	}

	visit(sf.AsNode())
	if found == nil {
		t.Fatalf("initializer call for variable %q not found", name)
	}
	return found
}
