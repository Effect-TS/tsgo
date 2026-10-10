package rules

import (
	"context"
	"slices"
	"sort"
	"strings"
	"testing"
	"testing/fstest"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/effect-ts/tsgo/internal/rule"
	"github.com/effect-ts/tsgo/internal/typeparser"
	"github.com/microsoft/TypeScript/tsc/shim/ast"
	"github.com/microsoft/TypeScript/tsc/shim/bundled"
	"github.com/microsoft/TypeScript/tsc/shim/checker"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
)

func compileApiStabilitySource(t testing.TB, source string) (*rule.Context, *ast.SourceFile, func()) {
	t.Helper()
	return compileApiStabilitySourcePrimed(t, source, true)
}

// compileApiStabilitySourcePrimed optionally skips the full type-check, so the
// rules run against a cold checker.
func compileApiStabilitySourcePrimed(t testing.TB, source string, prime bool) (*rule.Context, *ast.SourceFile, func()) {
	t.Helper()

	testfs := map[string]any{
		"/.src/test.ts": &fstest.MapFile{Data: []byte(source)},
	}
	fs := vfstest.FromMap(testfs, tspath.CaseSensitive)
	fs = bundled.WrapFS(fs)

	compilerOptions := &core.CompilerOptions{
		NewLine:             core.NewLineKindLF,
		SkipDefaultLibCheck: core.TSTrue,
		SkipLibCheck:        core.TSTrue,
		NoErrorTruncation:   core.TSTrue,
		Target:              core.ScriptTargetESNext,
		Module:              core.ModuleKindNodeNext,
		ModuleResolution:    core.ModuleResolutionKindNodeNext,
		Strict:              core.TSTrue,
	}
	host := compiler.NewCompilerHost(fs, bundled.LibPath(), nil, nil, nil)
	program := compiler.NewProgram(compiler.ProgramOptions{
		Config:         tsoptions.NewParsedCommandLine(compilerOptions, []tspath.RootedFilePath{"/.src/test.ts"}, nil, "/.src", tspath.CaseSensitive),
		Host:           host,
		SingleThreaded: core.TSTrue,
	})

	ctx := context.Background()
	if prime {
		_ = program.GetSemanticDiagnostics(ctx, nil)
	}
	c, done := program.GetTypeChecker(ctx)
	sf := program.GetSourceFile("/.src/test.ts")
	if sf == nil {
		done()
		t.Fatal("failed to get source file")
	}
	tp := typeparser.NewTypeParser(program, c)
	ruleCtx := rule.NewContext(ctx, program, c, tp, sf, nil, etscore.SeverityWarning)
	return ruleCtx, sf, done
}

func TestApiStabilityLeakRule(t *testing.T) {
	t.Parallel()

	source := `/** @stability unstable */
export interface Unstable { value: string }
/** @stability experimental */
export interface Experimental { value: string }
export interface Container<T> { value: T }
export interface StableLeak { member: Unstable }
export interface StableOk { member: string }
export type StableGenericLeak = Container<Experimental>
export class PrivateLeak {
  private secret: Experimental
  protected guarded: Unstable
  public exposed: Unstable
}
/** @stability unstable */
export interface UnstableLeak { member: Experimental }
/** @stability unstable */
export interface UnstableOk { member: Unstable }
/** @stability experimental */
export interface ExperimentalOk { member: Unstable }
`

	ruleCtx, _, done := compileApiStabilitySource(t, source)
	defer done()

	diagnostics := ApiStabilityLeak.Run(ruleCtx)
	var messages []string
	for _, diagnostic := range diagnostics {
		messages = append(messages, diagnostic.String())
	}
	if len(diagnostics) != 4 {
		t.Fatalf("got %d diagnostics, want 4: %v", len(diagnostics), messages)
	}
	contains := func(want string) bool {
		return slices.ContainsFunc(messages, func(message string) bool { return strings.Contains(message, want) })
	}
	for _, want := range []string{
		"`StableLeak` exposes `Unstable`, an unstable API.",
		"`StableGenericLeak` exposes `Experimental`, an experimental API.",
		"`UnstableLeak` exposes `Experimental`, an experimental API.",
		"`PrivateLeak` exposes `Unstable`, an unstable API.",
	} {
		if !contains(want) {
			t.Errorf("missing diagnostic %q in %v", want, messages)
		}
	}
	if contains("`PrivateLeak` exposes `Experimental`") {
		t.Errorf("private member leaked into the public surface: %v", messages)
	}
}

// TestApiStabilityLeakComputedNames pins the production diagnostic path
// that previously panicked on a type literal with a computed member name. The
// rule must report the erased experimental aliases written in the computed
// property and method annotations without evaluating the computed name, and it
// must not change the compiler's own diagnostics.
func TestApiStabilityLeakComputedNames(t *testing.T) {
	t.Parallel()

	ruleCtx, done := compileApiStabilityFilesPrimed(t, map[string]string{"test.ts": `/** @stability experimental */
type PropertyAlias = string
/** @stability experimental */
type ParameterAlias = string
/** @stability experimental */
type MethodAlias = string
export declare const x: {
  [Symbol.iterator]?: PropertyAlias
  [Symbol.toStringTag](value: ParameterAlias): MethodAlias
  ["computed"]: PropertyAlias
}
export type Y = { readonly [Symbol.iterator]?: never }
export interface Z { readonly [Symbol.iterator]?: never }
`}, true)
	defer done()
	before := apiStabilityCompilerDiagnostics(ruleCtx)
	beforeInstantiation := ruleCtx.Checker.TotalInstantiationCount
	messages := runApiStabilityMessages(t, ruleCtx)
	after := apiStabilityCompilerDiagnostics(ruleCtx)
	if !slices.Equal(before, after) {
		t.Errorf("rule changed compiler diagnostics:\nbefore=%v\nafter=%v", before, after)
	}
	for _, diagnostic := range ruleCtx.Checker.GetDiagnostics(context.Background(), ruleCtx.SourceFile) {
		if diagnostic.Code() == 2589 {
			t.Errorf("rule introduced TS2589: %v", diagnostic.String())
		}
	}
	if delta := ruleCtx.Checker.TotalInstantiationCount - beforeInstantiation; delta > apiStabilityMaxInstantiationDelta {
		t.Errorf("rule instantiated %d types, want at most %d", delta, apiStabilityMaxInstantiationDelta)
	}
	for _, want := range []string{
		"`x` exposes `PropertyAlias`",
		"`x` exposes `ParameterAlias`",
		"`x` exposes `MethodAlias`",
	} {
		if !containsApiStabilityMessage(t, messages, want) {
			t.Errorf("missing %q in %v", want, messages)
		}
	}
}

// TestApiStabilityLeakExportOrderIsDeterministic pins the deterministic
// export visit order the rule and the namespace walker use. The checker returns
// module exports in Go map order, and the order in which the per-checker
// dependency caches are filled decides which surfaces a later analysis reuses;
// an unsorted visit therefore makes the reported findings vary between runs of
// the same binary over the same sources. The helper must keep returning the
// same name-ordered sequence and must never drop an export.
func TestApiStabilityLeakExportOrderIsDeterministic(t *testing.T) {
	t.Parallel()

	ruleCtx, done := compileApiStabilityFilesPrimed(t, map[string]string{"test.ts": `/** @stability experimental */
interface E { value: string }
/** @stability experimental */
type P = string
export declare const zeta: E
export declare const alpha: P
export interface Middle { value: E }
export declare const beta: { value: P }
export declare const omega: Middle
`}, true)
	defer done()

	module := checker.Checker_getSymbolOfDeclaration(ruleCtx.Checker, ruleCtx.SourceFile.AsNode())
	if module == nil {
		t.Fatal("module symbol missing")
	}
	all := ruleCtx.Checker.GetExportsOfModule(module)
	if len(all) < 5 {
		t.Fatalf("fixture should export several symbols, got %d", len(all))
	}
	wantNames := make([]string, 0, len(all))
	for _, symbol := range all {
		if symbol != nil {
			wantNames = append(wantNames, symbol.Name)
		}
	}
	slices.Sort(wantNames)

	var first []string
	for range 64 {
		ordered := sortedApiStabilityExports(ruleCtx, module)
		if len(ordered) != len(all) {
			t.Fatalf("export sorting dropped symbols: got %d, want %d", len(ordered), len(all))
		}
		names := make([]string, 0, len(ordered))
		for index, symbol := range ordered {
			if symbol == nil {
				t.Fatalf("export sorting produced a nil symbol at %d", index)
			}
			names = append(names, symbol.Name)
		}
		if !slices.IsSorted(names) {
			t.Fatalf("export order is not canonical: %v", names)
		}
		if first == nil {
			first = names
			continue
		}
		if !slices.Equal(first, names) {
			t.Fatalf("export order changed between invocations:\nfirst=%v\nlater=%v", first, names)
		}
	}
	if !slices.Equal(first, wantNames) {
		t.Fatalf("canonical order does not match the sorted export set:\ngot=%v\nwant=%v", first, wantNames)
	}
}

// TestApiStabilityLeakInternalExportsIgnored pins the export-level
// `@internal` exclusion: an export whose own declaration carries the tag is not
// checked, while its public peers still are. The tag is read from the parsed
// JSDoc (the same marker native TypeScript uses for `stripInternal` declaration
// emit), never from a raw substring of the surrounding prose.
func TestApiStabilityLeakInternalExportsIgnored(t *testing.T) {
	t.Parallel()

	experimentalE := `/** @stability experimental */
interface E { value: string }
declare const e: E
`

	tests := []apiStabilityRegressionCase{
		{
			name: "direct internal export is ignored while its public peer warns",
			files: map[string]string{"test.ts": experimentalE + `/** @internal */
export declare const hidden: E
export declare const shown: E
`},
			want:    []string{"`shown` exposes `E`"},
			notWant: []string{"`hidden` exposes"},
		},
		{
			name: "declaration attachment across declaration kinds",
			files: map[string]string{"test.ts": experimentalE + `/** @internal */
export const hiddenVar: E = e
/** @internal */
export declare function hiddenFunction(): E
/** @internal */
export interface HiddenInterface { value: E }
/** @internal */
export type HiddenType = { value: E }
/** @internal */
export declare class HiddenClass { value: E }
export declare const shown: E
`},
			want: []string{"`shown` exposes `E`"},
			notWant: []string{
				"`hiddenVar` exposes",
				"`hiddenFunction` exposes",
				"`HiddenInterface` exposes",
				"`HiddenType` exposes",
				"`HiddenClass` exposes",
			},
		},
		{
			name: "default function export attachment",
			files: map[string]string{"test.ts": `/** @stability experimental */
interface E { value: string }
/** @internal */
export default function hiddenDefault(): E {
  return undefined as unknown as E
}
`},
			notWant: []string{"`default` exposes"},
		},
		{
			name: "default assignment export attachment",
			files: map[string]string{"test.ts": experimentalE + `/** @internal */
export default e
`},
			notWant: []string{"`default` exposes"},
		},
		{
			name: "named forwarding tag on the declaration itself",
			files: map[string]string{
				"test.ts": `/** @internal */
export { Leaky } from "./dep"
`,
				"dep.ts": experimentalE + `export interface Leaky { value: E }
`,
			},
			notWant: []string{"`Leaky` exposes"},
		},
		{
			name: "untagged named forwarding of an internal target is skipped",
			files: map[string]string{
				"test.ts": `export { Hidden } from "./dep"
export { Shown } from "./dep"
`,
				"dep.ts": experimentalE + `/** @internal */
export interface Hidden { value: E }
export interface Shown { value: E }
`,
			},
			want:    []string{"`Shown` exposes `E`"},
			notWant: []string{"`Hidden` exposes"},
		},
		{
			name: "named forwarding tag covers every specifier in the declaration",
			files: map[string]string{
				"test.ts": `/** @internal */
export { LeakyOne, LeakyTwo } from "./dep"
`,
				"dep.ts": experimentalE + `export interface LeakyOne { value: E }
export interface LeakyTwo { value: E }
`,
			},
			notWant: []string{"`LeakyOne` exposes", "`LeakyTwo` exposes"},
		},
		{
			name: "specifier tag ignores only that specifier",
			files: map[string]string{
				"test.ts": `export {
  /** @internal */
  Hidden,
  Shown
} from "./dep"
`,
				"dep.ts": experimentalE + `export interface Hidden { value: E }
export interface Shown { value: E }
`,
			},
			want:    []string{"`Shown` exposes `E`"},
			notWant: []string{"`Hidden` exposes"},
		},
		{
			name: "star forwarding tag on the declaration itself",
			files: map[string]string{
				"test.ts": `/** @internal */
export * from "./dep"
`,
				"dep.ts": experimentalE + `export interface Leaky { value: E }
`,
			},
			notWant: []string{"`Leaky` exposes"},
		},
		{
			name: "untagged star forwarding of an internal target is skipped",
			files: map[string]string{
				"test.ts": `export * from "./dep"
`,
				"dep.ts": experimentalE + `/** @internal */
export interface Hidden { value: E }
export interface Shown { value: E }
`,
			},
			want:    []string{"`Shown` exposes `E`"},
			notWant: []string{"`Hidden` exposes"},
		},
		{
			name: "namespace forwarding tag on the declaration itself",
			files: map[string]string{
				"test.ts": `/** @internal */
export * as api from "./dep"
`,
				"dep.ts": experimentalE + `export interface Leaky { value: E }
`,
			},
			notWant: []string{"`api` exposes"},
		},
		{
			name: "namespace nested internal members are not walked",
			files: map[string]string{
				"test.ts": `export * as api from "./dep"
`,
				"dep.ts": `/** @stability experimental */
interface E { value: string }
/** @stability experimental */
interface F { value: string }
/** @internal */
export interface Hidden { value: E }
/** @internal @stability experimental */
export interface HiddenTagged { value: string }
export interface Shown { value: F }
`,
			},
			want:    []string{"`api` exposes `F`"},
			notWant: []string{"`api` exposes `E`", "`api` exposes `HiddenTagged`"},
		},
		{
			name: "alias chain reaching an internal target is skipped",
			files: map[string]string{
				"test.ts":   `export { Leaky } from "./barrel"`,
				"barrel.ts": `export { Leaky } from "./dep"`,
				"dep.ts":    experimentalE + "/** @internal */\nexport interface Leaky { value: E }\n",
			},
			notWant: []string{"`Leaky` exposes"},
		},
		{
			name: "internal tag on an intermediate forwarding declaration",
			files: map[string]string{
				"test.ts":   `export { Leaky } from "./barrel"`,
				"barrel.ts": "/** @internal */\nexport { Leaky } from \"./dep\"",
				"dep.ts":    experimentalE + "export interface Leaky { value: E }\n",
			},
			notWant: []string{"`Leaky` exposes"},
		},
		{
			name: "public overload stays checked when only the implementation is internal",
			files: map[string]string{"test.ts": experimentalE + `export function overloaded(x: E): E
export function overloaded(x: string): string
/** @internal */
export function overloaded(x: E | string): E | string {
  return x as E
}
`},
			want: []string{"`overloaded` exposes `E`"},
		},
		{
			name: "all overload declarations internal are skipped",
			files: map[string]string{"test.ts": experimentalE + `/** @internal */
export declare function gated(x: E): E
/** @internal */
export declare function gated(x: string): string
`},
			notWant: []string{"`gated` exposes"},
		},
		{
			name: "merged declarations stay public when one declaration is not internal",
			files: map[string]string{"test.ts": experimentalE + `/** @internal */
export interface Merged { a: E }
export interface Merged { b: string }
`},
			want: []string{"`Merged` exposes `E`"},
		},
		{
			name: "merged declarations all internal are skipped",
			files: map[string]string{"test.ts": experimentalE + `/** @internal */
export interface MergedHidden { a: E }
/** @internal */
export interface MergedHidden { b: string }
`},
			notWant: []string{"`MergedHidden` exposes"},
		},
		{
			name: "prose mentioning internalized or another tag is not a tag",
			files: map[string]string{"test.ts": experimentalE + `/** This representation is internalized and @internally tracked. */
export interface Prose { value: E }
`},
			want: []string{"`Prose` exposes `E`"},
		},
		{
			name: "public signature exposing an internal unstable type still warns",
			files: map[string]string{"test.ts": `/** @internal @stability experimental */
interface InternalE { value: string }
/** @internal @stability unstable */
export interface InternalU { value: string }
export interface PublicSurface { value: InternalE }
export interface PublicOther { value: InternalU }
`},
			want: []string{
				"`PublicSurface` exposes `InternalE`, an experimental API.",
				"`PublicOther` exposes `InternalU`, an unstable API.",
			},
		},
		{
			name: "internal export with a redundant cyclic star path stays skipped",
			files: map[string]string{
				"test.ts": experimentalE + `/** @internal */
export interface Cyclic { value: E }
export * from "./dep"
`,
				"dep.ts": `export * from "./test"
`,
			},
			notWant: []string{"`Cyclic` exposes"},
		},
		{
			name: "public star declaration behind a cycle stays checked",
			files: map[string]string{
				"test.ts": `export * from "./dep"
`,
				"dep.ts": `export * from "./test"
/** @stability experimental */
interface E { value: string }
export interface CyclicPublic { value: E }
`,
			},
			want: []string{"`CyclicPublic` exposes `E`"},
		},
		{
			name: "internal export visited first does not poison its public peer",
			files: map[string]string{"test.ts": experimentalE + `/** @internal */
export declare const aHidden: E
export declare const zShown: E
`},
			want:    []string{"`zShown` exposes `E`"},
			notWant: []string{"`aHidden` exposes"},
		},
		{
			name: "internal export visited last does not poison its public peer",
			files: map[string]string{"test.ts": experimentalE + `export declare const aShown: E
/** @internal */
export declare const zHidden: E
`},
			want:    []string{"`aShown` exposes `E`"},
			notWant: []string{"`zHidden` exposes"},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			assertApiStabilitySourcesParse(t, test.files)
			runApiStabilityRegressionCase(t, test)
		})
	}
}

// TestApiStabilityLeakExplicitExportPrecedence pins that:
// an explicit `@internal` export is the compiler's export winner for its name,
// so a redundant `export *` forward of the same name must not defeat the tag.
// Tagged statements, tagged specifiers, alias chains and cyclic redundant star
// paths all preserve the tag, while public explicit exports stay checked even
// when a redundant star forward is tagged internal.
func TestApiStabilityLeakExplicitExportPrecedence(t *testing.T) {
	t.Parallel()

	experimentalE := `/** @stability experimental */
interface E { value: string }
`
	leakyDep := experimentalE + "export interface Leaky { value: E }\n"

	tests := []apiStabilityRegressionCase{
		{
			// Reviewer reproduction: the tagged named export governs the name; the
			// redundant star forward resolves to the same public target but must
			// not re-enable the export.
			name: "tagged named export shadows redundant public star",
			files: map[string]string{
				"test.ts": "/** @internal */\nexport { Leaky } from \"./dep\"\nexport * from \"./dep\"\n",
				"dep.ts":  leakyDep,
			},
			notWant: []string{"`Leaky` exposes"},
		},
		{
			// Reviewer reproduction: a tagged local declaration is the explicit
			// winner and a cyclic star forward back through a barrel cannot
			// loosen it.
			name: "tagged local export shadows redundant public star",
			files: map[string]string{
				"test.ts": experimentalE + "/** @internal */\nexport interface Leaky { value: E }\nexport * from \"./dep\"\n",
				"dep.ts":  "export { Leaky } from \"./test\"\n",
			},
			notWant: []string{"`Leaky` exposes"},
		},
		{
			// A tag on the specifier itself governs, like a tag above the whole
			// export declaration.
			name: "tagged specifier shadows redundant public star",
			files: map[string]string{
				"test.ts": "export {\n  /** @internal */\n  Leaky\n} from \"./dep\"\nexport * from \"./dep\"\n",
				"dep.ts":  leakyDep,
			},
			notWant: []string{"`Leaky` exposes"},
		},
		{
			// The explicit path is tagged while the redundant star path reaches
			// the same name through a public barrel; only the explicit export is
			// selected, so the star path stays irrelevant.
			name: "tagged explicit export shadows redundant star alias chain",
			files: map[string]string{
				"test.ts":  "/** @internal */\nexport { Leaky } from \"./dep\"\nexport * from \"./other\"\n",
				"other.ts": "export * from \"./dep\"\n",
				"dep.ts":   leakyDep,
			},
			notWant: []string{"`Leaky` exposes"},
		},
		{
			// A public explicit declaration stays checked even when a redundant
			// star forward of the same name is tagged internal: the explicit
			// declaration is public, so the export is public.
			name: "public local export wins over redundant tagged star",
			files: map[string]string{
				"test.ts": experimentalE + "export interface Leaky { value: E }\n/** @internal */\nexport * from \"./dep\"\n",
				"dep.ts":  "export * from \"./test\"\n",
			},
			want: []string{"`Leaky` exposes `E`"},
		},
		{
			// A public explicit named forward stays checked even when a second
			// redundant tag claims the same name from another module.
			name: "public explicit forward wins over redundant tagged star",
			files: map[string]string{
				"test.ts":  "export { Leaky } from \"./dep\"\n/** @internal */\nexport * from \"./other\"\n",
				"other.ts": "export * from \"./dep\"\n",
				"dep.ts":   leakyDep,
			},
			want: []string{"`Leaky` exposes `E`"},
		},
		{
			// An explicitly tagged local declaration with an untagged redundant
			// star through another module stays internal as well.
			name: "tagged local export shadows redundant untagged star chain",
			files: map[string]string{
				"test.ts":  experimentalE + "/** @internal */\nexport interface Leaky { value: E }\nexport * from \"./other\"\n",
				"other.ts": "export * from \"./dep\"\n",
				"dep.ts":   "export { Leaky } from \"./test\"\n",
			},
			notWant: []string{"`Leaky` exposes"},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			assertApiStabilitySourcesParse(t, test.files)
			runApiStabilityRegressionCase(t, test)
		})
	}
}

// TestApiStabilityLeakDeclaredNamespaceMembers pins that:
// symbols declared by a namespace ModuleDeclaration filter their own
// `@internal` export members exactly like source-file namespace re-exports. A
// public member of the same namespace is still walked, and the namespace itself
// remains public.
func TestApiStabilityLeakDeclaredNamespaceMembers(t *testing.T) {
	t.Parallel()

	experimental := `/** @stability experimental */
interface E { value: string }
/** @stability experimental */
interface F { value: string }
`

	tests := []apiStabilityRegressionCase{
		{
			// Reviewer reproduction: the tagged member is not part of the
			// namespace's public API, so it cannot leak its dependency.
			name: "declared namespace filters tagged member",
			files: map[string]string{
				"test.ts": experimental + "export namespace Api {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: string }\n}\n",
			},
			notWant: []string{"`Api` exposes `E`"},
		},
		{
			// The namespace stays public and its untagged member is still
			// walked, so filtering does not blanket-suppress the export.
			name: "declared namespace checks public peer",
			files: map[string]string{
				"test.ts": experimental + "export namespace Api {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: F }\n}\n",
			},
			want:    []string{"`Api` exposes `F`"},
			notWant: []string{"`Api` exposes `E`"},
		},
		{
			// A public safe constant keeps the namespace public while the
			// internal leaking member does not taint it.
			name: "declared namespace public constant with tagged member",
			files: map[string]string{
				"test.ts": experimental + "export namespace Api {\n  export const safe = \"ok\"\n  /** @internal */\n  export interface Hidden { value: E }\n}\n",
			},
			notWant: []string{"`Api` exposes `E`"},
		},
		{
			// Merged namespace declarations contribute the same export table, so
			// a tag in one block filters the merged member.
			name: "merged namespace declarations filter tagged member",
			files: map[string]string{
				"test.ts": experimental + "export namespace Api {\n  /** @internal */\n  export interface Hidden { value: E }\n}\nexport namespace Api {\n  export interface Shown { value: F }\n}\n",
			},
			want:    []string{"`Api` exposes `F`"},
			notWant: []string{"`Api` exposes `E`"},
		},
		{
			// Dotted namespace declarations nest module declarations; the inner
			// module's members are filtered the same way.
			name: "dotted namespace filters tagged member",
			files: map[string]string{
				"test.ts": experimental + "export namespace Outer.Inner {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: F }\n}\n",
			},
			want:    []string{"`Outer` exposes `F`"},
			notWant: []string{"`Outer` exposes `E`"},
		},
		{
			// An ambient namespace is a ModuleDeclaration too; its tagged member
			// is filtered while the public peer still warns.
			name: "ambient namespace filters tagged member",
			files: map[string]string{
				"test.ts": experimental + "declare namespace Ambient {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: F }\n}\nexport { Ambient }\n",
			},
			want:    []string{"`Ambient` exposes `F`"},
			notWant: []string{"`Ambient` exposes `E`"},
		},
		{
			// Reviewer reproduction for forwarding: a module that re-exports a
			// namespace is walked through the namespace so its tagged member is
			// filtered.
			name: "forwarded declared namespace filters tagged member",
			files: map[string]string{
				"test.ts": "export * as api from \"./dep\"\n",
				"dep.ts":  experimental + "export namespace Nested {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: F }\n}\n",
			},
			want:    []string{"`api` exposes `F`"},
			notWant: []string{"`api` exposes `E`"},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			assertApiStabilitySourcesParse(t, test.files)
			runApiStabilityRegressionCase(t, test)
		})
	}
}

// TestApiStabilityLeakImportThenExportAliases pins that:
// an untagged import followed by a local export follows the named or default
// import target to its original declaration. A public target still warns, a
// merged target with a public declaration is not suppressed, and the import
// form used is the one written in the declaration: no other identifier is
// inferred and no type is read.
func TestApiStabilityLeakImportThenExportAliases(t *testing.T) {
	t.Parallel()

	experimentalE := `/** @stability experimental */
interface E { value: string }
declare const e: E
`

	tests := []apiStabilityRegressionCase{
		{
			// Reviewer reproduction: the named import target is tagged internal,
			// so the local re-export is internal too.
			name: "named import then export follows internal target",
			files: map[string]string{
				"test.ts": "import { Hidden } from \"./dep\"\nexport { Hidden }\n",
				"dep.ts":  experimentalE + "/** @internal */\nexport interface Hidden { value: E }\n",
			},
			notWant: []string{"`Hidden` exposes"},
		},
		{
			// Reviewer reproduction: a default import followed by export default
			// follows the ImportClause to the tagged original declaration.
			name: "default import then export default follows internal target",
			files: map[string]string{
				"test.ts": "import Hidden from \"./dep\"\nexport default Hidden\n",
				"dep.ts":  experimentalE + "/** @internal */\nexport default function hidden(): E { return null as unknown as E }\n",
			},
			notWant: []string{"`default` exposes"},
		},
		{
			name: "renamed import then export follows internal target",
			files: map[string]string{
				"test.ts": "import { Hidden as Alias } from \"./dep\"\nexport { Alias }\n",
				"dep.ts":  experimentalE + "/** @internal */\nexport interface Hidden { value: E }\n",
			},
			notWant: []string{"`Alias` exposes"},
		},
		{
			name: "named import then export default follows internal target",
			files: map[string]string{
				"test.ts": "import { Hidden } from \"./dep\"\nexport default Hidden\n",
				"dep.ts":  experimentalE + "/** @internal */\nexport function Hidden(): E { return null as unknown as E }\n",
			},
			notWant: []string{"`default` exposes"},
		},
		{
			name: "import then export through barrel follows internal target",
			files: map[string]string{
				"test.ts":   "import { Hidden } from \"./barrel\"\nexport { Hidden }\n",
				"barrel.ts": "export { Hidden } from \"./dep\"\n",
				"dep.ts":    experimentalE + "/** @internal */\nexport interface Hidden { value: E }\n",
			},
			notWant: []string{"`Hidden` exposes"},
		},
		{
			// The imported target is an untagged re-export of an internal local
			// declaration; following the import target chain keeps it internal.
			name: "import then export follows untagged target re-export",
			files: map[string]string{
				"test.ts": "import { Hidden } from \"./dep\"\nexport { Hidden }\n",
				"dep.ts":  experimentalE + "/** @internal */\ninterface Hidden { value: E }\nexport { Hidden }\n",
			},
			notWant: []string{"`Hidden` exposes"},
		},
		{
			// Direct public import and export still warns: only the imported
			// declaration's tag is inherited.
			name: "public named import then export still warns",
			files: map[string]string{
				"test.ts": "import { Shown } from \"./dep\"\nexport { Shown }\n",
				"dep.ts":  experimentalE + "export interface Shown { value: E }\n",
			},
			want: []string{"`Shown` exposes `E`"},
		},
		{
			name: "public default import then export default still warns",
			files: map[string]string{
				"test.ts": "import Shown from \"./dep\"\nexport default Shown\n",
				"dep.ts":  experimentalE + "export default function shown(): E { return null as unknown as E }\n",
			},
			want: []string{"`default` exposes `E`"},
		},
		{
			// A merged target with a public declaration is public, so the import
			// re-export is not suppressed by the tagged declaration.
			name: "merged import target with public declaration stays checked",
			files: map[string]string{
				"test.ts": "import { Merged } from \"./dep\"\nexport { Merged }\n",
				"dep.ts":  experimentalE + "/** @internal */\nexport interface Merged { a: E }\nexport interface Merged { b: string }\n",
			},
			want: []string{"`Merged` exposes `E`"},
		},
		{
			// A namespace import targets the module itself, not a tagged
			// declaration; its internal members are filtered like any namespace
			// re-export, so no warning is reported.
			name: "namespace import then export filters internal member",
			files: map[string]string{
				"test.ts": "import * as ns from \"./dep\"\nexport { ns }\n",
				"dep.ts":  experimentalE + "/** @internal */\nexport interface Hidden { value: E }\n",
			},
			notWant: []string{"`ns` exposes"},
		},
		{
			// An explicit tag on the local export statement still governs over
			// the followed import target.
			name: "import then tagged export statement stays internal",
			files: map[string]string{
				"test.ts": "import { Shown } from \"./dep\"\n/** @internal */\nexport { Shown }\n",
				"dep.ts":  experimentalE + "export interface Shown { value: E }\n",
			},
			notWant: []string{"`Shown` exposes"},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			assertApiStabilitySourcesParse(t, test.files)
			runApiStabilityRegressionCase(t, test)
		})
	}
}

// TestApiStabilityLeakExplicitStableTag pins the branch-facing explicit
// stable support at the leak-rule level: `@stability stable` is a real tag, so
// a stable root still enforces its ceiling, and an explicit stable forwarding
// tag tightens a forwarded experimental symbol (it is not treated as absent).
func TestApiStabilityLeakExplicitStableTag(t *testing.T) {
	t.Parallel()

	tests := []apiStabilityRegressionCase{
		{
			name:  "explicit stable root still enforces its ceiling",
			files: singleApiStabilityFile("/** @stability stable */\nexport declare const root: E"),
			want:  []string{"`root` exposes `E`"},
		},
		{
			name: "explicit stable forwarding overrides inherited experimental",
			files: map[string]string{
				"test.ts": "/** @stability stable */\nexport { Leaky } from \"./dep\"\n",
				"dep.ts":  "/** @stability experimental */\ninterface E { value: string }\n/** @stability experimental */\nexport interface Leaky { value: E }\n",
			},
			want: []string{"`Leaky` exposes `E`"},
		},
		{
			name: "explicit stable forwarding overrides inherited unstable",
			files: map[string]string{
				"test.ts": "/** @stability stable */\nexport { Leaky } from \"./dep\"\n",
				"dep.ts":  "/** @stability experimental */\ninterface E { value: string }\n/** @stability unstable */\nexport interface Leaky { value: E }\n",
			},
			want: []string{"`Leaky` exposes `E`"},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			assertApiStabilitySourcesParse(t, test.files)
			runApiStabilityRegressionCase(t, test)
		})
	}
}

// TestApiStabilityLeakVisibilityIsMetadataOnly pins the metadata boundary
// of the export-level visibility work: filtering an internal export never
// resolves a type, so it adds no checker instantiation and produces no global
// diagnostic. The public peer is still analyzed, and a repeated run over the
// same checker is identical, so the metadata decisions cannot poison a later
// analysis.
func TestApiStabilityLeakVisibilityIsMetadataOnly(t *testing.T) {
	t.Parallel()

	files := map[string]string{
		"test.ts": `/** @stability experimental */
interface E { value: string }
declare const e: E
/** @internal */
export declare const hidden: E
export declare const shown: E
`,
	}

	ruleCtx, done := compileApiStabilityFiles(t, files)
	defer done()

	module := checker.Checker_getSymbolOfDeclaration(ruleCtx.Checker, ruleCtx.SourceFile.AsNode())
	if module == nil {
		t.Fatal("missing module symbol")
	}

	before := ruleCtx.Checker.TotalInstantiationCount
	first := runApiStabilityMessages(t, ruleCtx)
	delta := ruleCtx.Checker.TotalInstantiationCount - before
	second := runApiStabilityMessages(t, ruleCtx)
	delta2 := ruleCtx.Checker.TotalInstantiationCount - before - delta
	globals := ruleCtx.Checker.GetGlobalDiagnostics()

	if delta != 0 {
		t.Errorf("visibility filtering instantiated %d types, want 0", delta)
	}
	if delta2 != 0 {
		t.Errorf("repeated run instantiated %d types, want 0", delta2)
	}
	if len(globals) != 0 {
		t.Errorf("visibility filtering produced global diagnostics: %v", globals)
	}
	if !slices.Equal(first, second) {
		t.Errorf("repeated runs disagree:\nfirst=%v\nsecond=%v", first, second)
	}
	if containsApiStabilityMessage(t, first, "`hidden` exposes") {
		t.Errorf("tagged export was checked: %v", first)
	}
	if !containsApiStabilityMessage(t, first, "`shown` exposes `E`") {
		t.Errorf("public peer lost its diagnostic: %v", first)
	}
}

// TestApiStabilityLeakExportVisibilityIsOrderIndependent runs the
// explicit-precedence, declared-namespace and import-alias fixtures in both
// declaration orders and requires the same diagnostics, so the corrected export
// selection cannot depend on the order in which exports are analyzed.
func TestApiStabilityLeakExportVisibilityIsOrderIndependent(t *testing.T) {
	t.Parallel()

	experimentalE := "/** @stability experimental */\ninterface E { value: string }\n"

	orders := []struct {
		name  string
		files map[string]string
	}{
		{
			name: "internal first",
			files: map[string]string{
				"test.ts": experimentalE + "/** @internal */\nexport interface Hidden { value: E }\nexport interface Shown { value: E }\nexport * from \"./dep\"\n",
				"dep.ts":  "export * from \"./test\"\n",
			},
		},
		{
			name: "public first",
			files: map[string]string{
				"test.ts": experimentalE + "export interface Shown { value: E }\n/** @internal */\nexport interface Hidden { value: E }\nexport * from \"./dep\"\n",
				"dep.ts":  "export * from \"./test\"\n",
			},
		},
	}

	for _, order := range orders {
		t.Run(order.name, func(t *testing.T) {
			t.Parallel()
			assertApiStabilitySourcesParse(t, order.files)
			ruleCtx, done := compileApiStabilityFiles(t, order.files)
			defer done()
			messages := runApiStabilityMessages(t, ruleCtx)
			if !containsApiStabilityMessage(t, messages, "`Shown` exposes `E`") {
				t.Errorf("public export lost its diagnostic: %v", messages)
			}
			if containsApiStabilityMessage(t, messages, "`Hidden` exposes") {
				t.Errorf("tagged export was checked: %v", messages)
			}
		})
	}
}

// TestApiStabilityLeakImportEqualsAliases pins that an
// `export import X = ...` alias follows the target named by its module
// reference exactly like a named or default import alias, so an untagged
// forwarding of an `@internal` target is not checked while a public target is,
// including when the target is reached through a namespace qualifier, a typed
// alias chain or an external `require`.
func TestApiStabilityLeakImportEqualsAliases(t *testing.T) {
	t.Parallel()

	experimentalE := `/** @stability experimental */
interface E { value: string }
`
	namespaceImpl := experimentalE + "namespace Impl {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: E }\n}\n"

	tests := []apiStabilityRegressionCase{
		{
			// Reviewer reproduction: the local namespace qualifier names the
			// tagged interface, so only the untagged peer stays checked.
			name: "local namespace import equals follows internal target",
			files: map[string]string{
				"test.ts": namespaceImpl + "export import Hidden = Impl.Hidden\nexport import Shown = Impl.Shown\n",
			},
			want:    []string{"`Shown` exposes `E`"},
			notWant: []string{"`Hidden` exposes"},
		},
		{
			// A tag written on the `export import` declaration itself governs
			// over the forwarded symbol, including when the target is public.
			name: "tagged import equals declaration stays internal",
			files: map[string]string{
				"test.ts": namespaceImpl + "/** @internal */\nexport import Hidden = Impl.Shown\n",
			},
			notWant: []string{"`Hidden` exposes"},
		},
		{
			// A named import of the namespace resolves the qualifier to the
			// namespace declared in the other file.
			name: "cross file namespace import equals follows internal target",
			files: map[string]string{
				"test.ts": "import { Impl } from \"./dep\"\nexport import Hidden = Impl.Hidden\nexport import Shown = Impl.Shown\n",
				"dep.ts":  experimentalE + "export namespace Impl {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: E }\n}\n",
			},
			want:    []string{"`Shown` exposes `E`"},
			notWant: []string{"`Hidden` exposes"},
		},
		{
			// A namespace import names the module; the target is read from the
			// module's export table.
			name: "namespace import equals follows internal member",
			files: map[string]string{
				"test.ts": "import * as Dep from \"./dep\"\nexport import Hidden = Dep.Hidden\nexport import Shown = Dep.Shown\n",
				"dep.ts":  experimentalE + "/** @internal */\nexport interface Hidden { value: E }\nexport interface Shown { value: E }\n",
			},
			want:    []string{"`Shown` exposes `E`"},
			notWant: []string{"`Hidden` exposes"},
		},
		{
			// A dotted namespace declaration nests its module; the innermost
			// qualifier is the module the target is filtered from.
			name: "dotted namespace import equals follows internal target",
			files: map[string]string{
				"test.ts": experimentalE + "export namespace Outer.Inner {\n  /** @internal */\n  export interface Hidden { value: E }\n  export interface Shown { value: E }\n}\nexport import Hidden = Outer.Inner.Hidden\nexport import Shown = Outer.Inner.Shown\n",
			},
			want:    []string{"`Shown` exposes `E`"},
			notWant: []string{"`Hidden` exposes"},
		},
		{
			// A local namespace alias chain keeps resolving through metadata:
			// the second alias names the same tagged target.
			name: "import equals alias chain follows internal target",
			files: map[string]string{
				"test.ts": namespaceImpl + "import Alias = Impl\nexport import Hidden = Alias.Hidden\nexport import Shown = Alias.Shown\n",
			},
			want:    []string{"`Shown` exposes `E`"},
			notWant: []string{"`Hidden` exposes"},
		},
		{
			// An external `require` alias exposes a member of the required
			// module's export table.
			name: "external require import equals follows internal member",
			files: map[string]string{
				"test.ts": "import Dep = require(\"./dep\")\nexport import Hidden = Dep.Hidden\nexport import Shown = Dep.Shown\n",
				"dep.ts":  experimentalE + "/** @internal */\nexport interface Hidden { value: E }\nexport interface Shown { value: E }\n",
			},
			want:    []string{"`Shown` exposes `E`"},
			notWant: []string{"`Hidden` exposes"},
		},
		{
			// A whole-module `require` alias is public; its tagged members are
			// filtered by the namespace walk, so the safe member does not warn.
			name: "whole module require filters internal member",
			files: map[string]string{
				"test.ts": "import Dep = require(\"./dep\")\nexport import Api = Dep\n",
				"dep.ts":  experimentalE + "/** @internal */\nexport interface Hidden { value: E }\nexport interface Safe { value: string }\n",
			},
			notWant: []string{"`Api` exposes `E`"},
		},
		{
			// A whole-module `require` alias still reports its public members.
			name: "whole module require checks public member",
			files: map[string]string{
				"test.ts": "import Dep = require(\"./dep\")\nexport import Api = Dep\n",
				"dep.ts":  experimentalE + "/** @internal */\nexport interface Hidden { value: E }\nexport interface Shown { value: E }\n",
			},
			want:    []string{"`Api` exposes `E`"},
			notWant: []string{"`Api` exposes `Shown`"},
		},
		{
			// A public target keeps warning through the alias, exactly like a
			// named import alias.
			name: "public import equals target stays checked",
			files: map[string]string{
				"test.ts": namespaceImpl + "export import Shown = Impl.Shown\nexport { Shown as NormalAlias }\n",
			},
			want: []string{"`Shown` exposes `E`", "`NormalAlias` exposes `E`"},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			assertApiStabilitySourcesParse(t, test.files)
			runApiStabilityRegressionCase(t, test)
		})
	}
}

// TestApiStabilityLeakImportEqualsIsMetadataOnly pins the metadata
// boundary of the import-equals path: deciding that the alias forwards an
// `@internal` target resolves no type, so it adds no checker instantiation,
// produces no global diagnostic, and a repeated run is identical. The public
// peer is still analyzed, so the metadata decisions cannot poison it.
func TestApiStabilityLeakImportEqualsIsMetadataOnly(t *testing.T) {
	t.Parallel()

	files := map[string]string{
		"test.ts": `/** @stability experimental */
interface E { value: string }
declare const e: E
namespace Impl {
  /** @internal */
  export interface Hidden { value: E }
  export interface Shown { value: E }
}
export import Hidden = Impl.Hidden
export import Shown = Impl.Shown
`,
	}

	ruleCtx, done := compileApiStabilityFiles(t, files)
	defer done()

	module := checker.Checker_getSymbolOfDeclaration(ruleCtx.Checker, ruleCtx.SourceFile.AsNode())
	if module == nil {
		t.Fatal("missing module symbol")
	}

	before := ruleCtx.Checker.TotalInstantiationCount
	first := runApiStabilityMessages(t, ruleCtx)
	delta := ruleCtx.Checker.TotalInstantiationCount - before
	second := runApiStabilityMessages(t, ruleCtx)
	delta2 := ruleCtx.Checker.TotalInstantiationCount - before - delta
	globals := ruleCtx.Checker.GetGlobalDiagnostics()

	if delta != 0 {
		t.Errorf("import-equals filtering instantiated %d types, want 0", delta)
	}
	if delta2 != 0 {
		t.Errorf("repeated run instantiated %d types, want 0", delta2)
	}
	if len(globals) != 0 {
		t.Errorf("import-equals filtering produced global diagnostics: %v", globals)
	}
	if !slices.Equal(first, second) {
		t.Errorf("repeated runs disagree:\nfirst=%v\nsecond=%v", first, second)
	}
	if containsApiStabilityMessage(t, first, "`Hidden` exposes") {
		t.Errorf("tagged export was checked: %v", first)
	}
	if !containsApiStabilityMessage(t, first, "`Shown` exposes `E`") {
		t.Errorf("public peer lost its diagnostic: %v", first)
	}
}

// TestApiStabilityLeakSurfaceRootTagSplit pins the rule-facing
// side of the declared-versus-surface split: an export's own declared
// tag governs its ceiling and is never reported as its own dependency, while
// the export's children keep being reported even when their root is tagged.
func TestApiStabilityLeakSurfaceRootTagSplit(t *testing.T) {
	t.Parallel()

	tests := []apiStabilityRegressionCase{
		{
			name:  "tagged function root reports its children but not itself",
			files: singleApiStabilityFile("/** @stability unstable */\nexport function tagged(x: string): E"),
			want:  []string{"`tagged` exposes `E`"},
			notWant: []string{
				"`tagged` exposes `tagged`",
			},
		},
		{
			name:    "tagged function root without a leak reports nothing",
			files:   singleApiStabilityFile("/** @stability unstable */\nexport function clean(x: string): string"),
			notWant: []string{"exposes"},
		},
		{
			name:  "tagged interface root reports its children but not itself",
			files: singleApiStabilityFile("/** @stability unstable */\nexport interface Root { member: E }"),
			want:  []string{"`Root` exposes `E`"},
			notWant: []string{
				"`Root` exposes `Root`",
			},
		},
		{
			name:  "tagged alias root reports its children but not itself",
			files: singleApiStabilityFile("/** @stability unstable */\nexport type Root = { member: E }"),
			want:  []string{"`Root` exposes `E`"},
			notWant: []string{
				"`Root` exposes `Root`",
			},
		},
		{
			name:  "tagged generic root keeps its constraint checked",
			files: singleApiStabilityFile("/** @stability unstable */\nexport declare function tagged<T extends E>(): T"),
			want:  []string{"`tagged` exposes `E`"},
			notWant: []string{
				"`tagged` exposes `tagged`",
			},
		},
		{
			name: "the same tagged type as root and as child",
			files: singleApiStabilityFile(`/** @stability experimental */
export interface Shared { value: string }
export interface Holder { shared: Shared }
`),
			want: []string{"`Holder` exposes `Shared`"},
			notWant: []string{
				"`Shared` exposes `Shared`",
			},
		},
		{
			name: "an explicit stable child boundary keeps generic arguments",
			files: singleApiStabilityFile(`/** @stability stable */
export interface StableBox<T> { value: T }
export declare const boxed: StableBox<E>
`),
			want: []string{"`boxed` exposes `E`"},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			assertApiStabilitySourcesParse(t, test.files)
			runApiStabilityRegressionCase(t, test)
		})
	}
}

// TestApiStabilityLeakSurfaceChildBoundaryDifferentials pins the
// rule-facing explicit child boundary: a child with an explicit declared tag
// contributes its declared level and its internals are not reported through the
// parent, while an untagged child keeps composing. Every exported tagged
// component is still audited as its own root, generic arguments of a tagged
// child stay exposed, and a same-named own override is never suppressed by a
// tagged base.
func TestApiStabilityLeakSurfaceChildBoundaryDifferentials(t *testing.T) {
	t.Parallel()

	tests := []apiStabilityRegressionCase{
		{
			name: "heritage: tagged stable base is a boundary",
			files: singleApiStabilityFile(`/** @stability stable */
export interface TaggedBase { secret: E }
export interface Leaky extends TaggedBase {}
`),
			want: []string{"`TaggedBase` exposes `E`"},
			notWant: []string{
				"`Leaky` exposes `E`",
				"`Leaky` exposes `TaggedBase`",
			},
		},
		{
			name: "heritage: tagged unstable base contributes its level only",
			files: singleApiStabilityFile(`/** @stability unstable */
export interface TaggedBase { secret: E }
export interface Leaky extends TaggedBase {}
`),
			want: []string{
				"`TaggedBase` exposes `E`",
				"`Leaky` exposes `TaggedBase`, an unstable API.",
			},
			notWant: []string{"`Leaky` exposes `E`"},
		},
		{
			name: "heritage control: untagged base is expanded",
			files: singleApiStabilityFile(`interface PlainBase { secret: E }
export interface PlainChild extends PlainBase {}
`),
			want: []string{"`PlainChild` exposes `E`"},
		},
		{
			name: "signature: tagged stable callable member is a boundary",
			files: singleApiStabilityFile(`/** @stability stable */
export interface TaggedCallable { (): E }
export interface Holder { callable: TaggedCallable }
`),
			want:    []string{"`TaggedCallable` exposes `E`"},
			notWant: []string{"`Holder` exposes `E`"},
		},
		{
			name: "signature control: untagged callable member is inspected",
			files: singleApiStabilityFile(`interface PlainCallable { (): E }
export interface Holder { callable: PlainCallable }
`),
			want: []string{"`Holder` exposes `E`"},
		},
		{
			name: "member: tagged member is a boundary for its anonymous type",
			files: singleApiStabilityFile(`export interface Holder {
  /** @stability stable */
  nested: { inner: E }
}
`),
			notWant: []string{"`Holder` exposes"},
		},
		{
			name: "member control: untagged member keeps the anonymous walk",
			files: singleApiStabilityFile(`export interface Holder {
  nested: { inner: E }
}
`),
			want: []string{"`Holder` exposes `E`"},
		},
		{
			name: "signature: tagged call signature is its own boundary",
			files: singleApiStabilityFile(`export interface Holder {
  /** @stability stable */
  (): E
}
`),
			notWant: []string{"`Holder` exposes"},
		},
		{
			name: "signature control: untagged call signature is walked",
			files: singleApiStabilityFile(`export interface Holder {
  (): E
}
`),
			want: []string{"`Holder` exposes `E`"},
		},
		{
			name: "index: tagged index signature is its own boundary",
			files: singleApiStabilityFile(`export interface Holder {
  /** @stability stable */
  [key: string]: E
}
`),
			notWant: []string{"`Holder` exposes"},
		},
		{
			name: "index control: untagged index signature is walked",
			files: singleApiStabilityFile(`export interface Holder {
  [key: string]: E
}
`),
			want: []string{"`Holder` exposes `E`"},
		},
		{
			name: "generic argument: tagged child argument stays exposed",
			files: singleApiStabilityFile(`/** @stability stable */
export interface StableBox<T> { value: T }
export interface BoxHolder { box: StableBox<E> }
`),
			want: []string{"`BoxHolder` exposes `E`"},
		},
		{
			name: "same component: tagged child boundary and root audit",
			files: singleApiStabilityFile(`/** @stability unstable */
export interface TaggedChild { secret: E }
export interface Holder { child: TaggedChild }
`),
			want: []string{
				"`TaggedChild` exposes `E`",
				"`Holder` exposes `TaggedChild`, an unstable API.",
			},
			notWant: []string{"`Holder` exposes `E`"},
		},
		{
			name: "deep heritage: untagged intermediate keeps the tagged argument",
			files: singleApiStabilityFile(`/** @stability stable */
export interface TaggedChain<T> { value: T }
export interface Mid<U> extends TaggedChain<U> {}
export interface Deep extends Mid<E> {}
`),
			want: []string{"`Deep` exposes `E`"},
		},
		{
			name: "heritage override: own member is not suppressed by a tagged base",
			files: singleApiStabilityFile(`/** @stability stable */
export interface TaggedBase { secret: E }
export interface Overriding extends TaggedBase { secret: E }
`),
			want:    []string{"`Overriding` exposes `E`"},
			notWant: []string{"`Overriding` exposes `TaggedBase`"},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			assertApiStabilitySourcesParse(t, test.files)
			runApiStabilityRegressionCase(t, test)
		})
	}
}

// TestApiStabilityLeakSurfaceFlattenedInheritanceDifferentials
// pins the rule-facing side of the fix2 correction: call, construct and index
// signatures the compiler flattens into a derived structured type keep their
// declaring base provenance, so an explicitly tagged base bounds those
// internals for derived declarations and exported values. Each tagged base is
// still audited as its own root, own constructor/call/index overrides stay
// reported, independently represented generic arguments stay exposed and
// untagged controls keep composing. Every case runs warm and cold; the cold
// harness additionally proves the rule never changes compiler diagnostics.
func TestApiStabilityLeakSurfaceFlattenedInheritanceDifferentials(t *testing.T) {
	t.Parallel()

	tests := []apiStabilityRegressionCase{
		{
			name: "inherited call signature from tagged base",
			files: singleApiStabilityFile(`/** @stability experimental */
interface FixCallE { value: string }
/** @stability unstable */
export interface FixTaggedCallableBase { (): FixCallE }
export interface FixDerivedCallableGeneric<T> extends FixTaggedCallableBase { own: T }
export declare const fixCallRef: FixDerivedCallableGeneric<string>
interface FixPlainCallableBase { (): FixCallE }
export interface FixPlainDerivedCallableGeneric<T> extends FixPlainCallableBase { own: T }
export declare const fixPlainCallRef: FixPlainDerivedCallableGeneric<string>
`),
			want: []string{
				"`FixTaggedCallableBase` exposes `FixCallE`",
				"`fixPlainCallRef` exposes `FixCallE`",
				"`FixDerivedCallableGeneric` exposes `FixTaggedCallableBase`, an unstable API.",
				"`fixCallRef` exposes `FixTaggedCallableBase`, an unstable API.",
			},
			notWant: []string{
				"`fixCallRef` exposes `FixCallE`",
				"`FixDerivedCallableGeneric` exposes `FixCallE`",
			},
		},
		{
			name: "inherited index signature from tagged base",
			files: singleApiStabilityFile(`/** @stability experimental */
interface FixIdxE { value: string }
/** @stability unstable */
export interface FixTaggedIndexBase { [key: string]: FixIdxE }
export interface FixDerivedIndexGeneric<T> extends FixTaggedIndexBase { own: T }
export declare const fixIndexRef: FixDerivedIndexGeneric<string>
interface FixPlainIndexBase { [key: string]: FixIdxE }
export interface FixPlainDerivedIndexGeneric<T> extends FixPlainIndexBase { own: T }
export declare const fixPlainIndexRef: FixPlainDerivedIndexGeneric<string>
`),
			want: []string{
				"`FixTaggedIndexBase` exposes `FixIdxE`",
				"`fixPlainIndexRef` exposes `FixIdxE`",
			},
			notWant: []string{
				"`fixIndexRef` exposes `FixIdxE`",
				"`FixDerivedIndexGeneric` exposes `FixIdxE`",
			},
		},
		{
			name: "inherited call construct and index signatures from one tagged base",
			files: singleApiStabilityFile(`/** @stability experimental */
interface FixComboCallE { value: string }
/** @stability experimental */
interface FixComboCtorE { value: string }
/** @stability experimental */
interface FixComboIdxE { value: string }
/** @stability unstable */
export interface FixComboBase {
  (): FixComboCallE
  new (): FixComboCtorE
  [key: string]: FixComboIdxE
}
export interface FixComboDerived<T> extends FixComboBase { own: T }
export declare const fixComboRef: FixComboDerived<string>
`),
			want: []string{
				"`FixComboBase` exposes `FixComboCallE`",
				"`FixComboBase` exposes `FixComboCtorE`",
				"`FixComboBase` exposes `FixComboIdxE`",
			},
			notWant: []string{
				"`fixComboRef` exposes `FixComboCallE`",
				"`fixComboRef` exposes `FixComboCtorE`",
				"`fixComboRef` exposes `FixComboIdxE`",
				"`FixComboDerived` exposes `FixComboCallE`",
				"`FixComboDerived` exposes `FixComboCtorE`",
				"`FixComboDerived` exposes `FixComboIdxE`",
			},
		},
		{
			name: "inherited constructor from tagged class",
			files: singleApiStabilityFile(`/** @stability experimental */
interface FixCtorE { value: string }
/** @stability experimental */
interface FixStaticE { value: string }
/** @stability experimental */
interface FixMethodE { value: string }
/** @stability unstable */
export class FixTaggedBaseClass {
  constructor(x: FixCtorE)
  static s: FixStaticE
  m(): FixMethodE
}
export class FixDerivedClass extends FixTaggedBaseClass {}
`),
			want: []string{
				"`FixTaggedBaseClass` exposes `FixCtorE`",
				"`FixTaggedBaseClass` exposes `FixStaticE`",
				"`FixTaggedBaseClass` exposes `FixMethodE`",
			},
			notWant: []string{
				"`FixDerivedClass` exposes `FixCtorE`",
				"`FixDerivedClass` exposes `FixStaticE`",
				"`FixDerivedClass` exposes `FixMethodE`",
			},
		},
		{
			name: "own derived constructor overrides the tagged base constructor",
			files: singleApiStabilityFile(`/** @stability experimental */
interface FixOwnCtorE { value: string }
/** @stability unstable */
export class FixOwnCtorBase { constructor(x: FixOwnCtorE) }
export class FixOwnCtorDerived extends FixOwnCtorBase {
  constructor(y: E) { super(y as unknown as FixOwnCtorE) }
}
`),
			want: []string{"`FixOwnCtorDerived` exposes `E`"},
			notWant: []string{
				"`FixOwnCtorDerived` exposes `FixOwnCtorE`",
			},
		},
		{
			name: "untagged intermediate chain ending in a tagged callable ancestor",
			files: singleApiStabilityFile(`/** @stability experimental */
interface FixDeepE { value: string }
/** @stability unstable */
export interface FixTaggedDeepCallable { (): FixDeepE }
export interface FixMidCallable<T> extends FixTaggedDeepCallable { mid: T }
export interface FixDeepCallable<T> extends FixMidCallable<T> {}
export declare const fixDeepCallRef: FixDeepCallable<string>
`),
			want: []string{"`FixTaggedDeepCallable` exposes `FixDeepE`"},
			notWant: []string{
				"`fixDeepCallRef` exposes `FixDeepE`",
				"`FixDeepCallable` exposes `FixDeepE`",
			},
		},
		{
			name: "explicit stable callable base stays quiet",
			files: singleApiStabilityFile(`/** @stability experimental */
interface FixStableE { value: string }
/** @stability stable */
export interface FixStableCallableBase { (): FixStableE }
export interface FixDerivedStableCallable extends FixStableCallableBase {}
export declare const fixStableRef: FixDerivedStableCallable
`),
			notWant: []string{
				"`FixDerivedStableCallable` exposes",
				"`fixStableRef` exposes",
			},
		},
		{
			name: "own call and index signatures override tagged base signatures",
			files: singleApiStabilityFile(`/** @stability experimental */
interface FixOverrideCallE { value: string }
/** @stability experimental */
interface FixOverrideIdxE { value: string }
/** @stability unstable */
export interface FixOverrideBase { (): FixOverrideCallE; [key: string]: FixOverrideIdxE }
export interface FixOverrideDerived extends FixOverrideBase {
  (): E
  [key: string]: E
}
`),
			want: []string{"`FixOverrideDerived` exposes `E`"},
			notWant: []string{
				"`FixOverrideDerived` exposes `FixOverrideCallE`",
				"`FixOverrideDerived` exposes `FixOverrideIdxE`",
			},
		},
		{
			name: "tagged ancestor generic argument stays exposed",
			files: singleApiStabilityFile(`/** @stability experimental */
interface FixBoxHiddenE { value: string }
/** @stability experimental */
interface FixBoxArgE { value: string }
/** @stability unstable */
export interface FixTaggedCallableBox<T> { (): FixBoxHiddenE; value: T }
export interface FixDerivedCallableBox<T> extends FixTaggedCallableBox<T> {}
export declare const fixCallableBoxRef: FixDerivedCallableBox<FixBoxArgE>
`),
			want: []string{
				"`fixCallableBoxRef` exposes `FixBoxArgE`",
				"`FixTaggedCallableBox` exposes `FixBoxHiddenE`",
			},
			notWant: []string{
				"`fixCallableBoxRef` exposes `FixBoxHiddenE`",
			},
		},
		{
			name: "blocked required argument of a tagged ancestor stays incomplete and keeps internals bounded",
			files: singleApiStabilityFile(`/** @stability experimental */
interface FixBlockedHiddenE { value: string }
/** @stability unstable */
export interface FixBlockedBox<T> { value: T; hidden: FixBlockedHiddenE }
export interface FixBlockedDerived<T, K extends keyof T> extends FixBlockedBox<T[K]> {}
`),
			notWant: []string{"`FixBlockedDerived` exposes `FixBlockedHiddenE`"},
		},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			t.Parallel()
			assertApiStabilitySourcesParse(t, test.files)
			runApiStabilityRegressionCase(t, test)
		})
	}
}

// TestApiStabilityLeakSurfaceSharedSnapshotReuse pins the
// rule-facing side of the shared caches: running the rule twice over
// one checker must report exactly the same diagnostics, because the second run
// composes the complete type and signature snapshots the first run published.
// A snapshot that belongs to a tagged component is only ever reached as that
// component's own root audit; as a child it contributes its declared level and
// never leaks the snapshot internals, in either export order.
func TestApiStabilityLeakSurfaceSharedSnapshotReuse(t *testing.T) {
	t.Parallel()

	files := singleApiStabilityFile(`/** @stability unstable */
export interface ATagged { leak: E }
export interface ZHolder { child: ATagged }
/** @stability unstable */
export interface ZTagged { leak: E }
export interface AHolder { child: ZTagged }
/** @stability stable */
export interface StableBox<T> { value: T }
export interface BoxHolder { box: StableBox<E> }
export declare function makeTagged(): ZTagged
`)
	ruleCtx, done := compileApiStabilityFiles(t, files)
	defer done()

	first := runApiStabilityMessages(t, ruleCtx)
	second := runApiStabilityMessages(t, ruleCtx)
	sortedFirst := slices.Clone(first)
	sortedSecond := slices.Clone(second)
	sort.Strings(sortedFirst)
	sort.Strings(sortedSecond)
	if !slices.Equal(sortedFirst, sortedSecond) {
		t.Fatalf("second run over the shared snapshots disagrees:\nfirst=%v\nsecond=%v", sortedFirst, sortedSecond)
	}

	for _, want := range []string{
		"`ATagged` exposes `E`",
		"`ZTagged` exposes `E`",
		"`ZHolder` exposes `ATagged`, an unstable API.",
		"`AHolder` exposes `ZTagged`, an unstable API.",
		"`BoxHolder` exposes `E`",
		"`makeTagged` exposes `ZTagged`, an unstable API.",
	} {
		if !containsApiStabilityMessage(t, first, want) {
			t.Errorf("missing diagnostic %q in %v", want, first)
		}
	}
	for _, notWant := range []string{
		"`ZHolder` exposes `E`",
		"`AHolder` exposes `E`",
		"`makeTagged` exposes `E`",
		"`ATagged` exposes `ATagged`",
		"`ZTagged` exposes `ZTagged`",
	} {
		if containsApiStabilityMessage(t, first, notWant) {
			t.Errorf("unexpected diagnostic %q in %v", notWant, first)
		}
	}
}
