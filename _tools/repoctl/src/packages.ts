import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { buildTargets, componentArtifact, oxlintBuildTargets } from "./build.ts"
import { buildReleasePlan } from "./releasePlan.ts"
import { readUpstream } from "./upstream.ts"

export class PackagePreparationError extends Data.TaggedError("PackagePreparationError")<{
  readonly reason: string
}> {
  get message(): string {
    return `Unable to prepare platform packages: ${this.reason}`
  }
}

export const bundleUpstream = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  yield* readUpstream(repositoryRoot)
  const source = path.join(repositoryRoot, "_packages", "tsgo", "upstream.json")

  for (const target of Object.keys(buildTargets)) {
    const destination = path.join(repositoryRoot, "_packages", `tsgo-${target}`, "lib", "upstream.json")
    yield* fs.makeDirectory(path.dirname(destination), { recursive: true })
    yield* fs.copyFile(source, destination)
  }
  yield* Console.log("Bundled upstream.json in platform packages")
})

export const assembleReleaseArtifacts = Effect.fnUntraced(function*(
  repositoryRoot: string,
  artifactsDirectory: string
) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const upstream = yield* readUpstream(repositoryRoot)
  const plan = buildReleasePlan(upstream)
  const sourceRoot = path.resolve(repositoryRoot, artifactsDirectory)
  const actualArtifacts = (yield* fs.readDirectory(sourceRoot)).sort()
  const expectedArtifacts = plan.map((artifact) => artifact.artifactName).sort()
  const expectedSet = new Set(expectedArtifacts)
  const actualSet = new Set(actualArtifacts)
  const missing = expectedArtifacts.filter((artifact) => !actualSet.has(artifact))
  const unexpected = actualArtifacts.filter((artifact) => !expectedSet.has(artifact))
  if (missing.length > 0 || unexpected.length > 0) {
    return yield* new PackagePreparationError({
      reason: [
        ...(missing.length === 0 ? [] : [`missing artifacts: ${missing.join(", ")}`]),
        ...(unexpected.length === 0 ? [] : [`unexpected artifacts: ${unexpected.join(", ")}`])
      ].join("; ")
    })
  }

  const copies: Array<{ readonly source: string; readonly destination: string }> = []
  for (const artifact of plan) {
    const directory = path.join(sourceRoot, artifact.artifactName)
    const files = (yield* fs.readDirectory(directory)).sort()
    if (files.length !== 1 || files[0] !== artifact.fileName) {
      return yield* new PackagePreparationError({
        reason: `${artifact.artifactName} must contain only ${artifact.fileName}, found ${files.join(", ") || "nothing"}`
      })
    }
    const source = path.join(directory, artifact.fileName)
    const info = yield* fs.stat(source)
    if (info.type !== "File" || info.size === 0n) {
      return yield* new PackagePreparationError({ reason: `Invalid release artifact: ${source}` })
    }
    copies.push({ source, destination: path.join(repositoryRoot, artifact.destination) })
  }

  for (const copy of copies) {
    yield* fs.makeDirectory(path.dirname(copy.destination), { recursive: true })
    yield* fs.copyFile(copy.source, copy.destination)
  }
  yield* Console.log(`Assembled ${copies.length} release artifacts into platform packages`)
})

export const preparePlatformPackages = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const upstream = yield* readUpstream(repositoryRoot)
  const sourceUpstream = path.join(repositoryRoot, "_packages", "tsgo", "upstream.json")

  for (const target of Object.keys(buildTargets) as Array<keyof typeof buildTargets>) {
    const packageRoot = path.join(repositoryRoot, "_packages", `tsgo-${target}`)
    const windows = target.startsWith("win32-")
    const latest = componentArtifact(repositoryRoot, target, "typescript", upstream.typescript.latest, "tsc").path
    const alias = path.join(packageRoot, "lib", windows ? "tsc.exe" : "tsc")
    yield* fs.makeDirectory(path.dirname(alias), { recursive: true })
    yield* fs.copyFile(latest, alias)
    if (!windows) yield* fs.chmod(alias, 0o755)
    yield* fs.copyFile(sourceUpstream, path.join(packageRoot, "lib", "upstream.json"))
    yield* fs.remove(path.join(packageRoot, "lib", windows ? "tsc-next.exe" : "tsc-next"), { force: true })

    const executableFiles = [
      `./lib/tsc${windows ? ".exe" : ""}`,
      ...Object.keys(upstream.components.typescript).map((version) =>
        `./artifacts/typescript/${version}/tsc${windows ? ".exe" : ""}`),
      ...(target in oxlintBuildTargets
        ? Object.keys(upstream.components["oxlint-tsgolint"]).map((version) =>
          `./artifacts/oxlint-tsgolint/${version}/tsgolint${windows ? ".exe" : ""}`)
        : [])
    ]
    for (const executable of executableFiles) {
      const artifact = path.join(packageRoot, executable.slice(2))
      if (!(yield* fs.exists(artifact))) {
        return yield* new PackagePreparationError({ reason: `Missing packaged executable ${artifact}` })
      }
      if (!windows) yield* fs.chmod(artifact, 0o755)
    }

    const packageJsonPath = path.join(packageRoot, "package.json")
    const packageJsonText = yield* fs.readFileString(packageJsonPath)
    const packageJson = yield* Effect.try({
      try: () => JSON.parse(packageJsonText) as Record<string, unknown>,
      catch: (error) => new PackagePreparationError({
        reason: `Unable to parse ${packageJsonPath}: ${String(error)}`
      })
    })
    const publishConfig = typeof packageJson.publishConfig === "object" && packageJson.publishConfig !== null
      ? packageJson.publishConfig as Record<string, unknown>
      : {}
    yield* fs.writeFileString(packageJsonPath, `${JSON.stringify({
      ...packageJson,
      publishConfig: { ...publishConfig, executableFiles }
    }, null, 2)}\n`)
  }
  yield* Console.log("Prepared platform package manifests and latest TypeScript aliases")
})
