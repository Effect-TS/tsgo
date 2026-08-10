import * as nodeModule from "node:module"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { hashFile } from "./fileHash.js"
import type { Component, DiscoveredBinary } from "./types.js"

export const defaultTypescriptPackageNames = ["typescript", "@typescript/native"] as const

export class DiscoveryError extends Data.TaggedError("DiscoveryError")<{ readonly reason: string }> {
  get message(): string {
    return this.reason
  }
}

interface PackageMetadata {
  readonly packageJsonPath: string
  readonly version: string
  readonly main?: string
}

type DiscoveredBinaryLocation = Omit<DiscoveredBinary, "fileHash">

const isNativeTypescriptVersion = (version: string) => {
  const match = /\d+/.exec(version.trim())
  return match !== null && Number(match[0]) >= 7
}

const readPackage = (require: NodeJS.Require, packageName: string) => Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  const packageJsonPath = yield* Effect.try({
    try: () => require.resolve(`${packageName}/package.json`),
    catch: () => new DiscoveryError({ reason: `Unable to resolve ${packageName}.` })
  })
  const text = yield* fs.readFileString(packageJsonPath).pipe(
    Effect.mapError(() => new DiscoveryError({ reason: `Unable to read ${packageJsonPath}.` }))
  )
  const json = yield* Effect.try({
    try: () => JSON.parse(text) as unknown,
    catch: () => new DiscoveryError({ reason: `Unable to parse ${packageJsonPath}.` })
  })
  if (typeof json !== "object" || json === null || !("version" in json) || typeof json.version !== "string") {
    return yield* new DiscoveryError({ reason: `${packageJsonPath} does not contain a version.` })
  }
  return {
    packageJsonPath,
    version: json.version,
    ...("main" in json && typeof json.main === "string" ? { main: json.main } : {})
  } satisfies PackageMetadata
})

const optionally = <A, R>(effect: Effect.Effect<A, DiscoveryError, R>) =>
  effect.pipe(Effect.catchTag("DiscoveryError", (error) =>
    error.reason.startsWith("Unable to resolve ") ? Effect.succeed(undefined) : Effect.fail(error)))

const isGlibc = () => {
  if (process.platform !== "linux") return false
  const report = process.report?.getReport()
  const header = (report as { readonly header?: { readonly glibcVersionRuntime?: unknown } } | undefined)?.header
  return typeof header?.glibcVersionRuntime === "string"
}

export const experimentalOxlintTarget = (platform: NodeJS.Platform, arch: string, glibc: boolean) => {
  if (arch !== "x64" && arch !== "arm64") {
    throw new DiscoveryError({ reason: `Unsupported architecture ${arch}.` })
  }
  if (platform === "linux" && !glibc) {
    throw new DiscoveryError({ reason: "Linux musl is not supported by the packaged Oxlint integration." })
  }
  if (platform !== "linux" && platform !== "darwin" && platform !== "win32") {
    throw new DiscoveryError({ reason: `Unsupported platform ${platform}.` })
  }
  const packageTarget = `${platform}-${arch}`
  const codeTarget = platform === "linux" ? `${packageTarget}-gnu` : packageTarget
  const oxlintTarget = platform === "win32" ? `${codeTarget}-msvc` : codeTarget
  return {
    codeTarget,
    oxlintPackage: `@oxlint/binding-${oxlintTarget}`,
    tsgolintPackage: `@oxlint-tsgolint/${packageTarget}`,
    tsgolintExecutable: platform === "win32" ? "tsgolint.exe" : "tsgolint"
  }
}

const discoverTypeScript: (
  cwdRequire: NodeJS.Require,
  preferredPackage?: string
) => Effect.Effect<DiscoveredBinaryLocation[], DiscoveryError, FileSystem.FileSystem | Path.Path> = (
  cwdRequire,
  preferredPackage
) => Effect.gen(function*() {
  const path = yield* Path.Path
  const packageNames = preferredPackage === undefined || preferredPackage === ""
    ? defaultTypescriptPackageNames
    : Array.from(new Set([preferredPackage, ...defaultTypescriptPackageNames]))
  for (const packageName of packageNames) {
    const installed = yield* optionally(readPackage(cwdRequire, packageName))
    if (installed === undefined || !isNativeTypescriptVersion(installed.version)) continue
    const platformPackageName = `@typescript/typescript-${process.platform}-${process.arch}`
    const packageRequire = nodeModule.createRequire(installed.packageJsonPath)
    const platformPackage = yield* readPackage(packageRequire, platformPackageName)
    const binaryName = process.platform === "win32" ? "tsc.exe" : "tsc"
    return [{
      component: "typescript",
      packageName: platformPackageName,
      packageVersion: platformPackage.version,
      binaryPath: path.join(path.dirname(platformPackage.packageJsonPath), "lib", binaryName)
    } satisfies DiscoveredBinaryLocation]
  }
  return []
})

const discoverOxlint: (
  cwdRequire: NodeJS.Require
) => Effect.Effect<DiscoveredBinaryLocation[], DiscoveryError, FileSystem.FileSystem | Path.Path> = (cwdRequire) =>
  Effect.gen(function*() {
    const path = yield* Path.Path
    const oxlint = yield* optionally(readPackage(cwdRequire, "oxlint"))
    const tsgolint = yield* optionally(readPackage(cwdRequire, "oxlint-tsgolint"))
    if (oxlint === undefined && tsgolint === undefined) return []
    const target = yield* Effect.try({
      try: () => experimentalOxlintTarget(process.platform, process.arch, isGlibc()),
      catch: (error) => error instanceof DiscoveryError
        ? error
        : new DiscoveryError({ reason: "Unable to determine the Oxlint platform target." })
    })
    const discovered: Array<DiscoveredBinaryLocation> = []
    if (oxlint !== undefined) {
      const binding = yield* readPackage(nodeModule.createRequire(oxlint.packageJsonPath), target.oxlintPackage)
      if (binding.main === undefined) {
        return yield* new DiscoveryError({ reason: `${binding.packageJsonPath} does not contain a main entrypoint.` })
      }
      discovered.push({
        component: "oxlint",
        packageName: target.oxlintPackage,
        packageVersion: binding.version,
        binaryPath: path.join(path.dirname(binding.packageJsonPath), binding.main)
      })
      discovered.push({
        component: "oxlint-dts",
        packageName: "oxlint",
        packageVersion: oxlint.version,
        binaryPath: path.join(path.dirname(oxlint.packageJsonPath), "dist", "index.d.ts")
      })
    }
    if (tsgolint !== undefined) {
      const nativePackage = yield* readPackage(
        nodeModule.createRequire(tsgolint.packageJsonPath),
        target.tsgolintPackage
      )
      discovered.push({
        component: "oxlint-tsgolint",
        packageName: target.tsgolintPackage,
        packageVersion: nativePackage.version,
        binaryPath: path.join(path.dirname(nativePackage.packageJsonPath), target.tsgolintExecutable)
      })
    }
    return discovered
  })

const discoverVitePlusOxlint: (
  cwdRequire: NodeJS.Require
) => Effect.Effect<DiscoveredBinaryLocation[], DiscoveryError, FileSystem.FileSystem | Path.Path> = (cwdRequire) =>
  Effect.gen(function*() {
    const vitePlus = yield* optionally(readPackage(cwdRequire, "vite-plus"))
    if (vitePlus === undefined) return []
    return yield* discoverOxlint(nodeModule.createRequire(vitePlus.packageJsonPath))
  })

export const discoverBinaries = (cwd: string, preferredTypescriptPackage?: string) => Effect.gen(function*() {
  const path = yield* Path.Path
  const cwdRequire = nodeModule.createRequire(path.join(cwd, "noop.js"))
  const typescript = yield* discoverTypeScript(cwdRequire, preferredTypescriptPackage)
  const oxlint = yield* discoverOxlint(cwdRequire)
  const vitePlusOxlint = yield* discoverVitePlusOxlint(cwdRequire)
  const discovered = [...new Map(
    [...typescript, ...oxlint, ...vitePlusOxlint].map((binary) => [binary.binaryPath, binary])
  ).values()]
  return yield* Effect.forEach(discovered, (binary) => hashFile(binary.binaryPath).pipe(
    Effect.map((fileHash) => ({ ...binary, fileHash })),
    Effect.mapError(() => new DiscoveryError({ reason: `Unable to read discovered binary ${binary.binaryPath}.` }))
  ))
})

export const selectComponents = (
  binaries: ReadonlyArray<DiscoveredBinary>,
  components: ReadonlySet<Component>
) => binaries.filter((binary) => components.has(binary.component))

export const requireComponents = (
  binaries: ReadonlyArray<DiscoveredBinary>,
  components: ReadonlySet<Component>
) => Effect.gen(function*() {
  for (const component of components) {
    if (!binaries.some((binary) => binary.component === component)) {
      return yield* new DiscoveryError({ reason: `Unable to discover an installed ${component} binary.` })
    }
  }
  return binaries
})
