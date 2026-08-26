import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { constants } from "node:fs"
import { access, appendFile, readFile } from "node:fs/promises"
import { join } from "node:path"
import { ensureEffectFixtures } from "./fixtures.ts"
import { runCommand, runCommandString } from "./process.ts"
import {
  readUpstream,
  resolveMaterializedTypeScriptSource,
  type ComponentName,
  typeScriptSourceEntrypoint
} from "./upstream.ts"

export const buildTargets = {
  "darwin-arm64": { goos: "darwin", goarch: "arm64" },
  "darwin-x64": { goos: "darwin", goarch: "amd64" },
  "win32-x64": { goos: "windows", goarch: "amd64" },
  "win32-arm64": { goos: "windows", goarch: "arm64" },
  "linux-x64": { goos: "linux", goarch: "amd64" },
  "linux-arm64": { goos: "linux", goarch: "arm64" },
  "linux-arm": { goos: "linux", goarch: "arm", goarm: "6" }
} as const

export type BuildTarget = keyof typeof buildTargets

export const oxlintBuildTargets = {
  "darwin-arm64": {
    ...buildTargets["darwin-arm64"],
    rustTarget: "aarch64-apple-darwin",
    codeTarget: "darwin-arm64",
    napiArgs: []
  },
  "darwin-x64": {
    ...buildTargets["darwin-x64"],
    rustTarget: "x86_64-apple-darwin",
    codeTarget: "darwin-x64",
    napiArgs: []
  },
  "win32-x64": {
    ...buildTargets["win32-x64"],
    rustTarget: "x86_64-pc-windows-msvc",
    codeTarget: "win32-x64",
    napiArgs: []
  },
  "win32-arm64": {
    ...buildTargets["win32-arm64"],
    rustTarget: "aarch64-pc-windows-msvc",
    codeTarget: "win32-arm64",
    napiArgs: []
  },
  "linux-x64": {
    ...buildTargets["linux-x64"],
    rustTarget: "x86_64-unknown-linux-gnu",
    codeTarget: "linux-x64-gnu",
    napiArgs: ["--use-napi-cross"]
  },
  "linux-arm64": {
    ...buildTargets["linux-arm64"],
    rustTarget: "aarch64-unknown-linux-gnu",
    codeTarget: "linux-arm64-gnu",
    napiArgs: ["--use-napi-cross"]
  }
} as const

export type OxlintBuildTarget = keyof typeof oxlintBuildTargets

export const oxlintBindingName = (target: OxlintBuildTarget) =>
  `oxlint.${oxlintBuildTargets[target].codeTarget}${target.startsWith("win32-") ? "-msvc" : ""}.node`

export class BuildError extends Data.TaggedError("BuildError")<{
  readonly reason: string
}> {
  get message(): string {
    return `Build failed: ${this.reason}`
  }
}

export const componentArtifact = (
  repositoryRoot: string,
  target: BuildTarget,
  component: "typescript" | "oxlint-tsgolint" | "oxlint",
  version: string,
  binaryName: string
) => {
  const executable = target.startsWith("win32-") && !binaryName.endsWith(".node") ? `${binaryName}.exe` : binaryName
  return {
    binaryName,
    path: join(repositoryRoot, "_packages", `tsgo-${target}`, "artifacts", component, version, executable)
  }
}

export const oxlintArtifacts = (
  repositoryRoot: string,
  targetName: OxlintBuildTarget,
  oxlintVersion: string,
  tsgolintVersion: string
) => {
  const packageDirectory = join(repositoryRoot, "_packages", `tsgo-${targetName}`, "artifacts")
  const windows = targetName.startsWith("win32-")
  const bindingName = oxlintBindingName(targetName)
  return {
    packageDirectory,
    bindingName,
    bindingSourceName: bindingName,
    bindingPath: join(packageDirectory, "oxlint", oxlintVersion, bindingName),
    tsgolintPath: join(
      packageDirectory,
      "oxlint-tsgolint",
      tsgolintVersion,
      windows ? "tsgolint.exe" : "tsgolint"
    )
  }
}

const validateArtifact = Effect.fnUntraced(function*(artifact: string) {
  const fs = yield* FileSystem.FileSystem
  const info = yield* fs.stat(artifact).pipe(
    Effect.mapError((error) => new BuildError({ reason: `Missing artifact ${artifact}: ${error.message}` }))
  )
  if (info.size === 0n) {
    return yield* new BuildError({ reason: `Artifact is empty: ${artifact}` })
  }
})

const validateExecutableArtifact = Effect.fnUntraced(function*(artifact: string) {
  yield* validateArtifact(artifact)
  yield* Effect.tryPromise({
    try: () => access(artifact, constants.X_OK),
    catch: () => new BuildError({ reason: `Artifact is not executable: ${artifact}` })
  })
})

export const buildCli = Effect.fnUntraced(function*(repositoryRoot: string) {
  const path = yield* Path.Path
  yield* Console.log("Building CLI package")
  yield* runCommand("pnpm", path.join(repositoryRoot, "_packages", "tsgo"), [
    "exec",
    "tsdown",
    "--config",
    "tsdown.config.ts"
  ])
  yield* validateArtifact(path.join(repositoryRoot, "_packages", "tsgo", "dist", "effect-tsgo.cjs"))
})

export const buildLocal = Effect.fnUntraced(function*(repositoryRoot: string) {
  const path = yield* Path.Path
  const binary = path.join(repositoryRoot, "tsgo")
  const compiler = yield* resolveMaterializedTypeScriptSource(repositoryRoot)
  yield* ensureEffectFixtures(repositoryRoot)
  yield* Console.log("Building local Go binary")
  yield* runCommand("go", repositoryRoot, [
    "build",
    "-o",
    binary,
    typeScriptSourceEntrypoint(compiler)
  ], false, { CGO_ENABLED: "0" })
  yield* validateArtifact(binary)
  yield* buildCli(repositoryRoot)
})

const buildTsgolint = Effect.fnUntraced(function*(
  repositoryRoot: string,
  version: string,
  targetName: OxlintBuildTarget
) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const tsgolintSource = path.join(repositoryRoot, "tsgolint", "cmd", "tsgolint", "effect_rules_generated.go")
  if (!(yield* fs.exists(tsgolintSource))) {
    return yield* new BuildError({ reason: `Missing generated source ${tsgolintSource}; run profile codegen first` })
  }

  const upstream = yield* readUpstream(repositoryRoot)
  const component = upstream.components["oxlint-tsgolint"][version]
  if (component === undefined) {
    return yield* new BuildError({ reason: `Unknown oxlint-tsgolint component version ${version}` })
  }
  const gitHead = (yield* runCommandString("git", repositoryRoot, ["-C", "tsgolint", "rev-parse", "HEAD"])).trim()
  if (gitHead !== component.gitHead) {
    return yield* new BuildError({
      reason: `oxlint-tsgolint ${version} expects checkout ${component.gitHead}, found ${gitHead}`
    })
  }
  const target = oxlintBuildTargets[targetName]
  const artifact = componentArtifact(repositoryRoot, targetName, "oxlint-tsgolint", version, "tsgolint")
  yield* fs.makeDirectory(path.dirname(artifact.path), { recursive: true })
  yield* Console.log(`Building tsgolint for ${targetName}`)
  yield* runCommand("go", repositoryRoot, [
    "build",
    "-trimpath",
    "-ldflags=-s -w",
    "-o",
    artifact.path,
    "./tsgolint/cmd/tsgolint"
  ], false, {
    GOWORK: path.join(repositoryRoot, "go.work"),
    CGO_ENABLED: "0",
    GOOS: target.goos,
    GOARCH: target.goarch
  })
  yield* validateArtifact(artifact.path)

  if (process.env.GITHUB_OUTPUT !== undefined) {
    yield* Effect.tryPromise(() => appendFile(
      process.env.GITHUB_OUTPUT!,
      `component=oxlint-tsgolint\nnpm_version=${version}\nartifact_path=${artifact.path}\n`
    ))
  }
  return artifact.path
})

const buildOxlintBinding = Effect.fnUntraced(function*(
  repositoryRoot: string,
  version: string,
  targetName: OxlintBuildTarget
) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const oxlintSource = path.join(
    repositoryRoot,
    "oxlint",
    "crates",
    "oxc_linter",
    "src",
    "rules",
    "effecttsgo",
    "floating_effect.rs"
  )
  if (!(yield* fs.exists(oxlintSource))) {
    return yield* new BuildError({ reason: `Missing generated source ${oxlintSource}; run profile codegen first` })
  }

  const upstream = yield* readUpstream(repositoryRoot)
  const component = upstream.components.oxlint[version]
  if (component === undefined) {
    return yield* new BuildError({ reason: `Unknown oxlint component version ${version}` })
  }
  const gitHead = (yield* runCommandString("git", repositoryRoot, ["-C", "oxlint", "rev-parse", "HEAD"])).trim()
  if (gitHead !== component.gitHead) {
    return yield* new BuildError({ reason: `oxlint ${version} expects checkout ${component.gitHead}, found ${gitHead}` })
  }
  const target = oxlintBuildTargets[targetName]
  const artifact = componentArtifact(repositoryRoot, targetName, "oxlint", version, oxlintBindingName(targetName))
  const bindingSourceName = oxlintBindingName(targetName)
  const oxlint = path.join(repositoryRoot, "oxlint")
  const oxlintApp = path.join(oxlint, "apps", "oxlint")
  yield* fs.makeDirectory(path.dirname(artifact.path), { recursive: true })
  yield* Console.log(`Building Oxlint N-API addon for ${targetName}`)
  yield* runCommand("corepack", oxlint, ["pnpm", "install", "--frozen-lockfile"])
  yield* runCommand("corepack", oxlintApp, [
    "pnpm",
    "run",
    "build-napi-release",
    "--target",
    target.rustTarget,
    ...target.napiArgs
  ])
  const bindingSource = path.join(oxlintApp, "src-js", bindingSourceName)
  yield* validateArtifact(bindingSource)
  yield* fs.copyFile(bindingSource, artifact.path)
  yield* validateArtifact(artifact.path)
  yield* runCommand("corepack", oxlintApp, ["pnpm", "run", "build-js"])
  yield* validateArtifact(path.join(oxlintApp, "dist", "cli.js"))

  if (process.env.GITHUB_OUTPUT !== undefined) {
    yield* Effect.tryPromise(() => appendFile(
      process.env.GITHUB_OUTPUT!,
      `component=oxlint\nnpm_version=${version}\nartifact_path=${artifact.path}\n`
    ))
  }
  return artifact.path
})

const buildTsc = Effect.fnUntraced(function*(
  repositoryRoot: string,
  version: string,
  targetName: BuildTarget
) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  yield* ensureEffectFixtures(repositoryRoot)
  const upstream = yield* readUpstream(repositoryRoot)
  const component = upstream.components.typescript[version]
  if (component === undefined) {
    return yield* new BuildError({ reason: `Unknown TypeScript component version ${version}` })
  }
  const compiler = yield* resolveMaterializedTypeScriptSource(repositoryRoot)
  const checkoutRevision = (yield* runCommandString("git", repositoryRoot, [
    "-C",
    compiler.checkoutDir,
    "rev-parse",
    "HEAD"
  ])).trim()
  if (checkoutRevision !== component.gitHead) {
    return yield* new BuildError({
      reason: `TypeScript ${version} expects ${compiler.repositorySlug} ${component.gitHead}, found ${checkoutRevision}`
    })
  }

  const target = buildTargets[targetName]
  const artifact = componentArtifact(repositoryRoot, targetName, "typescript", version, "tsc")
  yield* fs.makeDirectory(path.dirname(artifact.path), { recursive: true })
  yield* Console.log(`Building TypeScript ${version} for ${targetName} -> ${artifact.path}`)
  yield* runCommand("go", repositoryRoot, [
    "build",
    "-ldflags=-s -w",
    "-o",
    artifact.path,
    typeScriptSourceEntrypoint(compiler)
  ], false, {
    CGO_ENABLED: "0",
    GOOS: target.goos,
    GOARCH: target.goarch,
    GOARM: "goarm" in target ? target.goarm : ""
  })
  yield* validateArtifact(artifact.path)

  if (process.env.GITHUB_OUTPUT !== undefined) {
    yield* Effect.tryPromise(() => appendFile(
      process.env.GITHUB_OUTPUT!,
      `component=typescript\nnpm_version=${version}\nartifact_path=${artifact.path}\n`
    ))
  }
  return artifact
})

export const buildArtifact = Effect.fnUntraced(function*(
  repositoryRoot: string,
  component: ComponentName,
  version: string,
  target: BuildTarget
) {
  if (component === "typescript") {
    return yield* buildTsc(repositoryRoot, version, target)
  }
  if (!(target in oxlintBuildTargets)) {
    return yield* new BuildError({ reason: `${component} does not support target ${target}` })
  }
  const oxlintTarget = target as OxlintBuildTarget
  if (component === "oxlint-tsgolint") {
    return yield* buildTsgolint(repositoryRoot, version, oxlintTarget)
  }
  return yield* buildOxlintBinding(repositoryRoot, version, oxlintTarget)
})

export const verifyReleaseArtifacts = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const upstream = yield* readUpstream(repositoryRoot)
  const sourceUpstream = yield* fs.readFileString(path.join(repositoryRoot, "_packages", "tsgo", "upstream.json"))
  for (const version of Object.keys(upstream.components.typescript)) {
    for (const target of Object.keys(buildTargets) as Array<BuildTarget>) {
      yield* validateArtifact(componentArtifact(repositoryRoot, target, "typescript", version, "tsc").path)
    }
  }
  for (const target of Object.keys(buildTargets) as Array<BuildTarget>) {
    const packagedUpstream = path.join(repositoryRoot, "_packages", `tsgo-${target}`, "lib", "upstream.json")
    const text = yield* fs.readFileString(packagedUpstream).pipe(
      Effect.mapError((error) => new BuildError({ reason: `Missing platform metadata ${packagedUpstream}: ${error.message}` }))
    )
    if (text !== sourceUpstream) {
      return yield* new BuildError({ reason: `Platform metadata does not match source: ${packagedUpstream}` })
    }
  }
  for (const target of Object.keys(oxlintBuildTargets) as Array<OxlintBuildTarget>) {
    for (const version of Object.keys(upstream.components["oxlint-tsgolint"])) {
      const artifact = componentArtifact(repositoryRoot, target, "oxlint-tsgolint", version, "tsgolint").path
      if (target.startsWith("win32-")) {
        yield* validateArtifact(artifact)
      } else {
        yield* validateExecutableArtifact(artifact)
      }
    }
    for (const version of Object.keys(upstream.components.oxlint)) {
      yield* validateArtifact(
        componentArtifact(repositoryRoot, target, "oxlint", version, oxlintBindingName(target)).path
      )
    }
  }
  for (const target of Object.keys(buildTargets) as Array<BuildTarget>) {
    const latest = componentArtifact(repositoryRoot, target, "typescript", upstream.tags.typescript.latest, "tsc").path
    const alias = join(repositoryRoot, "_packages", `tsgo-${target}`, "lib", target.startsWith("win32-") ? "tsc.exe" : "tsc")
    yield* validateArtifact(alias)
    if (!Buffer.from(yield* Effect.promise(() => readFile(latest))).equals(Buffer.from(yield* Effect.promise(() => readFile(alias))))) {
      return yield* new BuildError({ reason: `Latest TypeScript alias does not match ${latest}` })
    }
  }
  yield* Console.log("Verified release artifacts for all components and targets")
})
