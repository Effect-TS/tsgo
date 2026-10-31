package typeparser

import (
	"slices"

	"strings"
	"testing"

	"github.com/effect-ts/tsgo/internal/readonly"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
)

func TestReconstructPipingFlow_NormalizedCalls(t *testing.T) {
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
declare const signal: AbortSignal

export const provided = Effect.provide(program, MyService.Default, { local: true })
export const live = Layer.succeed(MyService, make)
export const run = Effect.runPromise(program, { signal })
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	flows := tp.PipingFlows(sf, false)
	for _, tt := range []struct {
		name string
		kind TransformationKind
		want string
	}{
		{name: "provided", kind: TransformationKindDataFirst, want: "Effect.provide(MyService.Default, { local: true })(program)"},
		{name: "live", kind: TransformationKindDataLast, want: "Layer.succeed(MyService)(make)"},
		{name: "run", kind: TransformationKindCallWithOptionals, want: "Effect.runPromise(program, { signal })"},
	} {
		flow := findFlowByNode(t, sf, flows, findVariableInitializerCallByName(t, sf, tt.name).AsNode())
		if len(flow.partial.transformations) != 1 || flow.partial.transformations[0].kind != tt.kind {
			t.Errorf("%s: expected a single %s transformation", tt.name, tt.kind)
			continue
		}
		if got := stripWhitespace(ReconstructPipingFlow(sf, &flow.partial.subject, ptrTransformations(flow.partial.transformations))); got != stripWhitespace(tt.want) {
			t.Errorf("%s reconstructed flow = %q, want %q", tt.name, got, tt.want)
		}
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
		if len(flow.partial.transformations) == 0 {
			t.Fatalf("%s transformation count = 0, want at least 1", test.name)
		}
		transformation := flow.partial.transformations[len(flow.partial.transformations)-1]
		if transformation.typeArguments == nil {
			t.Fatalf("%s type arguments are nil", test.name)
		}
		got := make([]string, 0, len(transformation.typeArguments.Nodes))
		for _, typeArgument := range transformation.typeArguments.Nodes {
			got = append(got, strings.TrimSpace(nodeText(sf, typeArgument)))
		}
		if !slices.Equal(got, test.want) {
			t.Fatalf("%s type arguments = %v, want %v", test.name, got, test.want)
		}
	}
}

func TestPipingFlows_DecomposesOnlyCurriedPipeableOverloads(t *testing.T) {
	t.Parallel()

	source := `
import { Effect } from "effect"

declare const program: Effect.Effect<number, string>

export const caught = Effect.catch(() => Effect.succeed(0))(program)
export const named = Effect.fn("named")(function*() {
  return 1
})
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	caughtCall := findVariableInitializerCallByName(t, sf, "caught")
	caughtFlow := tp.LongestPipingFlowAt(caughtCall.AsNode(), false)
	if caughtFlow == nil {
		t.Fatal("expected curried catch flow")
	}
	if got := strings.TrimSpace(nodeText(sf, caughtFlow.partial.subject.node)); got != "program" {
		t.Fatalf("caught subject = %q, want program", got)
	}
	assertSingleTransformation(t, sf, caughtFlow, TransformationKindCall, "Effect.catch", []string{"() => Effect.succeed(0)"})

	namedCall := findVariableInitializerCallByName(t, sf, "named")
	namedFlow := tp.LongestPipingFlowAt(namedCall.AsNode(), false)
	if namedFlow == nil {
		t.Fatal("expected named Effect.fn call flow")
	}
	assertSingleTransformation(t, sf, namedFlow, TransformationKindCall, `Effect.fn("named")`, nil)
}

func TestPipingFlows_TransformationOccurrencesAreUnique(t *testing.T) {
	t.Parallel()

	source := `
import { Effect, pipe } from "effect"

declare const base: Effect.Effect<number, string>

export const piped = pipe(
  base,
  Effect.map((value) => value + 1),
  Effect.flatMap((value) => Effect.succeed(value * 2))
)

export const nested = Effect.map(Effect.succeed(1), (value) => value + 1)

export const effectFn = Effect.fn(
  function*() {
    return 1
  },
  Effect.map((value) => value + 1),
  Effect.catch(() => Effect.succeed(0))
)
`

	_, tp, sf, done := compileAndGetCheckerAndSourceFileWithEffectV4Internal(t, source)
	defer done()

	seen := make(map[*ast.Node]int)
	for flowIndex, flow := range tp.PipingFlows(sf, true).All() {
		for _, transformation := range flow.partial.transformations {
			if previousFlowIndex, duplicate := seen[transformation.callee]; duplicate {
				t.Fatalf(
					"transformation %q occurs in flows %d and %d",
					nodeText(sf, transformation.callee),
					previousFlowIndex,
					flowIndex,
				)
			}
			seen[transformation.callee] = flowIndex
		}
	}
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
	if parsed := tp.DataFirstOrLastCall(dataFirstRace.AsNode()); parsed == nil || parsed.subjectIndex != 0 {
		t.Fatal("expected data-first Effect.raceFirst with omitted options to normalize around self")
	}
	dataFirstTimer := dataFirstRace.Arguments.Nodes[0]
	dataFirstFlow := tp.LongestPipingFlowAt(dataFirstTimer, false)
	if dataFirstFlow == nil {
		t.Fatal("expected data-first timer sub-flow")
	}
	if dataFirstFlow.node != dataFirstTimer {
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
	if prefix.subject != outerFlow.partial.subject {
		t.Fatal("prefix should preserve the flow subject")
	}
	if len(prefix.transformations) != 2 {
		t.Fatalf("prefix transformation count = %d, want 2", len(prefix.transformations))
	}
	if got := strings.TrimSpace(nodeText(sf, prefix.transformations[1].callee)); got != "Effect.flatMap" {
		t.Fatalf("last prefix transformation = %q, want Effect.flatMap", got)
	}
	originalKind := outerFlow.partial.transformations[0].kind
	prefix.transformations[0].kind = TransformationKind("test")
	if outerFlow.partial.transformations[0].kind != originalKind {
		t.Fatal("prefix transformations should have an independent backing slice")
	}
	if empty := outerFlow.CopyPrefix(0); empty == nil || len(empty.transformations) != 0 {
		t.Fatal("expected an empty prefix rooted at the flow subject")
	}
	if outerFlow.CopyPrefix(-1) != nil || outerFlow.CopyPrefix(len(outerFlow.partial.transformations)+1) != nil {
		t.Fatal("out-of-bounds prefixes should return nil")
	}
	var nilFlow *PipingFlow
	if nilFlow.CopyPrefix(0) != nil {
		t.Fatal("a nil complete flow should return a nil prefix")
	}
}

func transformationCallees(sf *ast.SourceFile, flow *PipingFlow) []string {
	result := make([]string, 0, len(flow.partial.transformations))
	for i := range flow.partial.transformations {
		result = append(result, strings.TrimSpace(nodeText(sf, flow.partial.transformations[i].callee)))
	}
	return result
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
	if !MatchesPipeableSignature(tp, syntheticDataFirst, syntheticPipeable, 0, nil) {
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
	if !pipeableSignatureShapesMatch(tp, resolved, pipeable, 0, false) {
		t.Fatal("expected signatures to be alpha-equivalent")
	}
	if !MatchesPipeableSignature(tp, resolved, pipeable, 0, nil) {
		t.Fatal("expected full matcher to recognize the signatures")
	}
	before := c.TotalInstantiationCount
	for range 100 {
		if !comparePipeableSignatureTypes(tp, rawSignature(resolved), rawSignature(pipeable), 0, false) {
			t.Fatal("expected repeated raw type comparison to match")
		}
		if !pipeableSignatureShapesMatch(tp, resolved, pipeable, 0, false) {
			t.Fatal("expected cached signature comparison to match")
		}
		if !MatchesPipeableSignature(tp, resolved, pipeable, 0, nil) {
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

func findFlowByNode(t *testing.T, sf *ast.SourceFile, flows readonly.Slice[*PipingFlow], node *ast.Node) *PipingFlow {
	t.Helper()
	for _, flow := range flows.All() {
		if flow != nil && flow.node == node {
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
	if len(flow.partial.transformations) != 1 {
		t.Fatalf("transformation count = %d, want 1", len(flow.partial.transformations))
	}
	tr := flow.partial.transformations[0]
	if tr.kind != wantKind {
		t.Fatalf("kind = %q, want %q", tr.kind, wantKind)
	}
	if got := strings.TrimSpace(nodeText(sf, tr.callee)); got != wantCallee {
		t.Fatalf("callee = %q, want %q", got, wantCallee)
	}
	if len(tr.args) != len(wantArgs) {
		t.Fatalf("arg count = %d, want %d", len(tr.args), len(wantArgs))
	}
	for i, arg := range tr.args {
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
