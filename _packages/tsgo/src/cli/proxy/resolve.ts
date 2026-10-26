import { constants as fsConstants } from "node:fs"
import { access } from "node:fs/promises"
import * as nodeModule from "node:module"
import * as nodePath from "node:path"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import * as pkgJson from "../../../package.json" with { type: "json" }
import upstreamJson from "../../../upstream.json" with { type: "json" }
import {
  defaultTypescriptPackageNames,
  experimentalOxlintTarget,
  findInstalledPackage,
  isNativeTypescriptVersion,
  type PackageMetadata,
  packagedComponentPath,
  supportedVersions,
  tsrsExecutableName,
  tsrsReplacementTargets,
  tsrsTarget
} from "../../patcher/index.js"
import type { ProxyInvocation, ProxyTool } from "./argv.js"
import type { ProxyCommand } from "./exec.js"

export class ProxyError extends Data.TaggedError("ProxyError")<{ readonly reason: string }> {
  get message(): string {
    return this.reason
  }
}

export class UnsupportedProxyVersionError extends Data.TaggedError("UnsupportedProxyVersionError")<{
  readonly packageName: string
  readonly version: string
  // Whether the version was requested as `<tool>@<version>` or found installed in the project.
  readonly source: "requested" | "installed"
  readonly supportedVersions: ReadonlyArray<string>
  readonly tags: Readonly<Record<string, string>>
}> {
  get message(): string {
    const tags = Object.entries(this.tags).map(([tag, version]) => `${tag} (${version})`)
    return this.source === "requested"
      ? `${this.packageName}@${this.version} is not supported by @effect/tsgo version ${pkgJson.version}. `
        + `Supported versions: ${this.supportedVersions.join(", ")}`
        + (tags.length === 0 ? "." : `; tags: ${tags.join(", ")}.`)
      : `Installed ${this.packageName} version ${this.version} is not supported by @effect/tsgo version ${pkgJson.version}. `
        + `Either change @effect/tsgo to a compatible version or set ${this.packageName} to one of the supported versions: ${
          this.supportedVersions.join(", ")
        }.`
  }
}

export interface ProxyContext {
  readonly cwd: string
  readonly env: NodeJS.ProcessEnv
  // The Node.js executable that runs the Oxlint CLI.
  readonly execPath: string
  readonly platform: NodeJS.Platform
  readonly arch: string
  readonly isGlibc: () => boolean
  // The directory of the @effect/tsgo-<platform>-<arch> package.
  readonly packageDirectory: string
}

type VersionedComponent = "typescript" | "tsrs"

const upstreamTags = (component: VersionedComponent): Readonly<Record<string, string>> => upstreamJson.tags[component]

// The packaged build to run: the requested version or tag, else the installed version when a build of it ships, else
// the latest supported version.
export const selectVersion = (
  tool: Exclude<ProxyTool, "oxlint">,
  component: VersionedComponent,
  requested: string | undefined,
  installed: string | undefined
) => Effect.gen(function*() {
  const versions = supportedVersions(component)
  const tags = upstreamTags(component)
  if (requested !== undefined) {
    if (requested === "") return yield* new ProxyError({ reason: `Expected a version or tag after ${tool}@.` })
    const version = Object.hasOwn(tags, requested) ? tags[requested]! : requested
    if (versions.includes(version)) return version
    return yield* new UnsupportedProxyVersionError({
      packageName: tool,
      version: requested,
      source: "requested",
      supportedVersions: versions,
      tags
    })
  }
  if (installed !== undefined && versions.includes(installed)) return installed
  return tags.latest!
})

const projectRequire = (context: ProxyContext) => Effect.gen(function*() {
  const path = yield* Path.Path
  return nodeModule.createRequire(path.join(context.cwd, "noop.js"))
})

const toProxyError = (error: { readonly message: string }) => new ProxyError({ reason: error.message })

// The native TypeScript version installed in the project (typescript or @typescript/native, >= 7).
const installedTypescriptVersion = (context: ProxyContext) => Effect.gen(function*() {
  const require = yield* projectRequire(context)
  for (const packageName of defaultTypescriptPackageNames) {
    const installed = yield* findInstalledPackage(require, packageName).pipe(Effect.mapError(toProxyError))
    if (installed !== undefined && isNativeTypescriptVersion(installed.version)) return installed.version
  }
  return undefined
})

const installedTsrsVersion = (context: ProxyContext) => Effect.gen(function*() {
  const require = yield* projectRequire(context)
  const installed = yield* findInstalledPackage(require, "tsrs").pipe(Effect.mapError(toProxyError))
  return installed?.version
})

const requireArtifact = (artifactPath: string) => Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  const exists = yield* fs.exists(artifactPath).pipe(Effect.mapError(toProxyError))
  if (!exists) return yield* new ProxyError({ reason: `Missing packaged artifact ${artifactPath}.` })
  return artifactPath
})

// Marks a packaged binary executable only when this process cannot already execute it, since shared or global stores
// may be read-only.
export const ensureExecutable = (filePath: string, platform: NodeJS.Platform) => Effect.gen(function*() {
  if (platform === "win32") return
  const fs = yield* FileSystem.FileSystem
  const executable = yield* Effect.promise(() => access(filePath, fsConstants.X_OK).then(() => true, () => false))
  if (executable) return
  yield* fs.chmod(filePath, 0o755).pipe(
    Effect.mapError((error) => new ProxyError({
      reason: `Failed to set executable permissions on ${filePath}: ${error.message}`
    }))
  )
})

// Like the upstream launchers, long Windows paths are passed in their \\?\ (or \\?\UNC\) namespaced form.
export const executablePath = (filePath: string, platform: NodeJS.Platform) =>
  platform === "win32" && filePath.length >= 248 ? nodePath.win32.toNamespacedPath(filePath) : filePath

export const tscExecutable = (context: ProxyContext, version: string) =>
  packagedComponentPath(context.packageDirectory, "typescript", version, context.platform === "win32" ? "tsc.exe" : "tsc")

export const tsrsExecutable = (context: ProxyContext, version: string) =>
  packagedComponentPath(context.packageDirectory, "tsrs", version, tsrsExecutableName(context.platform))

const resolveTsc = (context: ProxyContext, invocation: ProxyInvocation) => Effect.gen(function*() {
  const installed = invocation.version === undefined ? yield* installedTypescriptVersion(context) : undefined
  const version = yield* selectVersion("tsc", "typescript", invocation.version, installed)
  const executable = yield* requireArtifact(yield* tscExecutable(context, version))
  yield* ensureExecutable(executable, context.platform)
  return {
    file: executablePath(executable, context.platform),
    args: invocation.args,
    env: context.env
  } satisfies ProxyCommand
})

const resolveTsrs = (context: ProxyContext, invocation: ProxyInvocation) => Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  if (context.platform === "linux" && !context.isGlibc()) {
    return yield* new ProxyError({
      reason: `Linux musl is not supported by the packaged tsrs integration; it is built for ${tsrsReplacementTargets.join(", ")}.`
    })
  }
  const target = tsrsTarget(context.platform, context.arch, true)
  if (target === undefined || !(tsrsReplacementTargets as ReadonlyArray<string>).includes(target)) {
    return yield* new ProxyError({
      reason: `The packaged tsrs integration is not available for ${context.platform}-${context.arch}; it is built for ${
        tsrsReplacementTargets.join(", ")
      }.`
    })
  }
  const installed = invocation.version === undefined ? yield* installedTsrsVersion(context) : undefined
  const version = yield* selectVersion("tsrs", "tsrs", invocation.version, installed)
  const executable = yield* requireArtifact(yield* tsrsExecutable(context, version))
  // tsrs loads its lib*.d.ts files from the directory of its binary.
  const libraryPath = path.join(path.dirname(executable), "lib.d.ts")
  if (!(yield* fs.exists(libraryPath).pipe(Effect.mapError(toProxyError)))) {
    return yield* new ProxyError({
      reason: `Missing ${libraryPath}: tsrs needs its lib*.d.ts files next to the packaged binary ${executable}. `
        + `Reinstall @effect/tsgo-${context.platform}-${context.arch}.`
    })
  }
  yield* ensureExecutable(executable, context.platform)
  return { file: executablePath(executable, context.platform), args: invocation.args, env: context.env } satisfies
    ProxyCommand
})

export interface OxlintInstallation {
  readonly oxlint: PackageMetadata
  readonly tsgolint: PackageMetadata | undefined
}

// The oxlint package of the project, installed directly or through vite-plus, with the oxlint-tsgolint package next to
// it when installed.
export const findOxlintInstallation = (cwd: string) => Effect.gen(function*() {
  const path = yield* Path.Path
  const find = (require: NodeJS.Require, packageName: string) =>
    findInstalledPackage(require, packageName).pipe(Effect.mapError(toProxyError))
  const cwdRequire = nodeModule.createRequire(path.join(cwd, "noop.js"))
  const vitePlus = yield* find(cwdRequire, "vite-plus")
  const scopes = [cwdRequire, ...(vitePlus === undefined ? [] : [nodeModule.createRequire(vitePlus.packageJsonPath)])]
  for (const require of scopes) {
    const oxlint = yield* find(require, "oxlint")
    if (oxlint !== undefined) return { oxlint, tsgolint: yield* find(require, "oxlint-tsgolint") } satisfies OxlintInstallation
  }
  return undefined
})

export const oxlintBinPath = (oxlint: PackageMetadata) => Effect.gen(function*() {
  const path = yield* Path.Path
  const bin = typeof oxlint.bin === "string" ? oxlint.bin : oxlint.bin?.oxlint
  if (bin === undefined) {
    return yield* new ProxyError({ reason: `${oxlint.packageJsonPath} does not declare an oxlint bin entry.` })
  }
  return path.resolve(path.dirname(oxlint.packageJsonPath), bin)
})

const supportedInstalledVersion = (component: "oxlint" | "oxlint-tsgolint", installed: PackageMetadata) => {
  const versions = supportedVersions(component)
  return versions.includes(installed.version)
    ? Effect.succeed(installed.version)
    : Effect.fail(
      new UnsupportedProxyVersionError({
        packageName: component,
        version: installed.version,
        source: "installed",
        supportedVersions: versions,
        tags: {}
      })
    )
}

// Oxlint runs its own CLI with the patched native binding (NAPI_RS_NATIVE_LIBRARY_PATH, read by its napi-rs loader)
// and the patched tsgolint (OXLINT_TSGOLINT_PATH, read by the binding). The CLI runs without this process's
// execArgv: those are flags for effect-tsgo itself, and Oxlint honors NODE_OPTIONS.
export const oxlintCommand = (
  context: ProxyContext,
  args: ReadonlyArray<string>,
  installation: OxlintInstallation
) => Effect.gen(function*() {
  const target = yield* Effect.try({
    try: () => experimentalOxlintTarget(context.platform, context.arch, context.platform === "linux" && context.isGlibc()),
    catch: (error) => new ProxyError({ reason: error instanceof Error ? error.message : String(error) })
  })
  const oxlintVersion = yield* supportedInstalledVersion("oxlint", installation.oxlint)
  const binding = yield* requireArtifact(
    yield* packagedComponentPath(context.packageDirectory, "oxlint", oxlintVersion, target.oxlintBinding)
  )
  let tsgolint: string | undefined
  if (installation.tsgolint !== undefined) {
    const tsgolintVersion = yield* supportedInstalledVersion("oxlint-tsgolint", installation.tsgolint)
    tsgolint = yield* requireArtifact(
      yield* packagedComponentPath(context.packageDirectory, "oxlint-tsgolint", tsgolintVersion, target.tsgolintExecutable)
    )
    yield* ensureExecutable(tsgolint, context.platform)
  }
  const bin = yield* oxlintBinPath(installation.oxlint)
  return {
    file: context.execPath,
    args: [bin, ...args],
    env: {
      ...context.env,
      NAPI_RS_NATIVE_LIBRARY_PATH: binding,
      ...(tsgolint === undefined ? {} : { OXLINT_TSGOLINT_PATH: tsgolint })
    }
  } satisfies ProxyCommand
})

const resolveOxlint = (context: ProxyContext, invocation: ProxyInvocation) => Effect.gen(function*() {
  if (invocation.version !== undefined) {
    return yield* new ProxyError({
      reason: `oxlint@${invocation.version} is not supported: the packaged Oxlint build is selected by the installed `
        + "oxlint version. Run effect-tsgo oxlint instead."
    })
  }
  if (context.platform === "linux" && !context.isGlibc()) {
    return yield* new ProxyError({ reason: "Linux musl is not supported by the packaged Oxlint integration." })
  }
  const installation = yield* findOxlintInstallation(context.cwd)
  if (installation === undefined) {
    return yield* new ProxyError({
      reason: `Unable to resolve oxlint from ${context.cwd}. Install oxlint (or vite-plus) in the project.`
    })
  }
  return yield* oxlintCommand(context, invocation.args, installation)
})

export const resolveProxyCommand = (invocation: ProxyInvocation, context: ProxyContext) => {
  switch (invocation.tool) {
    case "tsc":
      return resolveTsc(context, invocation)
    case "tsrs":
      return resolveTsrs(context, invocation)
    case "oxlint":
      return resolveOxlint(context, invocation)
  }
}
