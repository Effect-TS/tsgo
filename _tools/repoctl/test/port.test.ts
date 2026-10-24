import assert from "node:assert/strict"
import { readFileSync } from "node:fs"
import { join } from "node:path"
import test from "node:test"
import { fileURLToPath } from "node:url"
import {
  ackPorted,
  buildManifest,
  checkPort,
  classifyPackage,
  crateFor,
  type Decl,
  diffManifests,
  fixMarkerLines,
  manifestText,
  type Marker,
  parseAckArguments,
  parseImports,
  parseManifest,
  parseMarkers,
  parseModulePath,
  parsePorted,
  parseScope,
  parseSkipped,
  portedText,
  progressBy,
  type RawDecl,
  type Scope
} from "../src/port.ts"

const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url))

const decl = (path: string, name: string, line: number, hash = "h", kind = "func"): Decl => ({
  path,
  name,
  kind,
  line,
  lines: 3,
  hash
})

const marker = (path: string, name: string, goLine: number, test = false): Marker => ({
  path,
  name,
  goLine,
  test,
  file: "crates/ets_x/src/lib.rs",
  line: 1
})

const baseScope: Scope = {
  include: ["etscore", "internal/..."],
  exclude: ["internal/skip", "shim/..."],
  excludeFiles: ["etscore/oxlint_test.go"],
  testPackages: ["internal/testonly"],
  tests: true,
  requireNew: false
}

test("names crates after Go packages", () => {
  assert.equal(crateFor("internal/rules"), "ets_rules")
  assert.equal(crateFor("internal/typeparser"), "ets_typeparser")
  assert.equal(crateFor("etscore"), "ets_etscore")
  assert.equal(crateFor("etsgoapi"), "ets_etsgoapi")
  assert.equal(crateFor("internal/Foo-bar/baz"), "ets_foo_bar_baz")
  assert.equal(crateFor("."), "ets_root")
})

test("parses the scope.toml subset", () => {
  const scope = parseScope([
    "# comment",
    "include = [",
    "  \"etscore\", # trailing comment",
    "  \"internal/...\",",
    "]",
    "exclude = [\"a#b\"]",
    "tests = false",
    "require_new = true"
  ].join("\n"))
  assert.deepEqual(scope.include, ["etscore", "internal/..."])
  assert.deepEqual(scope.exclude, ["a#b"])
  assert.deepEqual(scope.excludeFiles, [])
  assert.equal(scope.tests, false)
  assert.equal(scope.requireNew, true)
  assert.throws(() => parseScope("[table]\nx = 1"))
})

test("parses the committed scope.toml", () => {
  const scope = parseScope(readFileSync(join(repositoryRoot, "porting", "scope.toml"), "utf8"))
  assert.ok(scope.include.includes("internal/..."))
  assert.equal(classifyPackage(scope, "etsoxlintrunner"), "excluded")
  assert.equal(classifyPackage(scope, "internal/typeparser"), "included")
  assert.equal(scope.requireNew, true)
})

test("classifies packages, excludes winning over includes", () => {
  assert.equal(classifyPackage(baseScope, "etscore"), "included")
  assert.equal(classifyPackage(baseScope, "internal"), "included")
  assert.equal(classifyPackage(baseScope, "internal/rules"), "included")
  assert.equal(classifyPackage(baseScope, "internal/skip"), "excluded")
  assert.equal(classifyPackage(baseScope, "internal/skip/sub"), "included")
  assert.equal(classifyPackage(baseScope, "shim/ast"), "excluded")
  assert.equal(classifyPackage(baseScope, "shimmer"), "unclassified")
  assert.equal(classifyPackage(baseScope, "etscorex"), "unclassified")
})

test("builds the manifest from lister output", () => {
  const raw = (file: string, name: string, kind: string, extra: Partial<RawDecl> = {}): RawDecl => ({
    file,
    line: 10,
    endLine: 12,
    name,
    kind,
    generated: false,
    hash: "abc",
    ...extra
  })
  const decls = buildManifest([
    raw("etscore/a.go", "init", "func"),
    raw("etscore/a.go", "init", "func"),
    raw("etscore/gen.go", "Version", "const", { generated: true }),
    raw("etscore/oxlint_test.go", "TestOxlint", "test"),
    raw("internal/testonly/run.go", "Run", "func"),
    raw("etscore/a_test.go", "TestA", "test")
  ], baseScope)
  assert.deepEqual(decls.map((d) => `${d.path} ${d.name} ${d.kind} ${d.lines}`), [
    "etscore/a.go init func 3",
    "etscore/a.go init#2 func 3",
    "internal/testonly/run.go Run testhelper 3",
    "etscore/a_test.go TestA test 3"
  ])
  assert.equal(buildManifest([raw("etscore/a_test.go", "TestA", "test")], { ...baseScope, tests: false }).length, 0)
  assert.deepEqual(parseManifest(manifestText(decls)), decls)
})

test("parses origin markers", () => {
  const markers = parseMarkers([
    "// go: internal/typeparser/effect_type.go:36 TypeParser.EffectType",
    "pub fn effect_type() {}",
    "    // go test: internal/typeparser/effect_type_test.go:12 TestEffectType",
    "// Rust-only: no Go original",
    "// go (out of scope): etsoxlintrunner/runner.go:3 Run",
    "// go: not-a-go-file.rs:3 X",
    "// go: etscore/a.go:7 init#2"
  ].join("\n"), "crates/ets_typeparser/src/lib.rs")
  assert.deepEqual(markers, [
    {
      path: "internal/typeparser/effect_type.go",
      name: "TypeParser.EffectType",
      goLine: 36,
      test: false,
      file: "crates/ets_typeparser/src/lib.rs",
      line: 1
    },
    {
      path: "internal/typeparser/effect_type_test.go",
      name: "TestEffectType",
      goLine: 12,
      test: true,
      file: "crates/ets_typeparser/src/lib.rs",
      line: 3
    },
    { path: "etscore/a.go", name: "init#2", goLine: 7, test: false, file: "crates/ets_typeparser/src/lib.rs", line: 7 }
  ])
})

test("rewrites marker lines from the manifest", () => {
  const manifest = [decl("a/x.go", "F", 20), decl("a/x_test.go", "TestF", 5, "h", "test")]
  const text = [
    "// go: a/x.go:10 F",
    "fn f() {}",
    "  // go test: a/x_test.go:1 TestF",
    "// go: a/x.go:3 Unknown",
    "// go: a/x.go:20 F"
  ].join("\n")
  assert.equal(fixMarkerLines(text, manifest), [
    "// go: a/x.go:20 F",
    "fn f() {}",
    "  // go test: a/x_test.go:5 TestF",
    "// go: a/x.go:3 Unknown",
    "// go: a/x.go:20 F"
  ].join("\n"))
})

test("check accepts a consistent port", () => {
  const manifest = [decl("a/x.go", "F", 10, "h1"), decl("a/x.go", "G", 20, "h2")]
  assert.deepEqual(checkPort({
    manifest,
    markers: [marker("a/x.go", "F", 10)],
    skipped: new Map([["a/x.go G", "not needed"]]),
    ported: new Map([["a/x.go F", "h1"]]),
    requireNew: true
  }), [])
})

test("check reports every kind of inconsistency", () => {
  const manifest = [
    decl("a/x.go", "F", 10, "h1"),
    decl("a/x.go", "G", 20, "h2"),
    decl("a/x.go", "H", 30, "h3"),
    decl("a/x_test.go", "TestF", 5, "h4", "test"),
    decl("a/y.go", "Todo", 1, "h5")
  ]
  const errors = checkPort({
    manifest,
    markers: [
      marker("a/x.go", "F", 11),
      marker("a/x.go", "Missing", 1),
      marker("a/x.go", "G", 20),
      marker("a/x.go", "H", 30),
      marker("a/x_test.go", "TestF", 5)
    ],
    skipped: new Map([["a/x.go G", "r"], ["a/x.go Nope", "r"]]),
    ported: new Map([["a/x.go F", "h1"], ["a/x.go G", "old"], ["a/x.go Stale", "h"], ["a/x_test.go TestF", "h4"]]),
    requireNew: true
  })
  const expected = [
    "a/x.go F is at line 10, marker says 11 (run `repoctl tsrs port-fix-lines`)",
    "marker names 'a/x.go Missing', which is not in porting/manifest.tsv",
    "a/x_test.go TestF is a test declaration: write `// go test: a/x_test.go:5 TestF`",
    "porting/skipped.tsv: 'a/x.go G' is skipped but also has a marker",
    "porting/skipped.tsv: 'a/x.go Nope' is not in porting/manifest.tsv",
    "porting/ported.tsv: a/x.go G: Go declaration changed since it was ported: update the Rust port, then " +
    "`repoctl tsrs port-ack a/x.go G`",
    "porting/ported.tsv: a/x.go H is marked as ported but not recorded: run `repoctl tsrs port-ack a/x.go H`",
    "porting/ported.tsv: 'a/x.go Stale' has no marker under crates/ (run `repoctl tsrs port-ack --prune`)",
    "a/y.go:1: Todo is neither ported nor in porting/skipped.tsv (require_new = true)"
  ]
  for (const message of expected) {
    assert.ok(errors.some((error) => error.endsWith(message)), `missing: ${message}\nin:\n${errors.join("\n")}`)
  }
  assert.equal(errors.length, expected.length, errors.join("\n"))
  assert.equal(
    checkPort({ manifest, markers: [], skipped: new Map(), ported: new Map(), requireNew: false }).length,
    0
  )
})

test("ack records manifest hashes for marked declarations", () => {
  const manifest = [decl("a/x.go", "F", 10, "h1"), decl("a/x.go", "G", 20, "h2"), decl("b/y.go", "H", 1, "h3")]
  const markers = [marker("a/x.go", "F", 10), marker("a/x.go", "G", 20), marker("b/y.go", "H", 1)]
  const ported = new Map([["a/x.go F", "old"], ["gone.go X", "h"]])

  const explicit = ackPorted(ported, manifest, markers, { ids: ["a/x.go F"], prefixes: [], prune: false })
  assert.deepEqual(explicit.errors, [])
  assert.deepEqual([...explicit.ported], [["a/x.go F", "h1"], ["gone.go X", "h"]])

  const prefixed = ackPorted(ported, manifest, markers, { ids: [], prefixes: ["a/"], prune: true })
  assert.deepEqual(prefixed.acked, ["a/x.go F", "a/x.go G"])
  assert.deepEqual([...prefixed.ported], [["a/x.go F", "h1"], ["a/x.go G", "h2"]])

  const bad = ackPorted(ported, manifest, [marker("a/x.go", "F", 10)], {
    ids: ["a/x.go G", "a/x.go Nope"],
    prefixes: ["c/"],
    prune: false
  })
  assert.equal(bad.errors.length, 3)

  assert.deepEqual(parseAckArguments(["a/x.go", "F", "a/x.go:20", "G"]), ["a/x.go F", "a/x.go G"])
  assert.throws(() => parseAckArguments(["a/x.go"]))

  assert.deepEqual(parsePorted(portedText(prefixed.ported)), prefixed.ported)
})

test("parses skipped declarations", () => {
  const skipped = parseSkipped("# header\na/x.go\tF\tbecause\tof tabs\n\n")
  assert.deepEqual([...skipped], [["a/x.go F", "because\tof tabs"]])
})

test("reports progress for code and tests separately", () => {
  const manifest = [
    decl("a/x.go", "F", 1),
    decl("a/x.go", "G", 5),
    decl("a/x_test.go", "TestF", 1, "h", "test"),
    decl("b/y.go", "H", 1)
  ]
  const rows = progressBy(manifest, new Set(["a/x.go F", "a/x_test.go TestF"]), (d) => d.path.split("/")[0]!)
  assert.deepEqual(rows.get("a"), {
    code: 2,
    codeDone: 1,
    codeLines: 6,
    codeLinesDone: 3,
    test: 1,
    testDone: 1,
    testLines: 3,
    testLinesDone: 3
  })
  assert.equal(rows.get("b")!.codeDone, 0)
})

test("parses in-module imports", () => {
  const module = parseModulePath("module github.com/effect-ts/tsgo\n\ngo 1.27\n")
  assert.equal(module, "github.com/effect-ts/tsgo")
  const source = [
    "package rules",
    "",
    "import (",
    "\t\"strings\"",
    "",
    "\t\"github.com/effect-ts/tsgo/etscore\"",
    "\ttp \"github.com/effect-ts/tsgo/internal/typeparser\"",
    "\t\"github.com/microsoft/TypeScript/tsc/shim/ast\"",
    ")",
    "",
    "var x = \"github.com/effect-ts/tsgo/internal/notanimport\""
  ].join("\n")
  assert.deepEqual(parseImports(source, module), ["etscore", "internal/typeparser"])
  assert.deepEqual(parseImports("package x\nimport \"github.com/effect-ts/tsgo/etscore\"\n", module), ["etscore"])
})

test("diffs manifests into removed, moved, changed and added", () => {
  const old = [decl("a/x.go", "F", 1, "f"), decl("a/x.go", "G", 5, "g"), decl("a/x.go", "Gone", 9, "z")]
  const neu = [decl("a/x.go", "F", 2, "f2"), decl("a/y.go", "G", 1, "g"), decl("a/x.go", "New", 9, "n")]
  const diff = diffManifests(old, neu)
  assert.deepEqual(diff.removed.map((e) => e.statusId), ["a/x.go Gone"])
  assert.deepEqual(diff.moved.map((e) => [e.decl.path, e.from, e.statusId]), [["a/y.go", "a/x.go", "a/x.go G"]])
  assert.deepEqual(diff.changed.map((e) => e.statusId), ["a/x.go F"])
  assert.deepEqual(diff.added.map((e) => e.statusId), ["a/x.go New"])
})

test("committed manifest is well formed", () => {
  const manifest = parseManifest(readFileSync(join(repositoryRoot, "porting", "manifest.tsv"), "utf8"))
  assert.ok(manifest.length > 0)
  const ids = new Set(manifest.map((d) => `${d.path} ${d.name}`))
  assert.equal(ids.size, manifest.length)
  assert.ok(manifest.every((d) => d.line > 0 && d.lines > 0 && /^[0-9a-f]{16}$/.test(d.hash)))
  assert.ok(!manifest.some((d) => d.path.startsWith("etsoxlintrunner/") || d.path.startsWith("shim/")))
})
