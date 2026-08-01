import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { appendFile } from "node:fs/promises"
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

const validateArtifact = Effect.fnUntraced(function*(artifact: string) {
  const fs = yield* FileSystem.FileSystem
  const info = yield* fs.stat(artifact).pipe(
    Effect.mapError((error) => new BuildError({ reason: `Missing artifact ${artifact}: ${error.message}` }))
  )
  if (info.size === 0n) {
    return yield* new BuildError({ reason: `Artifact is empty: ${artifact}` })
  }
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

export const buildOxlint = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const tsgolintSource = path.join(repositoryRoot, "tsgolint", "cmd", "tsgolint", "effect_rules_generated.go")
  const oxlintSource = path.join(
    repositoryRoot,
    "oxlint",
    "crates",
    "oxc_linter",
    "src",
    "rules",
    "effect",
    "floating_effect.rs"
  )
  for (const generated of [tsgolintSource, oxlintSource]) {
    if (!(yield* fs.exists(generated))) {
      return yield* new BuildError({ reason: `Missing generated source ${generated}; run profile codegen first` })
    }
  }

  const buildDirectory = path.join(repositoryRoot, "build", "oxlint-tsgolint")
  const tsgolint = path.join(buildDirectory, "tsgolint")
  const oxlint = path.join(repositoryRoot, "oxlint")
  yield* fs.makeDirectory(buildDirectory, { recursive: true })
  yield* Console.log("Building tsgolint")
  yield* runCommand("go", repositoryRoot, [
    "build",
    "-trimpath",
    "-ldflags=-s -w",
    "-o",
    tsgolint,
    "./tsgolint/cmd/tsgolint"
  ], false, { GOWORK: path.join(repositoryRoot, "go.work"), CGO_ENABLED: "0" })
  yield* validateArtifact(tsgolint)

  yield* Console.log("Building Oxlint N-API addon and Node launcher")
  yield* runCommand("pnpm", oxlint, ["install", "--frozen-lockfile"])
  yield* runCommand("pnpm", path.join(oxlint, "apps", "oxlint"), ["run", "build"])
  yield* validateArtifact(path.join(oxlint, "apps", "oxlint", "dist", "cli.js"))
})

export const buildBinary = Effect.fnUntraced(function*(
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
  yield* Console.log("Verified release binaries for all profiles and targets")
})
