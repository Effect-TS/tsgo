package typeparser

import (
	"strings"
	"testing"

	"github.com/microsoft/typescript-go/shim/ast"
	"github.com/microsoft/typescript-go/shim/checker"
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
		derived := derivePipeableSignatureFromDataFirst(c, resolved, subjectIndex)
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
				"derived=%s | candidate %d=%s | return=%s typeArgs=%d returnedArity=%v cand->derived=%v derived->cand=%v",
				signatureString(c, derived),
				i,
				signatureString(c, candidate),
				c.TypeToString(candidateReturn),
				len(candidate.TypeParameters()),
				returnedArity,
				checker.Checker_isSignatureAssignableTo(c, candidate, derived, false),
				checker.Checker_isSignatureAssignableTo(c, derived, candidate, false),
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
