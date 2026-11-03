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
import { type ProxyInvocation, type ProxyTool, tsrsLintFlag, type TsrsLintSelection } from "./argv.js"
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
  // The Node.js or Bun executable that runs the Oxlint and vite-plus CLIs.
  readonly execPath: string
  readonly platform: NodeJS.Platform
  readonly arch: string
  readonly isGlibc: () => boolean
  // The directory of the @effect/tsgo-<platform>-<arch> package.
  readonly packageDirectory: string
  // lib/oxlint-bridge.cjs of @effect/tsgo, preloaded into the processes that load the Oxlint binding.
  readonly oxlintBridge: string
}

type VersionedComponent = "typescript" | "tsrs"

const upstreamTags = (component: VersionedComponent): Readonly<Record<string, string>> => upstreamJson.tags[component]

// The packaged build to run: the requested version or tag, else the installed version when a build of it ships, else
// the latest supported version.
export const selectVersion = (
  tool: Exclude<ProxyTool, "oxlint" | "vp">,
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
  yield* rejectTsrsLint(invocation)
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

// The packaged tsrs binary of the requested version or tag, else of the installed tsrs version when a build of it
// ships, else of the latest one.
const packagedTsrs = (context: ProxyContext, requested: string | undefined) => Effect.gen(function*() {
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
  const installed = requested === undefined ? yield* installedTsrsVersion(context) : undefined
  const version = yield* selectVersion("tsrs", "tsrs", requested, installed)
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
  return executablePath(executable, context.platform)
})

const resolveTsrs = (context: ProxyContext, invocation: ProxyInvocation) => Effect.gen(function*() {
  yield* rejectTsrsLint(invocation)
  return {
    file: yield* packagedTsrs(context, invocation.version),
    args: invocation.args,
    env: context.env
  } satisfies ProxyCommand
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

// The absolute path of a package's bin entry.
const packageBinPath = (installed: PackageMetadata, binName: string) => Effect.gen(function*() {
  const path = yield* Path.Path
  const bin = typeof installed.bin === "string" ? installed.bin : installed.bin?.[binName]
  if (bin === undefined) {
    return yield* new ProxyError({ reason: `${installed.packageJsonPath} does not declare a ${binName} bin entry.` })
  }
  return path.resolve(path.dirname(installed.packageJsonPath), bin)
})

export const oxlintBinPath = (oxlint: PackageMetadata) => packageBinPath(oxlint, "oxlint")

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

// The type-aware linter the patched Oxlint binding runs: the packaged tsgolint, or the packaged tsrs in its lint mode.
export interface TypeAwareLinter {
  readonly kind: "tsgolint" | "tsrs"
  readonly executable: string
}

export interface OxlintArtifacts {
  readonly binding: string
  readonly linter: TypeAwareLinter | undefined
}

// The packaged tsgolint of the installed oxlint-tsgolint version, undefined when oxlint-tsgolint is not installed.
const packagedTsgolint = (
  context: ProxyContext,
  installation: OxlintInstallation,
  target: ReturnType<typeof experimentalOxlintTarget>
) => Effect.gen(function*() {
  if (installation.tsgolint === undefined) return undefined
  const tsgolintVersion = yield* supportedInstalledVersion("oxlint-tsgolint", installation.tsgolint)
  const tsgolint = yield* requireArtifact(
    yield* packagedComponentPath(context.packageDirectory, "oxlint-tsgolint", tsgolintVersion, target.tsgolintExecutable)
  )
  yield* ensureExecutable(tsgolint, context.platform)
  return { kind: "tsgolint", executable: tsgolint } satisfies TypeAwareLinter
})

// The packaged tsrs of `--tsrs-lint[=<version|tag>]`, which replaces tsgolint: an installed oxlint-tsgolint is not used.
const packagedTsrsLint = (context: ProxyContext, selection: TsrsLintSelection) => Effect.gen(function*() {
  if (selection.version === "") {
    return yield* new ProxyError({ reason: `Expected a tsrs version or tag after ${tsrsLintFlag}=.` })
  }
  return { kind: "tsrs", executable: yield* packagedTsrs(context, selection.version) } satisfies TypeAwareLinter
})

// The packaged Oxlint binding of the installed oxlint version and the type-aware linter it runs: the packaged tsrs
// with `--tsrs-lint`, else the packaged tsgolint of the installed oxlint-tsgolint version, else none.
export const oxlintArtifacts = (
  context: ProxyContext,
  installation: OxlintInstallation,
  tsrsLint: TsrsLintSelection | undefined
) => Effect.gen(function*() {
  const target = yield* Effect.try({
    try: () => experimentalOxlintTarget(context.platform, context.arch, context.platform === "linux" && context.isGlibc()),
    catch: (error) => new ProxyError({ reason: error instanceof Error ? error.message : String(error) })
  })
  const oxlintVersion = yield* supportedInstalledVersion("oxlint", installation.oxlint)
  const binding = yield* requireArtifact(
    yield* packagedComponentPath(context.packageDirectory, "oxlint", oxlintVersion, target.oxlintBinding)
  )
  const linter = tsrsLint === undefined
    ? yield* packagedTsgolint(context, installation, target)
    : yield* packagedTsrsLint(context, tsrsLint)
  return { binding, linter } satisfies OxlintArtifacts
})

// The bridge path as written into NODE_OPTIONS and BUN_OPTIONS. Node.js unescapes backslashes inside quotes and Bun
// splits on them, so Windows paths use forward slashes, which both module loaders accept.
export const preloadPath = (bridge: string, platform: NodeJS.Platform) => {
  const preload = platform === "win32" ? bridge.replaceAll("\\", "/") : bridge
  return /["\\]/.test(preload)
    ? Effect.fail(
      new ProxyError({
        reason: `Unable to preload ${bridge}: NODE_OPTIONS and BUN_OPTIONS cannot carry a path with quotes or backslashes.`
      })
    )
    : Effect.succeed(preload)
}

// Puts an option in front of the caller's options, once: a nested effect-tsgo run inherits the bridge already.
const prependOption = (options: string | undefined, option: string, preload: string) =>
  options === undefined || options.trim() === "" ? option : options.includes(preload) ? options : `${option} ${options}`

// The variables that select the type-aware linter of the patched binding: EFFECT_TSGO_TSGOLINT_PATH names it, and
// EFFECT_TSGO_TSGOLINT_MODE=tsrs makes the binding run it as `tsrs --lint headless ...`. Without a linter both stay as
// inherited, so that a nested run keeps its parent's pair; with one, the mode always matches the path.
const linterEnv = (env: NodeJS.ProcessEnv, linter: TypeAwareLinter | undefined): NodeJS.ProcessEnv => {
  if (linter === undefined) return env
  const { EFFECT_TSGO_TSGOLINT_MODE: _mode, ...rest } = env
  return {
    ...rest,
    EFFECT_TSGO_TSGOLINT_PATH: linter.executable,
    ...(linter.kind === "tsrs" ? { EFFECT_TSGO_TSGOLINT_MODE: "tsrs" } : {})
  }
}

// The environment that loads the patched Oxlint in every Node.js and Bun process started with it: the bridge maps the
// stock binding to the patched one at process.dlopen, and the patched binding runs the type-aware linter of
// EFFECT_TSGO_TSGOLINT_PATH (vite-plus overwrites OXLINT_TSGOLINT_PATH). Bun reads BUN_OPTIONS instead of NODE_OPTIONS
// and needs the `--preload=` form under `bun run`.
export const oxlintBridgeEnv = (context: ProxyContext, artifacts: OxlintArtifacts) => Effect.gen(function*() {
  const preload = yield* preloadPath(context.oxlintBridge, context.platform)
  const env: NodeJS.ProcessEnv = {
    ...linterEnv(context.env, artifacts.linter),
    NODE_OPTIONS: prependOption(context.env.NODE_OPTIONS, `--require "${preload}"`, preload),
    BUN_OPTIONS: prependOption(context.env.BUN_OPTIONS, `"--preload=${preload}"`, preload),
    EFFECT_TSGO_OXLINT_BINDING: artifacts.binding
  }
  return env
})

// Oxlint runs its own CLI with the patched binding and tsgolint, through the bridge environment. The CLI runs without
// this process's execArgv: those are flags for effect-tsgo itself.
export const oxlintCommand = (
  context: ProxyContext,
  args: ReadonlyArray<string>,
  installation: OxlintInstallation,
  tsrsLint: TsrsLintSelection | undefined = undefined
) => Effect.gen(function*() {
  const artifacts = yield* oxlintArtifacts(context, installation, tsrsLint)
  return {
    file: context.execPath,
    args: [yield* oxlintBinPath(installation.oxlint), ...args],
    env: yield* oxlintBridgeEnv(context, artifacts)
  } satisfies ProxyCommand
})

const rejectVersion = (tool: "oxlint" | "vp", version: string | undefined) =>
  version === undefined ? Effect.void : Effect.fail(
    new ProxyError({
      reason: `${tool}@${version} is not supported: the packaged Oxlint build is selected by the installed `
        + `oxlint version. Run effect-tsgo ${tool} instead.`
    })
  )

// `--tsrs-lint` selects the linter of Oxlint, so it only goes with the tools that run Oxlint.
const rejectTsrsLint = (invocation: ProxyInvocation) =>
  invocation.tsrsLint === undefined ? Effect.void : Effect.fail(
    new ProxyError({
      reason: `${tsrsLintFlag} selects the type-aware linter of Oxlint: use it with effect-tsgo ${tsrsLintFlag} oxlint `
        + `or effect-tsgo ${tsrsLintFlag} vp.`
    })
  )

const rejectMusl = (context: ProxyContext) =>
  context.platform === "linux" && !context.isGlibc()
    ? Effect.fail(new ProxyError({ reason: "Linux musl is not supported by the packaged Oxlint integration." }))
    : Effect.void

const resolveOxlint = (context: ProxyContext, invocation: ProxyInvocation) => Effect.gen(function*() {
  yield* rejectVersion("oxlint", invocation.version)
  yield* rejectMusl(context)
  const installation = yield* findOxlintInstallation(context.cwd)
  if (installation === undefined) {
    return yield* new ProxyError({
      reason: `Unable to resolve oxlint from ${context.cwd}. Install oxlint (or vite-plus) in the project.`
    })
  }
  return yield* oxlintCommand(context, invocation.args, installation, invocation.tsrsLint)
})

// vite-plus runs Oxlint as a child process from its own native core, so the CLI runs with the bridge environment,
// which reaches that child. vite-plus resolves oxlint from the project before its own dependencies, as
// findOxlintInstallation does.
const resolveVitePlus = (context: ProxyContext, invocation: ProxyInvocation) => Effect.gen(function*() {
  yield* rejectVersion("vp", invocation.version)
  yield* rejectMusl(context)
  const path = yield* Path.Path
  const vitePlus = yield* findInstalledPackage(nodeModule.createRequire(path.join(context.cwd, "noop.js")), "vite-plus")
    .pipe(Effect.mapError(toProxyError))
  if (vitePlus === undefined) {
    return yield* new ProxyError({ reason: `Unable to resolve vite-plus from ${context.cwd}. Install vite-plus in the project.` })
  }
  const installation = yield* findOxlintInstallation(context.cwd)
  if (installation === undefined) {
    return yield* new ProxyError({ reason: `Unable to resolve the oxlint package of vite-plus from ${context.cwd}.` })
  }
  const artifacts = yield* oxlintArtifacts(context, installation, invocation.tsrsLint)
  return {
    file: context.execPath,
    args: [yield* packageBinPath(vitePlus, "vp"), ...invocation.args],
    env: yield* oxlintBridgeEnv(context, artifacts)
  } satisfies ProxyCommand
})

export const resolveProxyCommand = (invocation: ProxyInvocation, context: ProxyContext) => {
  switch (invocation.tool) {
    case "tsc":
      return resolveTsc(context, invocation)
    case "tsrs":
      return resolveTsrs(context, invocation)
    case "oxlint":
      return resolveOxlint(context, invocation)
    case "vp":
      return resolveVitePlus(context, invocation)
  }
}
