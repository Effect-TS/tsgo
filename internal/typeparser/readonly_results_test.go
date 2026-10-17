package typeparser_test

import (
	"context"
	"slices"
	"strings"
	"testing"
	"testing/fstest"

	"github.com/effect-ts/tsgo/etsgoapi"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/bundled"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
)

func TestReadonlyFlowConsumer(t *testing.T) {
	t.Parallel()
	const source = `
declare const source: { pipe(...steps: Array<(n: number) => number>): number }
declare function add(n: number): (n: number) => number
export const result = source.pipe(add(1), add(2), add(1))
`
	fs := bundled.WrapFS(vfstest.FromMap(map[string]any{
		"/test.ts": &fstest.MapFile{Data: []byte(source)},
	}, tspath.CaseSensitive))
	program := compiler.NewProgram(compiler.ProgramOptions{
		Config: tsoptions.NewParsedCommandLine(&core.CompilerOptions{
			Target: core.ScriptTargetESNext, Strict: core.TSTrue,
		}, []tspath.RootedFilePath{"/test.ts"}, nil, "/", tspath.CaseSensitive),
		Host:           compiler.NewCompilerHost(fs, bundled.LibPath(), nil, nil, nil),
		SingleThreaded: core.TSTrue,
	})
	c, done := program.GetTypeChecker(context.Background())
	defer done()
	sf := program.GetSourceFile("/test.ts")
	if sf == nil {
		t.Fatal("source file missing")
	}
	tp := typeparser.NewTypeParser(c.Program(), c)
	flows := tp.PipingFlows(sf, false)
	if flows.Len() != 1 {
		t.Fatalf("got %d flows, want 1", flows.Len())
	}
	flow := flows.At(0)
	text := func(node *ast.Node) string {
		if node == nil {
			return "<nil>"
		}
		return strings.TrimSpace(sf.Text()[node.Pos():node.End()])
	}
	assertFlow := func() {
		t.Helper()
		if got := text(flow.Subject().Node()); got != "source" {
			t.Fatalf("subject = %q, want source", got)
		}
		var args []string
		for _, step := range flow.Transformations().All() {
			if step.Kind() != typeparser.TransformationKindPipeable || text(step.Callee()) != "add" || step.Args().Len() != 1 {
				t.Fatal("pipe transformation changed")
			}
			args = append(args, text(step.Args().At(0)))
		}
		if !slices.Equal(args, []string{"1", "2", "1"}) {
			t.Fatalf("arguments = %v, want [1 2 1]", args)
		}
	}
	assertFlow()

	ownedFlows := flows.Copy()
	ownedFlows[0] = nil
	ownedSteps := flow.Transformations().Copy()
	ownedSteps[0] = typeparser.PipingFlowTransformation{}
	ownedArgs := flow.Transformations().At(0).Args().Copy()
	ownedArgs[0] = nil
	partial := flow.Partial()
	if partial.Transformations().Len() != 3 {
		t.Fatal("partial flow lost transformations")
	}
	partial = typeparser.PartialPipingFlow{}
	if partial.Transformations().Len() != 0 {
		t.Fatal("local partial was not replaced")
	}

	subject := func(value typeparser.PipingFlowSubject) bool {
		matched := text(value.Node()) == "source"
		value = typeparser.PipingFlowSubject{}
		if value.Node() != nil {
			t.Fatal("subject copy was not replaced")
		}
		return matched
	}
	step := func(want string) func(typeparser.PipingFlowTransformation) bool {
		return func(value typeparser.PipingFlowTransformation) bool {
			matched := value.Args().Len() == 1 && text(value.Args().At(0)) == want
			value = typeparser.PipingFlowTransformation{}
			if value.Args().Len() != 0 {
				t.Fatal("step copy was not replaced")
			}
			return matched
		}
	}
	if !flow.MatchesPrefix(subject, step("1")) || !flow.MatchesExactly(subject, step("1"), step("2"), step("1")) {
		t.Fatal("mutating predicate copies changed matching")
	}
	matches := flow.FindTransformationSequences(step("1"))
	var starts []int
	for _, match := range matches.All() {
		starts = append(starts, match.Start())
		owned := match.Transformations().Copy()
		owned[0] = typeparser.PipingFlowTransformation{}
	}
	if !slices.Equal(starts, []int{0, 2}) {
		t.Fatalf("sequence starts = %v, want [0 2]", starts)
	}
	if tp.PipingFlows(sf, false).At(0) != flow {
		t.Fatal("cached flow identity changed")
	}
	assertFlow()

	api := etsgoapi.NewTypeParser(c.Program(), c)
	publicFlows := api.PipingFlows(sf, false)
	if len(publicFlows) != 1 || publicFlows[0].Transformations().Len() != 3 {
		t.Fatal("public conversion lost the flow")
	}
	publicArgs := publicFlows[0].Transformations().At(0).Args().Copy()
	publicArgs[0] = nil
	publicFlows[0] = nil
	again := api.PipingFlows(sf, false)
	if len(again) != 1 || text(again[0].Transformations().At(0).Args().At(0)) != "1" {
		t.Fatal("public consumer replaced a cached argument")
	}
	assertFlow()
}
