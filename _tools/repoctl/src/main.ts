#!/usr/bin/env node

import * as NodeRuntime from "@effect/platform-node/NodeRuntime"
import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import * as Argument from "effect/unstable/cli/Argument"
import * as Command from "effect/unstable/cli/Command"
import * as Flag from "effect/unstable/cli/Flag"
import * as Option from "effect/Option"
import { fileURLToPath } from "node:url"
import { buildArtifact, buildCli, buildLocal, verifyReleaseArtifacts } from "./build.ts"
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
  printReleaseMatrix,
  printTypeScriptTestMatrix
} from "./matrix.ts"
import { comparePerformance } from "./perf.ts"
import { assembleReleaseArtifacts, bundleUpstream, preparePlatformPackages } from "./packages.ts"
import { prepareTsgolintComponent, validateOxlintComponent } from "./oxlint.ts"
import { cloneSubmodules, patchSubmodules } from "./submodules.ts"
import { runTests } from "./tests.ts"
import { updateUpstream } from "./upstream.ts"
import { printUpstreamInfo } from "./upstreamResolve.ts"

const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url))

const setup = Command.make("setup", {
  component: Flag.choice("component", ["typescript", "oxlint-tsgolint", "oxlint"]).pipe(
    Flag.withDefault("typescript"),
    Flag.withDescription("Upstream component to set up")
  ),
  version: Flag.string("version").pipe(
    Flag.optional,
    Flag.withDescription("Component version; TypeScript defaults to the configured next tag")
  )
}, ({ component, version }) => Effect.gen(function*() {
  const selected = yield* cloneSubmodules(repositoryRoot, component, Option.getOrUndefined(version))
  if (selected.name === "oxlint-tsgolint") {
    yield* prepareTsgolintComponent(repositoryRoot, {
      version: selected.version,
      gitHead: selected.gitHead,
      typescriptGitHead: selected.typescript.gitHead
    })
    yield* generateTsgolintIntegration(repositoryRoot)
  } else {
    if (selected.name === "oxlint") {
      yield* validateOxlintComponent(repositoryRoot, selected.version)
    }
    yield* patchSubmodules(repositoryRoot)
    yield* generateTypeScriptGoIntegration(repositoryRoot)
    if (selected.name === "oxlint") {
      yield* generateOxlintEffectRules(repositoryRoot)
    }
  }
  yield* ensureEffectFixtures(repositoryRoot)
})).pipe(
  Command.withDescription("Check out and patch the submodules required by an upstream component")
)

const submodules = Command.make("submodules").pipe(
  Command.withDescription("Manage repository submodules"),
  Command.withSubcommands([setup])
)

const codegenTsgolint = Command.make("tsgolint", {}, () => generateTsgolintIntegration(repositoryRoot)).pipe(
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
  Command.withDescription("Run Go tests followed by CLI tests")
)

const check = Command.make("check", {}, () => runChecks(repositoryRoot)).pipe(
  Command.withDescription("Check Go packages followed by the CLI package")
)

const lint = Command.make("lint", {}, () => runLint(repositoryRoot)).pipe(
  Command.withDescription("Run Go linters and dead-code analysis")
)

const buildLocalCommand = Command.make("local", {}, () => buildLocal(repositoryRoot)).pipe(
  Command.withDescription("Build the local Go binary and CLI package")
)

const buildCliCommand = Command.make("cli", {}, () => buildCli(repositoryRoot)).pipe(
  Command.withDescription("Build the CLI package")
)

const buildArtifactCommand = Command.make("artifact", {
  component: Flag.choice("component", ["typescript", "oxlint-tsgolint", "oxlint"]),
  version: Flag.string("version"),
  target: Flag.choice("target", [
    "darwin-arm64",
    "darwin-x64",
    "win32-x64",
    "win32-arm64",
    "linux-x64",
    "linux-arm64",
    "linux-arm"
  ])
}, ({ component, target, version }) => buildArtifact(repositoryRoot, component, version, target)).pipe(
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
  description: Argument.string("description"),
  id: Flag.string("id"),
  packageName: Flag.string("package").pipe(Flag.withDefault("@effect/tsgo")),
  bump: Flag.choice("bump", ["patch", "minor", "major"]).pipe(Flag.withDefault("patch"))
}, ({ bump, description, id, packageName }) =>
  addChangeset(repositoryRoot, id, packageName, bump, description)).pipe(
    Command.withDescription("Write a changeset file")
  )

const changeset = Command.make("changeset").pipe(
  Command.withDescription("Manage package versions and publishing"),
  Command.withSubcommands([changesetAdd, changesetVersion, changesetPublish])
)

const openPrIfChanged = Command.make("open-pr-if-changed", {
  base: Flag.string("base"),
  head: Flag.string("head").pipe(Flag.optional),
  headPrefix: Flag.string("head-prefix").pipe(Flag.optional),
  title: Flag.string("title"),
  body: Flag.string("body"),
  commitMessage: Flag.string("commit-message"),
  checks: Flag.keyValuePair("check").pipe(Flag.optional)
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
  checkId: Flag.string("check-id").pipe(Flag.withDefault("")),
  result: Flag.choice("result", ["success", "failure", "cancelled"]),
  successMessage: Flag.string("success-message"),
  failureMessage: Flag.string("failure-message"),
  summary: Flag.string("summary")
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
  Command.withDescription("Fetch and update moving upstream metadata and the tsconfig schema")
)

const resolveUpstreamCommand = Command.make("resolve", {
  component: Flag.choice("component", ["typescript", "oxlint-tsgolint", "oxlint"]).pipe(
    Flag.withDefault("typescript")
  ),
  version: Flag.string("version").pipe(Flag.optional),
  target: Flag.choice("target", [
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
  target: Argument.string("target"),
  version: Flag.string("version").pipe(Flag.optional),
  latest: Flag.boolean("latest"),
  output: Flag.string("output").pipe(Flag.optional),
  runId: Flag.string("run-id").pipe(Flag.optional),
  patchedBin: Flag.string("patched-bin").pipe(Flag.optional),
  stockBin: Flag.string("stock-bin").pipe(Flag.optional),
  config: Flag.string("config").pipe(Flag.withDefault("tsconfig.json")),
  runs: Flag.integer("runs").pipe(Flag.withDefault(1)),
  diagnosticsFlag: Flag.string("diagnostics-flag").pipe(Flag.withDefault("--diagnostics"))
}, ({ config, diagnosticsFlag, latest, output, patchedBin, runId, runs, stockBin, target, version }) =>
  comparePerformance(repositoryRoot, {
    config,
    diagnosticsFlag,
    output: Option.getOrUndefined(output),
    patchedBin: Option.getOrUndefined(patchedBin),
    version: Option.getOrUndefined(version),
    latest,
    runId: Option.getOrUndefined(runId),
    runs,
    stockBin: Option.getOrUndefined(stockBin),
    target
  })).pipe(
    Command.withDescription("Compare stock and Effect-patched TypeScript-Go performance")
  )

const perf = Command.make("perf").pipe(
  Command.withDescription("Run performance comparisons"),
  Command.withSubcommands([perfCompare])
)

const bundlePackageUpstream = Command.make("bundle-upstream", {}, () => bundleUpstream(repositoryRoot)).pipe(
  Command.withDescription("Copy upstream metadata into platform packages")
)

const preparePackages = Command.make("prepare", {}, () => preparePlatformPackages(repositoryRoot)).pipe(
  Command.withDescription("Prepare platform aliases and package manifests for publishing")
)

const assemblePackages = Command.make("assemble", {
  artifacts: Flag.string("artifacts").pipe(Flag.withDefault("_release-artifacts"))
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

const matrixRelease = Command.make("release", {}, () => printReleaseMatrix(repositoryRoot)).pipe(
  Command.withDescription("Print the platform release build matrix as JSON")
)

const matrix = Command.make("matrix").pipe(
  Command.withDescription("Generate CI matrices"),
  Command.withSubcommands([matrixGenerated, matrixRelease, matrixTestOxlint, matrixTestTypeScript])
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
    upstream
  ]),
  Command.run({ version: "0.0.0" }),
  Effect.provide(NodeServices.layer),
  NodeRuntime.runMain
)
