package rewriter_test

import (
	"context"
	"testing"
	"testing/fstest"

	"github.com/effect-ts/tsgo/internal/bundledeffect"
	"github.com/effect-ts/tsgo/internal/rewriter"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/bundled"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/ls/change"
	"github.com/microsoft/TypeScript/tsc/shim/ls/lsconv"
	"github.com/microsoft/TypeScript/tsc/shim/ls/lsutil"
	"github.com/microsoft/TypeScript/tsc/shim/lsp/lsproto"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
)

func TestRemoveTrailingPipingFlowTransformation(t *testing.T) {
	t.Parallel()
	const prefix = `import { Effect, pipe } from "effect"
declare const source: Effect.Effect<number>
declare function unary(self: Effect.Effect<number>): Effect.Effect<number>
declare function dataLast(n: number, self: Effect.Effect<number>): Effect.Effect<number>
declare function dataLast(n: number): (self: Effect.Effect<number>) => Effect.Effect<number>
export const value = `
	for _, test := range []struct {
		name, source, expected string
		kind                   typeparser.TransformationKind
		remove, deletionsOnly  bool
	}{
		{
			name: "unary call", source: `unary(source)`, expected: `source`,
			kind: typeparser.TransformationKindCall, remove: true,
		},
		{
			name: "data last call", source: `dataLast(0, source.pipe(Effect.map((n) => n + 1)))`, expected: `source.pipe(Effect.map((n) => n + 1))`,
			kind: typeparser.TransformationKindDataLast, remove: true,
		},
		{
			name: "parenthesized curried call", source: `(Effect.as(0))(source.pipe(Effect.map((n) => n + 1)))`, expected: `source.pipe(Effect.map((n) => n + 1))`,
			kind: typeparser.TransformationKindCall, remove: true,
		},
		{
			name: "block comment after comma", source: "source.pipe(Effect.as(1), /* keep */ \t Effect.as(0))", expected: "source.pipe(Effect.as(1) /* keep */)",
			kind: typeparser.TransformationKindPipeable, remove: true, deletionsOnly: true,
		},
		{
			name: "block comment before comma", source: "source.pipe(Effect.as(1) /* keep */ \t, \t Effect.as(0))", expected: "source.pipe(Effect.as(1) /* keep */)",
			kind: typeparser.TransformationKindPipeable, remove: true, deletionsOnly: true,
		},
		{
			name: "comments on both sides", source: "source.pipe(Effect.as(1) /* before */ \t, /* after */ \t Effect.as(0))", expected: "source.pipe(Effect.as(1) /* before */ \t /* after */)",
			kind: typeparser.TransformationKindPipeable, remove: true, deletionsOnly: true,
		},
		{
			name: "line comment before comma", source: "source.pipe(Effect.as(1) // keep\n \t, \tEffect.as(0))", expected: "source.pipe(Effect.as(1) // keep\n)",
			kind: typeparser.TransformationKindPipeable, remove: true, deletionsOnly: true,
		},
		{
			name: "comments and carriage return", source: "pipe(source, Effect.as(1), // keep 😀\r\n\t Effect.as(0))", expected: "pipe(source, Effect.as(1) // keep 😀\r\n)",
			kind: typeparser.TransformationKindPipe, remove: true, deletionsOnly: true,
		},
		{
			name: "nonbreaking space", source: "source.pipe(Effect.as(1), /* keep */\u00a0 \tEffect.as(0))", expected: "source.pipe(Effect.as(1) /* keep */\u00a0)",
			kind: typeparser.TransformationKindPipeable, remove: true, deletionsOnly: true,
		},
		{
			name: "trailing comma", source: "source.pipe(Effect.as(1), /* keep */ Effect.as(0),)", expected: "source.pipe(Effect.as(1) /* keep */,)",
			kind: typeparser.TransformationKindPipeable, remove: true, deletionsOnly: true,
		},
		{
			name: "source spelling", source: `source.pipe(Effect.as('\u0061'), Effect.as('discard'))`, expected: `source.pipe(Effect.as('\u0061'))`,
			kind: typeparser.TransformationKindPipeable, remove: true, deletionsOnly: true,
		},
		{
			name: "direct source spelling", source: `Effect.as(Effect.succeed(/* keep */ '\u0061'), 0)`, expected: `Effect.succeed(/* keep */ '\u0061')`,
			kind: typeparser.TransformationKindDataFirst, remove: true,
		},
		{
			name: "missing separator", source: `source.pipe(Effect.as(1) /* keep */ Effect.as(0))`, kind: typeparser.TransformationKindPipeable,
		},
		{
			name: "optional unary call", source: `unary?.(source)`, kind: typeparser.TransformationKindCall,
		},
		{
			name: "optional curried factory", source: `(Effect.as?.(0))(source)`, kind: typeparser.TransformationKindCall,
		},
		{
			name: "optional method pipe", source: `source?.pipe(Effect.as(0))`, kind: typeparser.TransformationKindPipeable,
		},
		{
			name: "optional pipe application", source: `source.pipe?.(Effect.as(0))`, kind: typeparser.TransformationKindPipeable,
		},
		{
			name: "optional pipe operator", source: `source.pipe(Effect.as(1), Effect.as?.(0))`, kind: typeparser.TransformationKindPipeable,
		},
		{
			name: "unsupported Effect fn", source: `Effect.fn(function*() { return 1 }, Effect.as(0))`, kind: typeparser.TransformationKindEffectFn,
		},
		{
			name: "empty flow", source: `source`,
		},
	} {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			source := prefix + test.source
			fixture := newPipingFlowRewriteFixture(t, source)
			sf, flow, tracker, converters := fixture.sf, fixture.flow, fixture.tracker, fixture.converters
			if test.kind != "" && (flow.Transformations().Len() == 0 || flow.Transformations().At(flow.Transformations().Len()-1).Kind() != test.kind) {
				t.Fatalf("expected terminal transformation kind %s", test.kind)
			}
			if got := tracker.RemoveTrailingPipingFlowTransformation(sf, flow); got != test.remove {
				t.Fatalf("removal = %v, want %v", got, test.remove)
			}
			if tracker.RemoveTrailingPipingFlowTransformation(sf, nil) || tracker.RemoveTrailingPipingFlowTransformation(nil, flow) || (*rewriter.Tracker)(nil).RemoveTrailingPipingFlowTransformation(sf, flow) {
				t.Fatal("nil inputs must not remove transformations")
			}
			edits := tracker.GetChanges()[sf.FileName()]
			if !test.remove && len(edits) != 0 {
				t.Fatalf("unsupported removal produced %d edits", len(edits))
			}
			var changes []core.TextChange
			for _, edit := range edits {
				if test.deletionsOnly && edit.NewText != "" {
					t.Fatalf("pipe removal inserted text %q", edit.NewText)
				}
				span := lsconv.FromLSPRangeToOriginal(converters, sf, edit.Range)
				changes = append(changes, core.TextChange{TextRange: span, NewText: edit.NewText})
			}
			expected := source
			if test.remove {
				expected = prefix + test.expected
			}
			if got := core.ApplyBulkEdits(source, changes); got != expected {
				t.Fatalf("replacement = %q, want %q", got, expected)
			}
		})
	}
}

func TestReplacePipingFlowPrefix(t *testing.T) {
	t.Parallel()
	const prefix = `import { Effect, pipe } from "effect"
declare const source: Effect.Effect<number>
declare const other: Effect.Effect<number>
declare function unary(self: Effect.Effect<number>): Effect.Effect<number>
declare function dataLast(n: number, self: Effect.Effect<number>): Effect.Effect<number>
declare function dataLast(n: number): (self: Effect.Effect<number>) => Effect.Effect<number>
declare function withOptions(self: Effect.Effect<number>, options?: { readonly n: number }): Effect.Effect<number>
export const value = `
	for _, test := range []struct {
		name, source, expected string
		kind                   typeparser.TransformationKind
		count                  int
		replace                bool
	}{
		{name: "unary call", source: `unary(source)`, expected: `other`, kind: typeparser.TransformationKindCall, count: 1, replace: true},
		{name: "data-first call", source: `Effect.as(source, 1)`, expected: `other`, kind: typeparser.TransformationKindDataFirst, count: 1, replace: true},
		{name: "data-last call", source: `dataLast(1, source)`, expected: `other`, kind: typeparser.TransformationKindDataLast, count: 1, replace: true},
		{name: "call with optionals", source: `withOptions(source, { n: 1 })`, expected: `other`, kind: typeparser.TransformationKindCallWithOptionals, count: 1, replace: true},
		{name: "pipe prefix", source: `pipe(source, Effect.as(1), Effect.as(2))`, expected: `pipe(other, Effect.as(2))`, kind: typeparser.TransformationKindPipe, count: 1, replace: true},
		{name: "whole pipe", source: `pipe(source, Effect.as(1), Effect.as(2))`, expected: `other`, kind: typeparser.TransformationKindPipe, count: 2, replace: true},
		{name: "method pipe prefix", source: `source.pipe(Effect.as(1), Effect.as(2))`, expected: `other.pipe(Effect.as(2))`, kind: typeparser.TransformationKindPipeable, count: 1, replace: true},
		{name: "unsupported Effect fn", source: `Effect.fn(function*() { return 1 }, Effect.as(0))`, kind: typeparser.TransformationKindEffectFn, count: 1},
		{name: "count out of range", source: `unary(source)`, kind: typeparser.TransformationKindCall, count: 2},
	} {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			source := prefix + test.source
			fixture := newPipingFlowRewriteFixture(t, source)
			if fixture.flow.Transformations().At(0).Kind() != test.kind {
				t.Fatalf("expected first transformation kind %s, got %s", test.kind, fixture.flow.Transformations().At(0).Kind())
			}
			if got := fixture.tracker.ReplacePipingFlowPrefix(fixture.sf, fixture.flow, test.count, fixture.tracker.NewIdentifier("other")); got != test.replace {
				t.Fatalf("replacement = %v, want %v", got, test.replace)
			}
			expected := source
			if test.replace {
				expected = prefix + test.expected
			}
			if got := fixture.apply(t); got != expected {
				t.Fatalf("result = %q, want %q", got, expected)
			}
		})
	}
}

// pipingFlowRewriteFixture holds the longest piping flow at the initializer of a
// source's last declaration, and a tracker to rewrite it.
type pipingFlowRewriteFixture struct {
	source     string
	sf         *ast.SourceFile
	flow       *typeparser.PipingFlow
	tracker    *rewriter.Tracker
	converters *lsconv.Converters
}

func newPipingFlowRewriteFixture(t *testing.T, source string) *pipingFlowRewriteFixture {
	t.Helper()
	files := map[string]any{"/test.ts": &fstest.MapFile{Data: []byte(source)}}
	if err := bundledeffect.MountEffect(bundledeffect.EffectV4, files); err != nil {
		t.Fatal(err)
	}
	fs := bundled.WrapFS(vfstest.FromMap(files, tspath.CaseSensitive))
	options := &core.CompilerOptions{
		Target: core.ScriptTargetESNext, Module: core.ModuleKindNodeNext, ModuleResolution: core.ModuleResolutionKindNodeNext,
		Strict: core.TSTrue, SkipLibCheck: core.TSTrue,
	}
	program := compiler.NewProgram(compiler.ProgramOptions{
		Config:         tsoptions.NewParsedCommandLine(options, []tspath.RootedFilePath{"/test.ts"}, nil, "/", tspath.CaseSensitive),
		Host:           compiler.NewCompilerHost(fs, bundled.LibPath(), nil, nil, nil),
		SingleThreaded: core.TSTrue,
	})
	ctx := context.Background()
	c, done := program.GetTypeChecker(ctx)
	t.Cleanup(done)
	sf := program.GetSourceFile("/test.ts")
	declaration := sf.Statements.Nodes[len(sf.Statements.Nodes)-1].AsVariableStatement().DeclarationList.AsVariableDeclarationList().Declarations.Nodes[0].AsVariableDeclaration()
	flow := typeparser.NewTypeParser(program, c).LongestPipingFlowAt(declaration.Initializer, true)
	if flow == nil {
		t.Fatal("expected a parsed flow")
	}
	converters := lsconv.NewConverters(lsproto.PositionEncodingKindUTF16, func(tspath.RootedFilePath) *lsconv.LSPLineMap {
		return lsconv.ComputeLSPLineStarts(source)
	})
	tracker := rewriter.NewTracker(change.NewTracker(ctx, options, lsutil.GetDefaultFormatCodeSettings(), converters))
	return &pipingFlowRewriteFixture{source: source, sf: sf, flow: flow, tracker: tracker, converters: converters}
}

// apply returns the source with the tracker's edits applied.
func (f *pipingFlowRewriteFixture) apply(t *testing.T) string {
	t.Helper()
	var changes []core.TextChange
	for _, edit := range f.tracker.GetChanges()[f.sf.FileName()] {
		span := lsconv.FromLSPRangeToOriginal(f.converters, f.sf, edit.Range)
		changes = append(changes, core.TextChange{TextRange: span, NewText: edit.NewText})
	}
	return core.ApplyBulkEdits(f.source, changes)
}
