// `repoctl oxlint verify`: the end-to-end check of the Oxlint integration (the Validate Oxlint workflow). Every
// supported runtime of _packages/tsgo/upstream.json (each profile, plus each tsrs version) gets a project of its own
// with the exact npm packages installed, and the built `effect-tsgo` proxy commands (`effect-tsgo oxlint`,
// `effect-tsgo vp`, `effect-tsgo --tsrs-lint ...`) run on the fixture in testdata/tests/oxlint, which must report the
// Effect diagnostics. The proxy picks the packaged artifacts from the installed versions, so the platform package must
// hold them (`repoctl build artifact`, or the downloaded build jobs' artifacts with `repoctl packages assemble`).
import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { runCommandCaptureSplit, runCommandString } from "./process.ts"
import { readUpstream, type Upstream } from "./upstream.ts"

export class OxlintVerifyError extends Data.TaggedError("OxlintVerifyError")<{
  readonly reason: string
}> {
  get message(): string {
    return `Oxlint verification failed: ${this.reason}`
  }
}

// The type-aware linter a case runs: the packaged tsgolint of the installed oxlint-tsgolint, or the packaged tsrs, of
// `--tsrs-lint=<version>` (`flag`) or, without a flag, of the tsrs installed instead of oxlint-tsgolint.
export type OxlintVerifyLinter =
  | { readonly kind: "tsgolint"; readonly version: string }
  | { readonly kind: "tsrs"; readonly version: string; readonly flag?: string }

export interface OxlintVerifyCase {
  readonly name: string
  // The npm packages of the project, next to effect.
  readonly dependencies: Readonly<Record<string, string>>
  // `oxlint`: `effect-tsgo oxlint` with the fixture's .oxlintrc files; `vp`: `effect-tsgo vp lint` with a vite.config.ts.
  readonly runner: "oxlint" | "vp"
  // The oxlint version the project must resolve (through vite-plus for `vp`).
  readonly oxlint: string
  readonly linter: OxlintVerifyLinter
}

const vitePlusProfile = /^vite-plus@(.+)$/

// One case per runtime profile (a vite-plus profile installs vite-plus, any other its oxlint and oxlint-tsgolint) and
// one per tsrs version (`--tsrs-lint=<version>` on the latest oxlint). A vite-plus that pins tsrs rather than
// oxlint-tsgolint lints with the packaged build of that tsrs, with no flag. The `vite-plus` profile repeats the newest
// `vite-plus@<version>` one, so cases are deduplicated by name.
export const buildOxlintVerifyCases = (upstream: typeof Upstream.Type): ReadonlyArray<OxlintVerifyCase> => {
  const cases = new Map<string, OxlintVerifyCase>()
  const add = (verifyCase: OxlintVerifyCase) => {
    if (!cases.has(verifyCase.name)) cases.set(verifyCase.name, verifyCase)
  }
  for (const profile of upstream.profiles) {
    const oxlint = profile.dependencies.oxlint
    const tsgolint = profile.dependencies["oxlint-tsgolint"]
    const tsrs = profile.dependencies.tsrs
    if (oxlint === undefined) continue
    const linter: OxlintVerifyLinter | undefined = tsgolint !== undefined
      ? { kind: "tsgolint", version: tsgolint }
      : tsrs !== undefined
      ? { kind: "tsrs", version: tsrs }
      : undefined
    if (linter === undefined) continue
    const vitePlus = vitePlusProfile.exec(profile.name)?.[1]
    if (vitePlus !== undefined) {
      add({
        name: `vite-plus@${vitePlus} (oxlint ${oxlint}, ${linter.kind} ${linter.version})`,
        dependencies: { "vite-plus": vitePlus },
        runner: "vp",
        oxlint,
        linter
      })
    } else if (profile.name !== "vite-plus" && linter.kind === "tsgolint") {
      add({
        name: `oxlint@${oxlint} + oxlint-tsgolint@${linter.version}`,
        dependencies: { oxlint, "oxlint-tsgolint": linter.version },
        runner: "oxlint",
        oxlint,
        linter
      })
    }
  }
  const oxlint = upstream.tags.oxlint.latest
  for (const version of Object.keys(upstream.components.tsrs ?? {}).sort()) {
    add({
      name: `oxlint@${oxlint} + tsrs@${version}`,
      dependencies: { oxlint },
      runner: "oxlint",
      oxlint,
      linter: { kind: "tsrs", version, flag: `--tsrs-lint=${version}` }
    })
  }
  return [...cases.values()].sort((left, right) => Buffer.compare(Buffer.from(left.name), Buffer.from(right.name)))
}

// The oxlint, oxlint-tsgolint and tsrs versions of the manifest that no case runs: each built artifact is verified.
export const uncoveredOxlintComponents = (
  upstream: typeof Upstream.Type,
  cases: ReadonlyArray<OxlintVerifyCase>
): ReadonlyArray<string> => {
  const covered = new Set(cases.flatMap((verifyCase) => [
    `oxlint@${verifyCase.oxlint}`,
    `${verifyCase.linter.kind === "tsgolint" ? "oxlint-tsgolint" : "tsrs"}@${verifyCase.linter.version}`
  ]))
  return [
    ...Object.keys(upstream.components.oxlint).map((version) => `oxlint@${version}`),
    ...Object.keys(upstream.components["oxlint-tsgolint"]).map((version) => `oxlint-tsgolint@${version}`),
    ...Object.keys(upstream.components.tsrs ?? {}).map((version) => `tsrs@${version}`)
  ].filter((component) => !covered.has(component)).sort()
}

const fixtureFiles = [
  "diagnostic.ts",
  "disabled.ts",
  "related-diagnostic.ts",
  "strict-boolean-expressions-duplicates.ts",
  "tsconfig.json",
  ".oxlintrc.json"
] as const

const vitePlusConfig = `import { defineConfig } from "vite-plus"

export default defineConfig({
  lint: {
    plugins: ["effecttsgo"],
    options: { typeAware: true },
    rules: { "effecttsgo/floating-effect": "error" }
  }
})
`

interface ProxyResult {
  readonly exitCode: number
  readonly stdout: string
  readonly output: string
}

export interface OxlintVerifyCheck {
  readonly name: string
  // Undefined when the check passed.
  readonly failure: string | undefined
}

const floatingEffect = /effecttsgo\(floating-effect\)/

const excerpt = (result: ProxyResult) => {
  const output = result.output.trim()
  return output.length > 1500 ? `${output.slice(0, 1500)}...` : output
}

const expectExit = (result: ProxyResult, exitCode: number) =>
  result.exitCode === exitCode ? undefined : `exit code ${result.exitCode}, expected ${exitCode}:\n${excerpt(result)}`

const parseJson = (result: ProxyResult): unknown => {
  try {
    return JSON.parse(result.stdout)
  } catch {
    return undefined
  }
}

interface JsonDiagnostic {
  readonly code?: string
  readonly message?: string
  readonly labels?: ReadonlyArray<{ readonly label?: string; readonly span?: unknown }>
}

const jsonDiagnostics = (result: ProxyResult): ReadonlyArray<JsonDiagnostic> | undefined => {
  const value = parseJson(result)
  return typeof value === "object" && value !== null && "diagnostics" in value && Array.isArray(value.diagnostics)
    ? value.diagnostics as ReadonlyArray<JsonDiagnostic>
    : undefined
}

// The checks of an `effect-tsgo oxlint` project (the former testdata/tests/oxlint/smoke.mjs), as pure functions of the
// command results so that they are unit tested.
export const oxlintChecks = {
  rules: (result: ProxyResult) => {
    const exit = expectExit(result, 0)
    if (exit !== undefined) return exit
    const rules = parseJson(result)
    return Array.isArray(rules) &&
        rules.some((rule) => rule?.scope === "effecttsgo" && rule?.value === "floating-effect")
      ? undefined
      : "`--rules` does not list effecttsgo/floating-effect"
  },
  reports: (result: ProxyResult) =>
    expectExit(result, 1) ?? (floatingEffect.test(result.output) ? undefined : `no effecttsgo(floating-effect):\n${excerpt(result)}`),
  silent: (result: ProxyResult) =>
    expectExit(result, 0) ?? (floatingEffect.test(result.output) ? `unexpected effecttsgo(floating-effect)` : undefined),
  relatedLabel: (result: ProxyResult) => {
    const exit = expectExit(result, 1)
    if (exit !== undefined) return exit
    const diagnostic = jsonDiagnostics(result)?.find((item) =>
      item.code === "effecttsgo(missing-star-in-yield-effect-gen)"
    )
    if (diagnostic === undefined) return "missing-star-in-yield-effect-gen was not reported"
    const label = diagnostic.labels?.find((item) => item.label === "Inside this Effect generator.")
    if (label === undefined) return "the related diagnostic was not converted to a labeled range"
    const expected = { offset: 67, length: 8, line: 3, column: 35 }
    return JSON.stringify(label.span) === JSON.stringify(expected)
      ? undefined
      : `label span ${JSON.stringify(label.span)}, expected ${JSON.stringify(expected)}`
  },
  strictBoolean: (result: ProxyResult) => {
    const exit = expectExit(result, 1)
    if (exit !== undefined) return exit
    const diagnostics = (jsonDiagnostics(result) ?? []).filter((item) =>
      item.code === "effecttsgo(strict-boolean-expressions)"
    )
    if (diagnostics.length !== 2) return `${diagnostics.length} strict-boolean-expressions diagnostics, expected 2`
    const messages = diagnostics.map((item) => item.message).sort()
    const expected = [
      "Unexpected `undefined` type in condition, expected strictly a boolean instead.",
      "Unexpected `{ id: string; }` type in condition, expected strictly a boolean instead."
    ].sort()
    if (JSON.stringify(messages) !== JSON.stringify(expected)) return `messages ${JSON.stringify(messages)}`
    return JSON.stringify(diagnostics[0]!.labels?.[0]?.span) === JSON.stringify(diagnostics[1]!.labels?.[0]?.span)
      ? undefined
      : "the two diagnostics do not share a span"
  },
  // A patched tsrs reports `+effect-tsgo.<version>` in its version (_patches/tsrs/021-core-version-suffix.patch).
  patchedTsrs: (result: ProxyResult) =>
    expectExit(result, 0) ??
      (/\+effect-tsgo\./.test(result.output) ? undefined : `not the Effect-patched tsrs:\n${excerpt(result)}`),
  traceRefused: (result: ProxyResult) =>
    result.exitCode === 0
      ? "OXLINT_TSGOLINT_TRACE was accepted by the tsrs linter"
      : /OXLINT_TSGOLINT_TRACE is not supported by the tsrs linter/.test(result.output)
      ? undefined
      : `no OXLINT_TSGOLINT_TRACE refusal:\n${excerpt(result)}`
}

export interface OxlintVerifyResult {
  readonly name: string
  readonly checks: ReadonlyArray<OxlintVerifyCheck>
}

export const formatOxlintVerifySummary = (results: ReadonlyArray<OxlintVerifyResult>) => {
  const failed = results.flatMap((result) =>
    result.checks.filter((check) => check.failure !== undefined).map((check) => ({ result, check }))
  )
  return [
    "## Oxlint integration",
    "",
    "| Runtime | Checks | Result |",
    "| --- | --- | --- |",
    ...results.map((result) => {
      const failures = result.checks.filter((check) => check.failure !== undefined).length
      return `| ${result.name} | ${result.checks.length} | ${failures === 0 ? "passed" : `${failures} failed`} |`
    }),
    ...(failed.length === 0 ? [] : [
      "",
      "### Failures",
      "",
      ...failed.flatMap(({ check, result }) => [
        `**${result.name}: ${check.name}**`,
        "",
        "```",
        check.failure!,
        "```",
        ""
      ])
    ])
  ].join("\n")
}

const installedVersion = Effect.fnUntraced(function*(project: string, packageName: string, scope?: string) {
  const script = [
    "const { createRequire } = require('node:module');",
    "const [name, scope] = process.argv.slice(1);",
    "let require_ = createRequire(process.cwd() + '/noop.js');",
    "if (scope) require_ = createRequire(require_.resolve(scope + '/package.json'));",
    "try { console.log(require_(name + '/package.json').version) } catch { console.log('') }"
  ].join(" ")
  return (yield* runCommandString(process.execPath, project, ["-e", script, packageName, ...(scope === undefined ? [] : [scope])]))
    .trim()
})

const createProject = Effect.fnUntraced(function*(
  repositoryRoot: string,
  project: string,
  verifyCase: OxlintVerifyCase,
  effectVersion: string
) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const fixture = path.join(repositoryRoot, "testdata", "tests", "oxlint")
  yield* fs.makeDirectory(project, { recursive: true })
  for (const file of fixtureFiles) {
    yield* fs.copyFile(path.join(fixture, file), path.join(project, file))
  }
  const presets = path.join(repositoryRoot, "_packages", "tsgo", "oxlint-presets", "recommended.json")
  yield* fs.writeFileString(
    path.join(project, ".oxlintrc-recommended.json"),
    `${JSON.stringify({ extends: [presets] }, null, 2)}\n`
  )
  yield* fs.writeFileString(
    path.join(project, ".oxlintrc-recommended-override.json"),
    `${JSON.stringify({ extends: [presets], rules: { "effecttsgo/floating-effect": "off" } }, null, 2)}\n`
  )
  if (verifyCase.runner === "vp") {
    yield* fs.writeFileString(path.join(project, "vite.config.ts"), vitePlusConfig)
  }
  yield* fs.writeFileString(path.join(project, "package.json"), `${JSON.stringify({
    name: "effect-tsgo-oxlint-verify",
    private: true,
    type: "module",
    dependencies: { effect: effectVersion, ...verifyCase.dependencies }
  }, null, 2)}\n`)
  yield* runCommandString("npm", project, ["install", "--no-audit", "--no-fund", "--loglevel=error"])
})

const runCase = Effect.fnUntraced(function*(
  repositoryRoot: string,
  root: string,
  verifyCase: OxlintVerifyCase,
  index: number,
  effectVersion: string
) {
  const path = yield* Path.Path
  const project = path.join(root, String(index))
  const cli = path.join(repositoryRoot, "_packages", "tsgo", "dist", "effect-tsgo.cjs")
  const checks: Array<OxlintVerifyCheck> = []
  const created = yield* Effect.result(createProject(repositoryRoot, project, verifyCase, effectVersion))
  if (created._tag === "Failure") {
    return { name: verifyCase.name, checks: [{ name: "npm install", failure: created.failure.message }] }
  }

  // The project resolves the versions the case is about: a manifest that drifted from npm fails here.
  const scope = verifyCase.runner === "vp" ? "vite-plus" : undefined
  const oxlint = yield* installedVersion(project, "oxlint", scope)
  const linterPackage = verifyCase.linter.kind === "tsgolint" ? "oxlint-tsgolint" : "tsrs"
  const linter = verifyCase.linter.kind === "tsrs" && verifyCase.runner === "oxlint"
    ? verifyCase.linter.version
    : yield* installedVersion(project, linterPackage, scope)
  checks.push({
    name: "installed versions",
    failure: oxlint === verifyCase.oxlint && linter === verifyCase.linter.version
      ? undefined
      : `resolved oxlint ${oxlint || "none"} and ${linterPackage} ${linter || "none"}, expected ${verifyCase.oxlint} and ${verifyCase.linter.version}`
  })

  const flag = verifyCase.linter.kind === "tsrs" && verifyCase.linter.flag !== undefined ? [verifyCase.linter.flag] : []
  const run = (tool: "oxlint" | "vp", args: ReadonlyArray<string>, env?: Readonly<Record<string, string>>) =>
    runCommandCaptureSplit(process.execPath, project, [cli, ...flag, tool, ...args], env).pipe(
      Effect.map(({ exitCode, stderr, stdout }): ProxyResult => ({
        exitCode: Number(exitCode),
        stdout,
        output: `${stdout}\n${stderr}`
      }))
    )
  const check = <E, R>(
    name: string,
    result: Effect.Effect<ProxyResult, E, R>,
    assert: (result: ProxyResult) => string | undefined
  ) =>
    Effect.result(result).pipe(Effect.map((outcome) => {
      checks.push({
        name,
        failure: outcome._tag === "Failure" ? String(outcome.failure) : assert(outcome.success)
      })
    }))

  if (verifyCase.runner === "vp") {
    const tsrs = verifyCase.linter.kind === "tsrs"
    yield* check("effect-tsgo vp lint reports floating-effect", run("vp", ["lint", "diagnostic.ts"]), oxlintChecks.reports)
    yield* check("effect-tsgo vp lint on disabled.ts", run("vp", ["lint", "disabled.ts"]), oxlintChecks.silent)
    if (tsrs) {
      // TSRS_BINARY: every tsrs that vite-plus runs through tsrs's launcher is the packaged build.
      yield* check(
        "effect-tsgo vp exec tsrs runs the packaged tsrs",
        run("vp", ["exec", "tsrs", "--version"]),
        oxlintChecks.patchedTsrs
      )
    }
    // `effect-tsgo patch`, then vite-plus itself: the patched binding, and the patched tsrs that vite-plus lints with.
    const vp = path.join(project, "node_modules", "vite-plus", "bin", "vp")
    const plain = (args: ReadonlyArray<string>) =>
      runCommandCaptureSplit(process.execPath, project, [vp, ...args]).pipe(
        Effect.map(({ exitCode, stderr, stdout }): ProxyResult => ({
          exitCode: Number(exitCode),
          stdout,
          output: `${stdout}\n${stderr}`
        }))
      )
    yield* check(
      `effect-tsgo patch --oxlint${tsrs ? " --tsrs" : ""}`,
      runCommandCaptureSplit(process.execPath, project, [cli, "patch", "--no-typescript", "--oxlint", ...(tsrs ? ["--tsrs"] : [])])
        .pipe(Effect.map(({ exitCode, stderr, stdout }): ProxyResult => ({
          exitCode: Number(exitCode),
          stdout,
          output: `${stdout}\n${stderr}`
        }))),
      (result) => expectExit(result, 0)
    )
    yield* check("patched vp lint reports floating-effect", plain(["lint", "diagnostic.ts"]), oxlintChecks.reports)
    yield* check("patched vp lint on disabled.ts", plain(["lint", "disabled.ts"]), oxlintChecks.silent)
    if (tsrs) {
      yield* check("patched vp exec tsrs runs the patched tsrs", plain(["exec", "tsrs", "--version"]), oxlintChecks.patchedTsrs)
    }
  } else {
    const config = ["--type-aware", "--config", ".oxlintrc.json"]
    yield* check("--rules lists the Effect rules", run("oxlint", ["--rules", "--format", "json"]), oxlintChecks.rules)
    yield* check("reports floating-effect", run("oxlint", [...config, "diagnostic.ts"]), oxlintChecks.reports)
    yield* check(
      "related diagnostic as a labeled range",
      run("oxlint", [...config, "--format", "json", "related-diagnostic.ts"]),
      oxlintChecks.relatedLabel
    )
    yield* check(
      "one strict-boolean-expressions diagnostic per type",
      run("oxlint", [...config, "--format", "json", "strict-boolean-expressions-duplicates.ts"]),
      oxlintChecks.strictBoolean
    )
    yield* check("disabled.ts reports nothing", run("oxlint", [...config, "disabled.ts"]), oxlintChecks.silent)
    yield* check(
      "recommended preset",
      run("oxlint", ["--config", ".oxlintrc-recommended.json", "diagnostic.ts"]),
      oxlintChecks.reports
    )
    yield* check(
      "recommended preset with an override",
      run("oxlint", ["--config", ".oxlintrc-recommended-override.json", "diagnostic.ts"]),
      oxlintChecks.silent
    )
    if (verifyCase.linter.kind === "tsgolint") {
      // vite-plus overwrites OXLINT_TSGOLINT_PATH with its stock tsgolint: the proxy's EFFECT_TSGO_TSGOLINT_PATH wins.
      yield* check(
        "EFFECT_TSGO_TSGOLINT_PATH wins over OXLINT_TSGOLINT_PATH",
        run("oxlint", [...config, "diagnostic.ts"], { OXLINT_TSGOLINT_PATH: path.join(project, "missing-tsgolint") }),
        oxlintChecks.reports
      )
    } else {
      // tsrs's linter has no Go profiling flags: the binding refuses them instead of passing them on.
      yield* check(
        "tsrs refuses OXLINT_TSGOLINT_TRACE",
        run("oxlint", [...config, "diagnostic.ts"], { OXLINT_TSGOLINT_TRACE: path.join(project, "trace.out") }),
        oxlintChecks.traceRefused
      )
    }
  }
  return { name: verifyCase.name, checks } satisfies OxlintVerifyResult
})

// The presets the package exports, as a project imports them.
const checkPackagePresets = Effect.fnUntraced(function*(repositoryRoot: string) {
  const path = yield* Path.Path
  const result = yield* runCommandCaptureSplit(process.execPath, path.join(repositoryRoot, "_packages", "tsgo"), [
    "--input-type=module",
    "--eval",
    `import { recommended, strict } from "@effect/tsgo/oxlint-presets";
     import recommendedJson from "@effect/tsgo/oxlint-presets/recommended.json" with { type: "json" };
     import strictJson from "@effect/tsgo/oxlint-presets/strict.json" with { type: "json" };
     if (recommended.rules["effecttsgo/floating-effect"] !== "error" || recommendedJson.rules["effecttsgo/floating-effect"] !== "error") process.exit(1);
     if (recommended.rules["effecttsgo/global-date"] !== undefined || recommendedJson.rules["effecttsgo/global-date"] !== undefined) process.exit(1);
     if (strict.rules["effecttsgo/catch-unfailable-effect"] !== "error" || strictJson.rules["effecttsgo/catch-unfailable-effect"] !== "error") process.exit(1);`
  ])
  return {
    name: "@effect/tsgo oxlint presets",
    checks: [{
      name: "presets export the expected rules",
      failure: Number(result.exitCode) === 0 ? undefined : `exit code ${result.exitCode}:\n${result.stderr.trim()}`
    }]
  } satisfies OxlintVerifyResult
})

export interface OxlintVerifyOptions {
  // Appends the markdown summary to this file (GITHUB_STEP_SUMMARY).
  readonly summary?: string
  // Runs only the cases whose name contains this text.
  readonly filter?: string
  // Keeps the projects instead of deleting them.
  readonly keep?: boolean
}

export const verifyOxlint = Effect.fnUntraced(function*(repositoryRoot: string, options: OxlintVerifyOptions = {}) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const upstream = yield* readUpstream(repositoryRoot)
  const cli = path.join(repositoryRoot, "_packages", "tsgo", "dist", "effect-tsgo.cjs")
  if (!(yield* fs.exists(cli))) {
    return yield* new OxlintVerifyError({ reason: `${cli} is missing: run \`pnpm --filter @effect/tsgo build\`` })
  }
  const allCases = buildOxlintVerifyCases(upstream)
  const uncovered = uncoveredOxlintComponents(upstream, allCases)
  if (uncovered.length > 0) {
    return yield* new OxlintVerifyError({ reason: `no case runs ${uncovered.join(", ")}` })
  }
  const cases = options.filter === undefined
    ? allCases
    : allCases.filter((verifyCase) => verifyCase.name.includes(options.filter!))
  const fixture = JSON.parse(
    yield* fs.readFileString(path.join(repositoryRoot, "testdata", "tests", "oxlint", "package.json"))
  ) as { readonly dependencies: { readonly effect: string } }
  const root = yield* Effect.promise(() => mkdtemp(join(tmpdir(), "effect-tsgo-oxlint-verify-")))
  yield* Console.log(`Verifying ${cases.length} Oxlint runtimes in ${root}`)
  const results = yield* Effect.forEach(
    cases,
    (verifyCase, index) =>
      runCase(repositoryRoot, root, verifyCase, index, fixture.dependencies.effect).pipe(
        Effect.tap((result) => {
          const failed = result.checks.filter((check) => check.failure !== undefined)
          return Console.log(`${failed.length === 0 ? "ok  " : "FAIL"} ${result.name} (${result.checks.length} checks)`)
        })
      ),
    { concurrency: 4 }
  ).pipe(Effect.ensuring(options.keep === true ? Effect.void : Effect.promise(() => rm(root, { recursive: true, force: true }))))
  const all = [yield* checkPackagePresets(repositoryRoot), ...results]
  const summary = formatOxlintVerifySummary(all)
  if (options.summary !== undefined) {
    yield* fs.writeFileString(options.summary, `${summary}\n`, { flag: "a" })
  }
  const failures = all.flatMap((result) =>
    result.checks.filter((check) => check.failure !== undefined).map((check) => `${result.name}: ${check.name}\n${check.failure}`)
  )
  if (failures.length > 0) {
    yield* Console.error(failures.join("\n\n"))
    return yield* new OxlintVerifyError({ reason: `${failures.length} check(s) failed` })
  }
  yield* Console.log(`Oxlint integration verified on ${cases.length} runtimes`)
})
