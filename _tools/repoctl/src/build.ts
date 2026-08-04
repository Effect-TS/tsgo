import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { constants } from "node:fs"
import { access, appendFile } from "node:fs/promises"
import { join } from "node:path"
import { ensureEffectFixtures } from "./fixtures.ts"
import { runCommand, runCommandString } from "./process.ts"
import { getProfile, readUpstream } from "./upstream.ts"

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
export type BuildProfile = "next" | "latest"
const buildProfiles: ReadonlyArray<BuildProfile> = ["next", "latest"]

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

export class BuildError extends Data.TaggedError("BuildError")<{
  readonly reason: string
}> {
  get message(): string {
    return `Build failed: ${this.reason}`
  }
}

export const binaryArtifact = (
  repositoryRoot: string,
  target: BuildTarget,
  binaryName: string
) => {
  const executable = target.startsWith("win32-") ? `${binaryName}.exe` : binaryName
  return {
    binaryName,
    path: join(repositoryRoot, "_packages", `tsgo-${target}`, "lib", executable)
  }
}

export const isBinaryName = (name: string) => /^[A-Za-z0-9._-]+$/.test(name)

export const oxlintArtifacts = (repositoryRoot: string, targetName: OxlintBuildTarget) => {
  const target = oxlintBuildTargets[targetName]
  const packageDirectory = join(repositoryRoot, "_packages", `tsgo-${targetName}`, "lib")
  const windows = targetName.startsWith("win32-")
  const bindingName = `oxlint.${target.codeTarget}.node`
  return {
    packageDirectory,
    bindingName,
    bindingSourceName: `oxlint.${target.codeTarget}${windows ? "-msvc" : ""}.node`,
    bindingPath: join(packageDirectory, bindingName),
    tsgolintPath: join(packageDirectory, windows ? "tsgolint.exe" : "tsgolint")
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
  yield* ensureEffectFixtures(repositoryRoot)
  yield* Console.log("Building local Go binary")
  yield* runCommand("go", repositoryRoot, [
    "build",
    "-o",
    binary,
    "./typescript-go/cmd/tsgo"
  ], false, { CGO_ENABLED: "0" })
  yield* validateArtifact(binary)
  yield* buildCli(repositoryRoot)
})

export const buildTsgolint = Effect.fnUntraced(function*(repositoryRoot: string, targetName: OxlintBuildTarget) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const tsgolintSource = path.join(repositoryRoot, "tsgolint", "cmd", "tsgolint", "effect_rules_generated.go")
  if (!(yield* fs.exists(tsgolintSource))) {
    return yield* new BuildError({ reason: `Missing generated source ${tsgolintSource}; run profile codegen first` })
  }

  const target = oxlintBuildTargets[targetName]
  const artifacts = oxlintArtifacts(repositoryRoot, targetName)
  yield* fs.makeDirectory(artifacts.packageDirectory, { recursive: true })
  yield* Console.log(`Building tsgolint for ${targetName}`)
  yield* runCommand("go", repositoryRoot, [
    "build",
    "-trimpath",
    "-ldflags=-s -w",
    "-o",
    artifacts.tsgolintPath,
    "./tsgolint/cmd/tsgolint"
  ], false, {
    GOWORK: path.join(repositoryRoot, "go.work"),
    CGO_ENABLED: "0",
    GOOS: target.goos,
    GOARCH: target.goarch
  })
  yield* validateArtifact(artifacts.tsgolintPath)

  if (process.env.GITHUB_OUTPUT !== undefined) {
    yield* Effect.tryPromise(() => appendFile(
      process.env.GITHUB_OUTPUT!,
      `artifact_path=${artifacts.tsgolintPath}\n`
    ))
  }
  return artifacts.tsgolintPath
})

export const buildOxlintBinding = Effect.fnUntraced(function*(repositoryRoot: string, targetName: OxlintBuildTarget) {
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

  const target = oxlintBuildTargets[targetName]
  const artifacts = oxlintArtifacts(repositoryRoot, targetName)
  const oxlint = path.join(repositoryRoot, "oxlint")
  const oxlintApp = path.join(oxlint, "apps", "oxlint")
  yield* fs.makeDirectory(artifacts.packageDirectory, { recursive: true })
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
  const bindingSource = path.join(oxlintApp, "src-js", artifacts.bindingSourceName)
  yield* validateArtifact(bindingSource)
  yield* fs.copyFile(bindingSource, artifacts.bindingPath)
  yield* validateArtifact(artifacts.bindingPath)
  yield* runCommand("corepack", oxlintApp, ["pnpm", "run", "build-js"])
  yield* validateArtifact(path.join(oxlintApp, "dist", "cli.js"))

  if (process.env.GITHUB_OUTPUT !== undefined) {
    yield* Effect.tryPromise(() => appendFile(
      process.env.GITHUB_OUTPUT!,
      `artifact_path=${artifacts.bindingPath}\n`
    ))
  }
  return artifacts.bindingPath
})

export const buildTsc = Effect.fnUntraced(function*(
  repositoryRoot: string,
  profileName: BuildProfile,
  targetName: BuildTarget
) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  yield* ensureEffectFixtures(repositoryRoot)
  const upstream = yield* readUpstream(repositoryRoot)
  const profile = yield* getProfile(upstream, profileName)
  if (profile.kind !== "ts") {
    return yield* new BuildError({ reason: `Profile ${profileName} does not define a TypeScript binary` })
  }
  if (!isBinaryName(profile.binName)) {
    return yield* new BuildError({ reason: `Profile ${profileName} has invalid binary name ${JSON.stringify(profile.binName)}` })
  }
  const checkoutRevision = (yield* runCommandString("git", repositoryRoot, [
    "-C",
    "typescript-go",
    "rev-parse",
    "HEAD"
  ])).trim()
  if (checkoutRevision !== profile.ts.gitHead) {
    return yield* new BuildError({
      reason: `Profile ${profileName} expects TypeScript-Go ${profile.ts.gitHead}, found ${checkoutRevision}`
    })
  }

  const target = buildTargets[targetName]
  const artifact = binaryArtifact(repositoryRoot, targetName, profile.binName)
  yield* fs.makeDirectory(path.dirname(artifact.path), { recursive: true })
  yield* Console.log(`Building ${profileName} for ${targetName} -> ${artifact.path}`)
  yield* runCommand("go", repositoryRoot, [
    "build",
    "-ldflags=-s -w",
    "-o",
    artifact.path,
    "./typescript-go/cmd/tsgo"
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
      `binary_name=${artifact.binaryName}\nbinary_path=${artifact.path}\n`
    ))
  }
  return artifact
})

export const verifyReleaseArtifacts = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const upstream = yield* readUpstream(repositoryRoot)
  const sourceUpstream = yield* fs.readFileString(path.join(repositoryRoot, "_packages", "tsgo", "upstream.json"))
  for (const profileName of buildProfiles) {
    const profile = yield* getProfile(upstream, profileName)
    if (profile.kind !== "ts" || !isBinaryName(profile.binName)) {
      return yield* new BuildError({ reason: `Profile ${profileName} does not define a valid binary name` })
    }
    for (const target of Object.keys(buildTargets) as Array<BuildTarget>) {
      yield* validateArtifact(binaryArtifact(repositoryRoot, target, profile.binName).path)
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
    const artifacts = oxlintArtifacts(repositoryRoot, target)
    if (target.startsWith("win32-")) {
      yield* validateArtifact(artifacts.tsgolintPath)
    } else {
      yield* validateExecutableArtifact(artifacts.tsgolintPath)
    }
    yield* validateArtifact(artifacts.bindingPath)
  }
  yield* Console.log("Verified release binaries for all profiles and targets")
})
