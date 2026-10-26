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

export interface PackageMetadata {
  readonly packageJsonPath: string
  readonly version: string
  readonly main?: string
  readonly bin?: string | Readonly<Record<string, string>>
}

type DiscoveredBinaryLocation = Omit<DiscoveredBinary, "fileHash">

export const isNativeTypescriptVersion = (version: string) => {
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
  const bin = "bin" in json ? json.bin : undefined
  return {
    packageJsonPath,
    version: json.version,
    ...("main" in json && typeof json.main === "string" ? { main: json.main } : {}),
    ...(typeof bin === "string" || (typeof bin === "object" && bin !== null)
      ? { bin: bin as string | Readonly<Record<string, string>> }
      : {})
  } satisfies PackageMetadata
})

const optionally = <A, R>(effect: Effect.Effect<A, DiscoveryError, R>) =>
  effect.pipe(Effect.catchTag("DiscoveryError", (error) =>
    error.reason.startsWith("Unable to resolve ") ? Effect.succeed(undefined) : Effect.fail(error)))

// The metadata of a package resolved through `require`, or undefined when the package is not installed.
export const findInstalledPackage = (require: NodeJS.Require, packageName: string) =>
  optionally(readPackage(require, packageName))

export const isGlibc = () => {
  if (process.platform !== "linux") return false
  const report = process.report?.getReport()
  const header = (report as { readonly header?: { readonly glibcVersionRuntime?: unknown } } | undefined)?.header
  return typeof header?.glibcVersionRuntime === "string"
}

export const experimentalOxlintTarget = (platform: NodeJS.Platform, arch: string, glibc: boolean) => {
  if (arch !== "x64" && arch !== "arm64") {
    throw new DiscoveryError({ reason: `Unsupported architecture ${arch}.` })
  }
  if (platform !== "linux" && platform !== "darwin" && platform !== "win32") {
    throw new DiscoveryError({ reason: `Unsupported platform ${platform}.` })
  }
  const packageTarget = `${platform}-${arch}`
  const codeTarget = platform === "linux" ? `${packageTarget}-${glibc ? "gnu" : "musl"}` : packageTarget
  const oxlintTarget = platform === "win32" ? `${codeTarget}-msvc` : codeTarget
  return {
    codeTarget,
    oxlintBinding: `oxlint.${oxlintTarget}.node`,
    oxlintPackage: `@oxlint/binding-${oxlintTarget}`,
    tsgolintPackage: `@oxlint-tsgolint/${packageTarget}`,
    tsgolintExecutable: platform === "win32" ? "tsgolint.exe" : "tsgolint"
  }
}

// The platform packages tsrs publishes (`@tsrs/<target>`, an optional dependency of the `tsrs` package).
export const tsrsTargets = [
  "darwin-arm64",
  "darwin-x64",
  "linux-x64-gnu",
  "linux-x64-musl",
  "linux-arm64-gnu",
  "linux-arm64-musl",
  "win32-x64"
] as const

export type TsrsTarget = typeof tsrsTargets[number]

// The tsrs platform package for a host, or undefined when tsrs publishes none for it.
export const tsrsTarget = (platform: NodeJS.Platform, arch: string, glibc: boolean): TsrsTarget | undefined => {
  const target = platform === "linux" ? `${platform}-${arch}-${glibc ? "gnu" : "musl"}` : `${platform}-${arch}`
  return (tsrsTargets as ReadonlyArray<string>).includes(target) ? target as TsrsTarget : undefined
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

// The tsrs executable's file name in its platform package (and in ours: artifacts/tsrs/<version>/).
export const tsrsExecutableName = (platform: NodeJS.Platform) => platform === "win32" ? "tsrs.exe" : "tsrs"

// The installed tsrs binary: `<@tsrs/<target> package>/tsrs[.exe]` (the package also ships the lib*.d.ts files next to it,
// which are left alone). Nothing is discovered when tsrs is not installed or publishes no binary for this host.
const discoverTsrs: (
  cwdRequire: NodeJS.Require
) => Effect.Effect<DiscoveredBinaryLocation[], DiscoveryError, FileSystem.FileSystem | Path.Path> = (cwdRequire) =>
  Effect.gen(function*() {
    const path = yield* Path.Path
    const tsrs = yield* optionally(readPackage(cwdRequire, "tsrs"))
    if (tsrs === undefined) return []
    const target = tsrsTarget(process.platform, process.arch, isGlibc())
    if (target === undefined) return []
    const platformPackageName = `@tsrs/${target}`
    const platformPackage = yield* optionally(readPackage(nodeModule.createRequire(tsrs.packageJsonPath), platformPackageName))
    if (platformPackage === undefined) return []
    return [{
      component: "tsrs",
      packageName: platformPackageName,
      packageVersion: platformPackage.version,
      binaryPath: path.join(path.dirname(platformPackage.packageJsonPath), tsrsExecutableName(process.platform))
    } satisfies DiscoveredBinaryLocation]
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
  const tsrs = yield* discoverTsrs(cwdRequire)
  const discovered = [...new Map(
    [...typescript, ...oxlint, ...vitePlusOxlint, ...tsrs].map((binary) => [binary.binaryPath, binary])
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
      return yield* new DiscoveryError({
        reason: component === "tsrs"
          ? `Unable to discover an installed tsrs binary. Install the tsrs package; its binary is published for ${
            tsrsTargets.join(", ")
          }.`
          : `Unable to discover an installed ${component} binary.`
      })
    }
  }
  return binaries
})
