#!/usr/bin/env node

import * as NodeRuntime from "@effect/platform-node/NodeRuntime"
import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import { Argument } from "effect/cli"
import { Command } from "effect/cli"
import { Flag } from "effect/cli"
import * as Option from "effect/Option"
import { fileURLToPath } from "node:url"
import { buildArtifact, buildCli, buildLocal, tsrsOptimizations, verifyReleaseArtifacts } from "./build.ts"
import { runChecks } from "./checks.ts"
import { addChangeset, publishChangeset, versionChangeset } from "./changesets.ts"
import {
  generateOxlintEffectRules,
  generateTsgolintIntegration,
  generateTypeScriptGoIntegration
} from "./codegen.ts"
import { ensureEffectFixtures } from "./fixtures.ts"
import { updateFlake } from "./flake.ts"
import { completeCheck, openPullRequestIfChanged } from "./github.ts"
import { runLint } from "./lint.ts"
import {
  printGeneratedMatrix,
  printOxlintTestMatrix,
  printPerfMatrix,
  printReleaseMatrix,
  printTypeScriptTestMatrix
} from "./matrix.ts"
import { comparePerformance } from "./perf.ts"
import { prepareWorkload, runPerfCi } from "./perfCi.ts"
import { writePerfReport } from "./perfReport.ts"
import { assembleReleaseArtifacts, bundleUpstream, preparePlatformPackages } from "./packages.ts"
import { prepareTsgolintComponent, validateOxlintComponent } from "./oxlint.ts"
import {
  portAck,
  portCheck,
  portCrate,
  portDeps,
  portDiff,
  portFixLines,
  portManifest,
  portReady,
  portStatus,
  portTodo
} from "./port.ts"
import { runRustChecks, runRustLint, runRustTests } from "./rust.ts"
import { cloneSubmodules, patchSubmodules } from "./submodules.ts"
import { runTests } from "./tests.ts"
import { readUpstream, resolveComponent, updateUpstream } from "./upstream.ts"
import { printUpstreamInfo } from "./upstreamResolve.ts"

const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url))

const setup = Command.make("setup", {
  component: Flag.Literals("component", ["typescript", "oxlint-tsgolint", "oxlint", "tsrs"]).pipe(
    Flag.withDefault("typescript"),
    Flag.withDescription(
      "Upstream component to set up; tsrs also sets up the TypeScript next compiler, for both the Go and the Rust build"
    )
  ),
  version: Flag.String("version").pipe(
    Flag.optional,
    Flag.withDescription("Component version; TypeScript defaults to the configured next tag, tsrs to its latest tag")
  )
}, ({ component, version }) => Effect.gen(function*() {
  const selected = yield* cloneSubmodules(repositoryRoot, component, Option.getOrUndefined(version))
  if (selected.name === "oxlint-tsgolint") {
    yield* prepareTsgolintComponent(repositoryRoot, {
      version: selected.version,
      gitHead: selected.gitHead,
      typescriptGitHead: selected.typescript.gitHead,
      compiler: selected.typescript.source
    })
    yield* generateTsgolintIntegration(repositoryRoot, selected.typescript.source)
  } else {
    if (selected.name === "oxlint") {
      yield* validateOxlintComponent(repositoryRoot, selected.version)
    }
    yield* patchSubmodules(repositoryRoot, selected.typescript.source)
    yield* generateTypeScriptGoIntegration(repositoryRoot, selected.typescript.source)
    if (selected.name === "oxlint") {
      yield* generateOxlintEffectRules(repositoryRoot)
    }
  }
  yield* ensureEffectFixtures(repositoryRoot)
})).pipe(
  Command.withDescription(
    "Check out and patch the submodules required by an upstream component (tsrs/ only with --component tsrs)"
  )
)

const submodules = Command.make("submodules").pipe(
  Command.withDescription("Manage repository submodules"),
  Command.withSubcommands([setup])
)

const codegenTsgolint = Command.make("tsgolint", {}, () => Effect.gen(function*() {
  const upstream = yield* readUpstream(repositoryRoot)
  const selected = yield* resolveComponent(
    repositoryRoot,
    upstream,
    "oxlint-tsgolint",
    upstream.tags["oxlint-tsgolint"].latest
  )
  yield* generateTsgolintIntegration(repositoryRoot, selected.typescript.source)
})).pipe(
  Command.withDescription("Generate the shared Go workspace and native tsgolint Effect rules")
)

const codegenOxlint = Command.make("oxlint", {}, () => generateOxlintEffectRules(repositoryRoot)).pipe(
  Command.withDescription("Generate and register built-in Oxlint Effect rules")
)

const codegen = Command.make("codegen").pipe(
  Command.withDescription("Generate repository integration code"),
  Command.withSubcommands([codegenOxlint, codegenTsgolint])
)

const test = Command.make("test", {}, () => runTests(repositoryRoot)).pipe(
  Command.withDescription("Run Go tests followed by CLI tests (the Rust tests: `repoctl tsrs test`)")
)

const check = Command.make("check", {}, () => runChecks(repositoryRoot)).pipe(
  Command.withDescription(
    "Check Go packages, the Rust port bookkeeping (`repoctl tsrs port-check`) and the CLI package"
  )
)

const lint = Command.make("lint", {}, () => runLint(repositoryRoot)).pipe(
  Command.withDescription("Run Go linters and dead-code analysis (the Rust lints: `repoctl tsrs lint`)")
)

const buildLocalCommand = Command.make("local", {}, () => buildLocal(repositoryRoot)).pipe(
  Command.withDescription(
    "Build the local Go binary, the CLI package and, with a tsrs checkout, cargo and CMake, the tsrs binary"
  )
)

const buildCliCommand = Command.make("cli", {}, () => buildCli(repositoryRoot)).pipe(
  Command.withDescription("Build the CLI package")
)

const buildArtifactCommand = Command.make("artifact", {
  component: Flag.Literals("component", ["typescript", "oxlint-tsgolint", "oxlint", "tsrs"]),
  version: Flag.String("version"),
  target: Flag.Literals("target", [
    "darwin-arm64",
    "darwin-x64",
    "win32-x64",
    "win32-arm64",
    "linux-x64",
    "linux-arm64",
    "linux-arm"
  ]),
  optimize: Flag.Literals("optimize", tsrsOptimizations).pipe(
    Flag.withDefault("none"),
    Flag.withDescription(
      "tsrs only: PGO (pgo), plus BOLT on Linux (pgo-bolt), as tsrs's release; the release matrix passes each target's"
        + " setting. Needs network, Node.js, pnpm and Go for the training"
    )
  )
}, ({ component, optimize, target, version }) => buildArtifact(repositoryRoot, component, version, target, optimize)).pipe(
  Command.withDescription("Build a versioned component artifact for a platform package")
)

const verifyReleaseBuildCommand = Command.make("verify-release", {}, () => verifyReleaseArtifacts(repositoryRoot)).pipe(
  Command.withDescription("Verify all release component artifacts in platform packages")
)

const build = Command.make("build").pipe(
  Command.withDescription("Build repository artifacts"),
  Command.withSubcommands([buildArtifactCommand, buildCliCommand, buildLocalCommand, verifyReleaseBuildCommand])
)

const changesetVersion = Command.make("version", {}, () => versionChangeset(repositoryRoot)).pipe(
  Command.withDescription("Apply changesets and update the generated Go version")
)

const changesetPublish = Command.make("publish", {}, () => publishChangeset(repositoryRoot)).pipe(
  Command.withDescription("Publish packages with Changesets")
)

const changesetAdd = Command.make("add", {
  description: Argument.String("description"),
  id: Flag.String("id"),
  packageName: Flag.String("package").pipe(Flag.withDefault("@effect/tsgo")),
  bump: Flag.Literals("bump", ["patch", "minor", "major"]).pipe(Flag.withDefault("patch"))
}, ({ bump, description, id, packageName }) =>
  addChangeset(repositoryRoot, id, packageName, bump, description)).pipe(
    Command.withDescription("Write a changeset file")
  )

const changeset = Command.make("changeset").pipe(
  Command.withDescription("Manage package versions and publishing"),
  Command.withSubcommands([changesetAdd, changesetVersion, changesetPublish])
)

const openPrIfChanged = Command.make("open-pr-if-changed", {
  base: Flag.String("base"),
  head: Flag.String("head").pipe(Flag.optional),
  headPrefix: Flag.String("head-prefix").pipe(Flag.optional),
  title: Flag.String("title"),
  body: Flag.String("body"),
  commitMessage: Flag.String("commit-message"),
  checks: Flag.KeyValuePair("check").pipe(Flag.optional)
}, ({ base, body, checks, commitMessage, head, headPrefix, title }) =>
  openPullRequestIfChanged(repositoryRoot, {
    base,
    body,
    checks: Option.getOrElse(checks, () => ({})),
    commitMessage,
    head: Option.getOrUndefined(head),
    headPrefix: Option.getOrUndefined(headPrefix),
    title
  })).pipe(
    Command.withDescription("Open or update a pull request when the generated repository tree changed")
  )

const completeGithubCheck = Command.make("complete-check", {
  checkId: Flag.String("check-id").pipe(Flag.withDefault("")),
  result: Flag.Literals("result", ["success", "failure", "cancelled"]),
  successMessage: Flag.String("success-message"),
  failureMessage: Flag.String("failure-message"),
  summary: Flag.String("summary")
}, ({ checkId, failureMessage, result, successMessage, summary }) =>
  completeCheck(repositoryRoot, {
    checkId,
    failureMessage,
    result,
    successMessage,
    summary
  })).pipe(
    Command.withDescription("Complete a GitHub check run with the supplied job result")
  )

const github = Command.make("github").pipe(
  Command.withDescription("Manage GitHub automation"),
  Command.withSubcommands([completeGithubCheck, openPrIfChanged])
)

const updateUpstreamCommand = Command.make("update", {}, () => updateUpstream(repositoryRoot)).pipe(
  Command.withDescription("Fetch and update moving upstream metadata (TypeScript, Oxlint, Vite+, tsrs) and the tsconfig schema")
)

const resolveUpstreamCommand = Command.make("resolve", {
  component: Flag.Literals("component", ["typescript", "oxlint-tsgolint", "oxlint", "tsrs"]).pipe(
    Flag.withDefault("typescript")
  ),
  version: Flag.String("version").pipe(Flag.optional),
  target: Flag.Literals("target", [
    "darwin-arm64",
    "darwin-x64",
    "win32-x64",
    "win32-arm64",
    "linux-x64",
    "linux-arm64",
    "linux-arm"
  ]).pipe(Flag.optional)
}, ({ component, target, version }) => printUpstreamInfo(
  repositoryRoot,
  component,
  Option.getOrUndefined(version),
  Option.getOrUndefined(target)
)).pipe(
  Command.withDescription("Resolve an upstream component and its platform build metadata")
)

const upstream = Command.make("upstream").pipe(
  Command.withDescription("Manage upstream metadata"),
  Command.withSubcommands([resolveUpstreamCommand, updateUpstreamCommand])
)

const updateFlakeCommand = Command.make("update", {}, () => updateFlake(repositoryRoot)).pipe(
  Command.withDescription("Synchronize Nix inputs and the Go vendor hash with the TypeScript next tag")
)

const flake = Command.make("flake").pipe(
  Command.withDescription("Manage the Nix flake"),
  Command.withSubcommands([updateFlakeCommand])
)

const perfCompare = Command.make("compare", {
  target: Argument.String("target"),
  version: Flag.String("version").pipe(Flag.optional),
  latest: Flag.Boolean("latest").pipe(Flag.withDefault(false)),
  output: Flag.String("output").pipe(Flag.optional),
  runId: Flag.String("run-id").pipe(Flag.optional),
  patchedBin: Flag.String("patched-bin").pipe(Flag.optional),
  stockBin: Flag.String("stock-bin").pipe(Flag.optional),
  config: Flag.String("config").pipe(Flag.withDefault("tsconfig.json")),
  runs: Flag.Int("runs").pipe(Flag.withDefault(1)),
  diagnosticsFlag: Flag.String("diagnostics-flag").pipe(Flag.withDefault("--diagnostics")),
  engine: Flag.Literals("engine", ["tsgo", "tsrs"]).pipe(
    Flag.withDefault("tsgo"),
    Flag.withDescription(
      "tsgo: stock vs patched TypeScript-Go. tsrs: stock tsrs (pinned commit, no _patches/tsrs, built with our " +
        "release profile under _tools/.perf-tsrs) vs the patched Rust binary"
    )
  ),
  checkerAssignment: Flag.String("checker-assignment").pipe(
    Flag.withDefault("go"),
    Flag.withDescription("tsrs only: TSRS_CHECKER_ASSIGNMENT (--checkerAssignment) for both binaries (empty to omit)")
  ),
  effectWorkload: Flag.Boolean("effect-workload").pipe(
    Flag.withDefault(false),
    Flag.withDescription("Fetch the target as perf CI's pinned Effect workload (if missing) and enable all rules")
  )
}, ({
  checkerAssignment,
  config,
  diagnosticsFlag,
  effectWorkload,
  engine,
  latest,
  output,
  patchedBin,
  runId,
  runs,
  stockBin,
  target,
  version
}) =>
  Effect.gen(function*() {
    if (effectWorkload) yield* prepareWorkload(target)
    yield* comparePerformance(repositoryRoot, {
      checkerAssignment,
      config,
      diagnosticsFlag,
      engine,
      output: Option.getOrUndefined(output),
      patchedBin: Option.getOrUndefined(patchedBin),
      version: Option.getOrUndefined(version),
      latest,
      runId: Option.getOrUndefined(runId),
      runs,
      stockBin: Option.getOrUndefined(stockBin),
      target
    })
  })).pipe(
    Command.withDescription("Compare stock and Effect-patched compilers (TypeScript-Go or tsrs) performance")
  )

const perfCi = Command.make("ci", {
  channel: Flag.String("channel"),
  side: Flag.Literals("side", ["base", "head"]),
  commit: Flag.String("commit"),
  version: Flag.String("version"),
  patchedBin: Flag.String("patched-bin"),
  workloadDir: Flag.String("workload-dir"),
  pairs: Flag.Int("pairs").pipe(Flag.withDefault(5)),
  output: Flag.String("output")
}, (options) => runPerfCi(options)).pipe(
  Command.withDescription("Measure stock and Effect-patched compilers on the pinned Effect workload with all rules")
)

const perfReport = Command.make("report", {
  input: Flag.String("input"),
  output: Flag.String("output"),
  matrix: Flag.String("matrix").pipe(
    Flag.optional,
    Flag.withDescription("Perf matrix JSON used to tell missing results apart from new or removed channels")
  )
}, ({ input, matrix, output }) => writePerfReport({ input, output, matrix: Option.getOrUndefined(matrix) })).pipe(
  Command.withDescription("Render a markdown report from base and head perf cells")
)

const perf = Command.make("perf").pipe(
  Command.withDescription("Run performance comparisons"),
  Command.withSubcommands([perfCi, perfCompare, perfReport])
)

const bundlePackageUpstream = Command.make("bundle-upstream", {}, () => bundleUpstream(repositoryRoot)).pipe(
  Command.withDescription("Copy upstream metadata into platform packages")
)

const preparePackages = Command.make("prepare", {}, () => preparePlatformPackages(repositoryRoot)).pipe(
  Command.withDescription("Prepare platform aliases and package manifests for publishing")
)

const assemblePackages = Command.make("assemble", {
  artifacts: Flag.String("artifacts").pipe(Flag.withDefault("_release-artifacts"))
}, ({ artifacts }) => assembleReleaseArtifacts(repositoryRoot, artifacts)).pipe(
  Command.withDescription("Assemble downloaded release artifacts into platform packages")
)

const packages = Command.make("packages").pipe(
  Command.withDescription("Prepare platform packages"),
  Command.withSubcommands([assemblePackages, bundlePackageUpstream, preparePackages])
)

const matrixTestTypeScript = Command.make("test-typescript", {}, () => printTypeScriptTestMatrix(repositoryRoot)).pipe(
  Command.withDescription("Print the TypeScript component test matrix as JSON")
)

const matrixTestOxlint = Command.make("test-oxlint", {}, () => printOxlintTestMatrix(repositoryRoot)).pipe(
  Command.withDescription("Print the Oxlint runtime test matrix as JSON")
)

const matrixGenerated = Command.make("generated", {}, () => printGeneratedMatrix(repositoryRoot)).pipe(
  Command.withDescription("Print the generated branch matrix as JSON")
)

const matrixPerf = Command.make("perf", {
  baseRef: Flag.String("base-ref"),
  headRef: Flag.String("head-ref")
}, ({ baseRef, headRef }) => printPerfMatrix(repositoryRoot, baseRef, headRef)).pipe(
  Command.withDescription("Print the base and head perf matrix as JSON")
)

const matrixRelease = Command.make("release", {}, () => printReleaseMatrix(repositoryRoot)).pipe(
  Command.withDescription("Print the platform release build matrix as JSON")
)

const matrix = Command.make("matrix").pipe(
  Command.withDescription("Generate CI matrices"),
  Command.withSubcommands([matrixGenerated, matrixPerf, matrixRelease, matrixTestOxlint, matrixTestTypeScript])
)

const portManifestCommand = Command.make("port-manifest", {
  write: Flag.Boolean("write").pipe(
    Flag.withDefault(false),
    Flag.withDescription("Write porting/manifest.tsv instead of printing it")
  )
}, ({ write }) => portManifest(repositoryRoot, write)).pipe(
  Command.withDescription("Regenerate the manifest of in-scope Go declarations (porting/scope.toml)")
)

const portStatusCommand = Command.make("port-status", {
  prefix: Argument.String("prefix").pipe(Argument.withDefault("")),
  files: Flag.Boolean("files").pipe(
    Flag.withDefault(false),
    Flag.withDescription("One row per Go file instead of per package")
  )
}, ({ files, prefix }) => portStatus(repositoryRoot, prefix, files)).pipe(
  Command.withDescription("Ported (or skipped) / total declarations and lines per package, code and tests separately")
)

const portTodoCommand = Command.make("port-todo", {
  prefix: Argument.String("prefix")
}, ({ prefix }) => portTodo(repositoryRoot, prefix)).pipe(
  Command.withDescription("Unported declarations under a path prefix, as origin markers ready to paste")
)

const portCheckCommand = Command.make("port-check", {
  summary: Flag.String("summary").pipe(
    Flag.optional,
    Flag.withDescription("Append a markdown progress table to this file (e.g. $GITHUB_STEP_SUMMARY)")
  )
}, ({ summary }) => portCheck(repositoryRoot, Option.getOrUndefined(summary))).pipe(
  Command.withDescription("Fail on a stale manifest, bad markers or skips, and Rust ports out of sync with Go")
)

const portFixLinesCommand = Command.make("port-fix-lines", {}, () => portFixLines(repositoryRoot)).pipe(
  Command.withDescription("Rewrite the Go line numbers of origin markers under crates/ from the manifest")
)

const portAckCommand = Command.make("port-ack", {
  declarations: Argument.String("declaration").pipe(
    Argument.variadic(),
    Argument.withDescription("Pairs of <go file path> <declaration name>, as in a marker")
  ),
  prefix: Flag.String("prefix").pipe(
    Flag.atLeast(0),
    Flag.withDescription("Ack every marked declaration whose Go path starts with this prefix (repeatable)")
  ),
  all: Flag.Boolean("all").pipe(
    Flag.withDefault(false),
    Flag.withDescription("Ack every marked declaration")
  ),
  prune: Flag.Boolean("prune").pipe(
    Flag.withDefault(false),
    Flag.withDescription("Drop porting/ported.tsv entries that have no marker")
  )
}, ({ all, declarations, prefix, prune }) => portAck(repositoryRoot, declarations, { all, prefixes: prefix, prune }))
  .pipe(
    Command.withDescription("Record the current Go hash of ported declarations in porting/ported.tsv")
  )

const portDiffCommand = Command.make("port-diff", {
  oldRef: Argument.String("old-ref"),
  newRef: Argument.String("new-ref")
}, ({ newRef, oldRef }) => portDiff(repositoryRoot, oldRef, newRef)).pipe(
  Command.withDescription("Added, removed, moved and changed in-scope Go declarations between two git refs")
)

const portCrateCommand = Command.make("port-crate", {
  packages: Argument.String("package").pipe(Argument.variadic({ min: 1 }))
}, ({ packages }) => portCrate(packages)).pipe(
  Command.withDescription("The Rust crate name of each Go package")
)

const portDepsCommand = Command.make("port-deps", {
  pkg: Argument.String("package")
}, ({ pkg }) => portDeps(repositoryRoot, pkg)).pipe(
  Command.withDescription("The in-scope Go packages a package imports, with their port status")
)

const portReadyCommand = Command.make("port-ready", {}, () => portReady(repositoryRoot)).pipe(
  Command.withDescription("Packages whose in-scope imports are all ported: what can be ported now")
)

const tsrsCheckCommand = Command.make("check", {}, () => runRustChecks(repositoryRoot)).pipe(
  Command.withDescription("Run cargo check on the Rust workspace")
)

const tsrsLintCommand = Command.make("lint", {}, () => runRustLint(repositoryRoot)).pipe(
  Command.withDescription("Run cargo fmt --check and Clippy (warnings denied) on the Rust workspace")
)

const tsrsTestCommand = Command.make("test", {}, () => runRustTests(repositoryRoot)).pipe(
  Command.withDescription("Run the Rust workspace tests, then build the tsrs binary and run its --version smoke test")
)

const tsrs = Command.make("tsrs").pipe(
  Command.withDescription(
    "The Rust port (crates/ on tsrs): cargo steps (they need `repoctl submodules setup --component tsrs`) and the port bookkeeping (porting/)"
  ),
  Command.withSubcommands([
    tsrsCheckCommand,
    tsrsLintCommand,
    portAckCommand,
    portCheckCommand,
    portCrateCommand,
    portDepsCommand,
    portDiffCommand,
    portFixLinesCommand,
    portManifestCommand,
    portReadyCommand,
    portStatusCommand,
    portTodoCommand,
    tsrsTestCommand
  ])
)

Command.make("repoctl").pipe(
  Command.withDescription("Effect TypeScript-Go repository maintenance"),
  Command.withSubcommands([
    submodules,
    build,
    changeset,
    check,
    codegen,
    flake,
    github,
    lint,
    matrix,
    packages,
    perf,
    test,
    tsrs,
    upstream
  ]),
  Command.run({ version: "0.0.0" }),
  Effect.provide(NodeServices.layer),
  NodeRuntime.runMain
)
