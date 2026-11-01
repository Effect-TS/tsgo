package etscheckerhooks_test

import (
	"strings"
	"testing"
	"testing/fstest"

	"github.com/effect-ts/tsgo/etscore"
	"github.com/microsoft/TypeScript/tsc/shim/bundled"
	"github.com/microsoft/TypeScript/tsc/shim/compiler"
	"github.com/microsoft/TypeScript/tsc/shim/core"
	"github.com/microsoft/TypeScript/tsc/shim/fourslash"
	"github.com/microsoft/TypeScript/tsc/shim/tsoptions"
	"github.com/microsoft/TypeScript/tsc/shim/tspath"
	"github.com/microsoft/TypeScript/tsc/shim/vfs/vfstest"
)

const captureTemplate = "export {};\ndeclare function capture<const Parts extends readonly string[]>(strings: Parts, ...values: unknown[]): Parts;\n"

func TestLiteralTemplateStringsInference(t *testing.T) {
	t.Parallel()
	source := captureTemplate +
		"const plain = capture`SELECT id FROM users`;\n" +
		"const plainText: 'SELECT id FROM users' = plain[0];\n" +
		"const plainLength: 1 = plain.length;\n" +
		"const parts = capture`SELECT id\\nFROM users WHERE age >= ${18} LIMIT ${2}`;\n" +
		"const head: 'SELECT id\\nFROM users WHERE age >= ' = parts[0];\n" +
		"const middle: ' LIMIT ' = parts[1];\n" +
		"const tail: '' = parts[2];\n" +
		"const length: 3 = parts.length;\n" +
		"const compatible: TemplateStringsArray = parts;\n" +
		"const raw: string = parts.raw[0];\n" +
		"const ordinary: string = String.raw`raw ${1}`;\n"
	program := literalTemplateProgram(true, source)
	if diagnostics := program.GetSemanticDiagnostics(t.Context(), nil); len(diagnostics) != 0 {
		t.Fatalf("expected literal segments, got %v", diagnostics)
	}
}

func TestLiteralTemplateStringsOptIn(t *testing.T) {
	t.Parallel()
	program := literalTemplateProgram(false, captureTemplate+"const text: 'hello' = capture`hello`[0];")
	diagnostics := program.GetSemanticDiagnostics(t.Context(), nil)
	if len(diagnostics) != 1 || diagnostics[0].Code() != 2322 {
		t.Fatalf("expected ordinary widened template text without the opt-in, got %v", diagnostics)
	}
}

func TestLiteralTemplateStringsReadonlyAndSubstitutions(t *testing.T) {
	t.Parallel()
	source := captureTemplate +
		"const parts = capture`hello`;\n" +
		"parts[0] = 'hello';\n" +
		"declare function numeric(strings: readonly ['age >= ', ''], age: number): void;\n" +
		"numeric`age >= ${'wrong'}`;\n"
	program := literalTemplateProgram(true, source)
	diagnostics := program.GetSemanticDiagnostics(t.Context(), nil)
	if len(diagnostics) != 2 || (!strings.Contains(diagnostics[0].String(), "read-only") && !strings.Contains(diagnostics[0].String(), "only permits reading")) || !strings.Contains(diagnostics[1].String(), "number") {
		t.Fatalf("expected readonly and substitution type errors, got %v", diagnostics)
	}
}

func TestLiteralTemplateStringsInvalidEscape(t *testing.T) {
	t.Parallel()
	// Invalid escapes have undefined cooked values. Do not invent a string literal.
	source := captureTemplate +
		"const parts = capture`\\unicode`;\n" +
		"const compatible: TemplateStringsArray = parts;\n" +
		"const tuple: readonly [string] = parts;\n"
	program := literalTemplateProgram(true, source)
	diagnostics := program.GetSemanticDiagnostics(t.Context(), nil)
	if len(diagnostics) != 1 || diagnostics[0].Code() != 2741 {
		t.Fatalf("expected invalid escapes to retain the standard template type, got %v", diagnostics)
	}
}

func TestLiteralTemplateStringsHover(t *testing.T) {
	t.Parallel()
	content := `// @Filename: /tsconfig.json
{"compilerOptions":{"strict":true,"target":"ES2023","types":[],"plugins":[{"name":"@effect/language-service","diagnostics":false,"experimentalLiteralTemplateStrings":true}]},"files":["main.ts"]}
// @Filename: /main.ts
` + captureTemplate + "export const /*head*/head = capture`SELECT id FROM users WHERE age >= ${18}`[0];"
	f, done := fourslash.NewFourslash(t, nil, content)
	defer done()
	f.GoToFile(t, "/main.ts")
	f.VerifyDiagnostics(t, nil)
	f.VerifyQuickInfoAt(t, "head", `const head: "SELECT id FROM users WHERE age >= "`, "")
}

func literalTemplateProgram(enabled bool, source string) *compiler.Program {
	fs := bundled.WrapFS(vfstest.FromMap(map[string]any{
		"/src/main.ts": &fstest.MapFile{Data: []byte(source)},
	}, tspath.CaseSensitive))
	options := &core.CompilerOptions{
		Strict:              core.TSTrue,
		Target:              core.ScriptTargetES2023,
		SkipDefaultLibCheck: core.TSTrue,
		Effect: &etscore.EffectPluginOptions{
			ExperimentalLiteralTemplateStrings: enabled,
		},
	}
	return compiler.NewProgram(compiler.ProgramOptions{
		Config:         tsoptions.NewParsedCommandLine(options, []tspath.RootedFilePath{"/src/main.ts"}, nil, "/", tspath.CaseSensitive),
		Host:           compiler.NewCompilerHost(fs, bundled.LibPath(), nil, nil, nil),
		SingleThreaded: core.TSTrue,
	})
}
