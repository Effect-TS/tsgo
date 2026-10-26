import { randomUUID } from "node:crypto"
import * as nodeModule from "node:module"
import * as Crypto from "effect/Crypto"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import * as Scope from "effect/Scope"
import * as pkgJson from "../../package.json" with { type: "json" }
import metadataJson from "../metadata.json" with { type: "json" }
import upstreamJson from "../../upstream.json" with { type: "json" }
import { discoverBinaries, isGlibc, requireComponents, selectComponents } from "./discovery.js"
import { hashBytes, hashFile } from "./fileHash.js"
import type {
  Component,
  DiscoveredBinary,
  FileSystemOperation,
  PreparedPlan,
  RemoveOperation,
  SkippedTarget
} from "./types.js"

export * from "./discovery.js"
export * from "./types.js"

export class PatcherError extends Data.TaggedError("PatcherError")<{ readonly reason: string }> {
  get message(): string {
    return this.reason
  }
}

export class ReplacementUnavailableError extends Data.TaggedError("ReplacementUnavailableError")<{
  readonly target: DiscoveredBinary
  readonly reason: string
}> {
  get message(): string {
    return this.reason
  }
}

export class UnsupportedTargetPackageVersionError extends Data.TaggedError("UnsupportedTargetPackageVersionError")<{
  readonly target: DiscoveredBinary
  readonly supportedVersions: ReadonlyArray<string>
}> {
  get message(): string {
    return `Package ${this.target.packageName} version ${this.target.packageVersion} is not supported by @effect/tsgo version ${pkgJson.version}. `
      + `Either change @effect/tsgo to a compatible version or set ${this.target.packageName} to one of the supported versions: ${
        this.supportedVersions.join(", ")
      }.`
  }
}

const exists = (fs: FileSystem.FileSystem, filePath: string) => fs.exists(filePath).pipe(
  Effect.mapError((error) => new PatcherError({ reason: `Unable to inspect ${filePath}: ${error.message}` }))
)

export interface ResolvedReplacement {
  readonly path: string
  readonly fileHash: string
}

export type ReplacementResolver = (
  target: DiscoveredBinary
) => Effect.Effect<
  ResolvedReplacement,
  PatcherError | ReplacementUnavailableError | UnsupportedTargetPackageVersionError,
  Crypto.Crypto | FileSystem.FileSystem | Path.Path | Scope.Scope
>

export const supportedVersions = (component: Exclude<Component, "oxlint-dts">): ReadonlyArray<string> =>
  Object.keys(upstreamJson.components[component]).sort()

const toKebabCase = (value: string): string => {
  let result = ""
  const characters = [...value]
  for (let index = 0; index < characters.length; index++) {
    const current = characters[index]!
    if (current >= "A" && current <= "Z") {
      const previous = characters[index - 1]
      const next = characters[index + 1]
      const previousIsLower = previous !== undefined && previous >= "a" && previous <= "z"
      const nextIsLower = next !== undefined && next >= "a" && next <= "z"
      if (index > 0 && (previousIsLower || nextIsLower)) result += "-"
      result += current.toLowerCase()
    } else {
      result += current
    }
  }
  return result
}

export const renderOxlintDeclarations = (source: string): string => {
  const pluginDeclaration = /^type LintPluginOptionsSchema = ([^;\n]+);$/m
  const pluginMatches = source.match(new RegExp(pluginDeclaration.source, "gm"))
  if (pluginMatches?.length !== 1) {
    throw new PatcherError({ reason: "Unable to locate the Oxlint plugin declaration." })
  }
  if (pluginMatches[0]!.includes('"effecttsgo"')) {
    throw new PatcherError({ reason: "The Oxlint declaration already contains the effecttsgo plugin." })
  }

  const ruleMapMarker = "interface DummyRuleMap {\n"
  if (source.split(ruleMapMarker).length !== 2) {
    throw new PatcherError({ reason: "Unable to locate the Oxlint rule map declaration." })
  }
  const effectRules = metadataJson.rules
    .map(({ name }) => `  "effecttsgo/${toKebabCase(name)}"?: RuleNoConfig;`)
    .sort()
    .join("\n")

  return source
    .replace(pluginDeclaration, 'type LintPluginOptionsSchema = "effecttsgo" | $1;')
    .replace(ruleMapMarker, `${ruleMapMarker}${effectRules}\n`)
}

const resolveOxlintDeclarations = (target: DiscoveredBinary) => Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const backupPath = `${target.binaryPath}.original`
  const sourcePath = (yield* exists(fs, backupPath)) ? backupPath : target.binaryPath
  const source = yield* fs.readFileString(sourcePath).pipe(
    Effect.mapError((error) => new PatcherError({
      reason: `Unable to read Oxlint declarations at ${sourcePath}: ${error.message}`
    }))
  )
  const replacement = yield* Effect.try({
    try: () => renderOxlintDeclarations(source),
    catch: (error) => error instanceof PatcherError
      ? error
      : new PatcherError({ reason: `Unable to generate Oxlint declarations: ${String(error)}` })
  })
  const replacementPath = yield* fs.makeTempFileScoped({ prefix: "effect-tsgo-oxlint-", suffix: ".d.ts" }).pipe(
    Effect.mapError((error) => new PatcherError({
      reason: `Unable to create a temporary Oxlint declaration file: ${error.message}`
    }))
  )
  yield* fs.writeFileString(replacementPath, replacement).pipe(
    Effect.mapError((error) => new PatcherError({
      reason: `Unable to write generated Oxlint declarations at ${replacementPath}: ${error.message}`
    }))
  )
  const fileHash = yield* hashBytes(replacement).pipe(
    Effect.mapError((error) => new PatcherError({
      reason: `Unable to hash generated Oxlint declarations: ${error.message}`
    }))
  )
  return { path: replacementPath, fileHash }
})

// The directory of this host's @effect/tsgo-<platform>-<arch> package, which ships the packaged artifacts.
export const resolvePlatformPackageDirectory = Effect.gen(function*() {
  const path = yield* Path.Path
  const packageName = `@effect/tsgo-${process.platform}-${process.arch}`
  const selfRequire = nodeModule.createRequire(import.meta.url)
  const packageJsonPath = yield* Effect.try({
    try: () => selfRequire.resolve(`${packageName}/package.json`),
    catch: () => new PatcherError({ reason: `Unable to resolve ${packageName}; no packaged replacement is available.` })
  })
  return path.dirname(packageJsonPath)
})

const resolvePlatformPackage = (target: DiscoveredBinary) => resolvePlatformPackageDirectory.pipe(
  Effect.map((packageDir) => ({ packageDir })),
  Effect.mapError(({ reason }) => new ReplacementUnavailableError({ target, reason }))
)

// The platforms @effect/tsgo ships a patched tsrs binary for (artifacts/tsrs/<tsrs version>/tsrs): tsrs's own glibc
// and macOS targets. Its musl targets are discovered but reported as unavailable.
export const tsrsReplacementTargets = ["darwin-arm64", "linux-x64-gnu", "linux-arm64-gnu"] as const

// The packaged build of a component version: the latest TypeScript compiler ships in lib/, everything else in
// artifacts/<component>/<version>/.
export const packagedComponentPath = (
  packageDir: string,
  component: Exclude<Component, "oxlint-dts">,
  version: string,
  fileName: string
) => Effect.gen(function*() {
  const path = yield* Path.Path
  return component === "typescript" && version === upstreamJson.tags.typescript.latest
    ? path.join(packageDir, "lib", fileName)
    : path.join(packageDir, "artifacts", component, version, fileName)
})

export const packagedArtifactPath = (packageDir: string, target: DiscoveredBinary) => Effect.gen(function*() {
  const path = yield* Path.Path
  const fileName = path.basename(target.binaryPath)
  return target.component === "oxlint-dts"
    ? path.join(packageDir, "artifacts", target.component, target.packageVersion, fileName)
    : yield* packagedComponentPath(packageDir, target.component, target.packageVersion, fileName)
})

export const resolveReplacement: ReplacementResolver = (target) => Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  if (target.component !== "typescript" && process.platform === "linux" && !isGlibc()) {
    return yield* new ReplacementUnavailableError({
      target,
      reason: target.component === "tsrs"
        ? `Linux musl is not supported by the packaged tsrs integration; it is built for ${tsrsReplacementTargets.join(", ")}.`
        : "Linux musl is not supported by the packaged Oxlint integration."
    })
  }
  if (target.component === "oxlint-dts") return yield* resolveOxlintDeclarations(target)
  const platform = yield* resolvePlatformPackage(target)
  const replacementPath = yield* packagedArtifactPath(platform.packageDir, target)
  if (!(yield* exists(fs, replacementPath))) {
    const versions = supportedVersions(target.component)
    if (!versions.includes(target.packageVersion)) {
      return yield* new UnsupportedTargetPackageVersionError({ target, supportedVersions: versions })
    }
    return yield* new ReplacementUnavailableError({ target, reason: `Missing packaged artifact ${replacementPath}.` })
  }
  const fileHash = yield* hashFile(replacementPath).pipe(
    Effect.mapError((error) => new PatcherError({
      reason: `Unable to hash packaged artifact ${replacementPath}: ${error.message}`
    }))
  )
  return { path: replacementPath, fileHash }
})

export interface PreparePatchOptions {
  readonly skipMissing: boolean
  readonly resolveReplacement?: ReplacementResolver
}

export type PreparedPatch = PreparedPlan

export const preparePatch = (
  targets: ReadonlyArray<DiscoveredBinary>,
  options: PreparePatchOptions
) => Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  const resolver = options.resolveReplacement ?? resolveReplacement
  const operations: Array<FileSystemOperation> = []
  const cleanup: Array<RemoveOperation> = []
  const skipped: Array<SkippedTarget> = []
  const available: Array<{
    readonly target: DiscoveredBinary
    readonly replacementPath: string
    readonly backupExists: boolean
  }> = []

  for (const target of targets) {
    const backupPath = `${target.binaryPath}.original`
    const targetExists = yield* exists(fs, target.binaryPath)
    const backupExists = yield* exists(fs, backupPath)
    if (!targetExists) {
      return yield* new PatcherError({ reason: `Installed binary does not exist: ${target.binaryPath}` })
    }
    const onUnavailable = (error: ReplacementUnavailableError | UnsupportedTargetPackageVersionError) => {
      if (!options.skipMissing) return Effect.fail(error)
      skipped.push({ target, reason: "replacement-unavailable", message: error.message })
      return Effect.succeed(undefined)
    }
    const replacement = yield* resolver(target).pipe(
      Effect.catchTags({
        ReplacementUnavailableError: onUnavailable,
        UnsupportedTargetPackageVersionError: onUnavailable
      })
    )
    if (replacement === undefined) continue
    if (backupExists && target.fileHash === replacement.fileHash) {
      skipped.push({
        target,
        reason: "already-patched",
        message: `${target.component} skipped because its hash matches the replacement.`
      })
      continue
    }
    available.push({ target, replacementPath: replacement.path, backupExists })
  }

  for (const { backupExists, replacementPath, target } of available) {
    const backupPath = `${target.binaryPath}.original`
    if (backupExists) {
      const quarantinePath = `${target.binaryPath}.${randomUUID()}.patched`
      operations.push({ _tag: "Rename", sourcePath: target.binaryPath, destinationPath: quarantinePath })
      cleanup.push({ _tag: "Remove", path: quarantinePath })
    } else {
      operations.push({ _tag: "Rename", sourcePath: target.binaryPath, destinationPath: backupPath })
    }
    operations.push({
      _tag: "Copy",
      sourcePath: replacementPath,
      destinationPath: target.binaryPath
    })
    if (!target.binaryPath.endsWith(".node") && target.component !== "oxlint-dts") {
      operations.push({ _tag: "Chmod", path: target.binaryPath, mode: 0o755 })
    }
  }

  return { operations, cleanup, skipped } satisfies PreparedPatch
})

export const prepareUnpatch = (targets: ReadonlyArray<DiscoveredBinary>) => Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  const operations: Array<FileSystemOperation> = []
  const cleanup: Array<RemoveOperation> = []
  const skipped: Array<SkippedTarget> = []
  for (const target of targets) {
    const backupPath = `${target.binaryPath}.original`
    if (!(yield* exists(fs, backupPath))) {
      skipped.push({
        target,
        reason: "no-backup",
        message: `No backup found at ${backupPath}. Nothing to restore.`
      })
      continue
    }
    if (yield* exists(fs, target.binaryPath)) {
      const quarantinePath = `${target.binaryPath}.${randomUUID()}.patched`
      operations.push({ _tag: "Rename", sourcePath: target.binaryPath, destinationPath: quarantinePath })
      cleanup.push({ _tag: "Remove", path: quarantinePath })
    }
    operations.push({ _tag: "Rename", sourcePath: backupPath, destinationPath: target.binaryPath })
  }
  return { operations, cleanup, skipped } satisfies PreparedPlan
})

const operationPaths = (operation: FileSystemOperation) => {
  switch (operation._tag) {
    case "Rename":
    case "Copy":
      return [operation.sourcePath, operation.destinationPath]
    case "Remove":
    case "Chmod":
      return [operation.path]
  }
}

const preflight = (plan: PreparedPlan) => Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  const state = new Map<string, boolean>()
  for (const operation of [...plan.operations, ...plan.cleanup]) {
    for (const filePath of operationPaths(operation)) {
      if (!state.has(filePath)) state.set(filePath, yield* exists(fs, filePath))
    }
  }
  for (const operation of plan.operations) {
    if (operation._tag === "Remove") continue
    if (operation._tag === "Chmod") {
      if (!state.get(operation.path)) {
        return yield* new PatcherError({ reason: `Transaction chmod target does not exist: ${operation.path}` })
      }
      continue
    }
    if (!state.get(operation.sourcePath)) {
      return yield* new PatcherError({ reason: `Transaction source does not exist: ${operation.sourcePath}` })
    }
    if (state.get(operation.destinationPath)) {
      return yield* new PatcherError({ reason: `Transaction destination already exists: ${operation.destinationPath}` })
    }
    if (operation._tag === "Rename") state.set(operation.sourcePath, false)
    state.set(operation.destinationPath, true)
  }
})

const runOperation = (operation: FileSystemOperation) => Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  switch (operation._tag) {
    case "Rename":
      yield* fs.rename(operation.sourcePath, operation.destinationPath)
      break
    case "Copy":
      yield* fs.copyFile(operation.sourcePath, operation.destinationPath)
      break
    case "Chmod":
      yield* fs.chmod(operation.path, operation.mode)
      break
    case "Remove":
      yield* fs.remove(operation.path, { force: true })
      break
  }
}).pipe(Effect.mapError((error) => new PatcherError({ reason: `Filesystem transaction failed: ${error.message}` })))

export const applyPlan = (plan: PreparedPlan) => Effect.gen(function*() {
  yield* preflight(plan)
  const inverses: Array<FileSystemOperation> = []
  yield* Effect.gen(function*() {
    for (const operation of plan.operations) {
      if (operation._tag === "Copy") {
        inverses.push({ _tag: "Remove", path: operation.destinationPath })
      }
      yield* runOperation(operation)
      if (operation._tag === "Rename") {
        inverses.push({
          _tag: "Rename",
          sourcePath: operation.destinationPath,
          destinationPath: operation.sourcePath
        })
      }
    }
  }).pipe(Effect.catch((error) => Effect.gen(function*() {
    const rollbackErrors: Array<string> = []
    for (const inverse of [...inverses].reverse()) {
      yield* runOperation(inverse).pipe(Effect.catch((rollbackError) => {
        rollbackErrors.push(rollbackError.message)
        return Effect.void
      }))
    }
    return yield* new PatcherError({
      reason: rollbackErrors.length === 0
        ? error.message
        : `${error.message} Rollback also failed: ${rollbackErrors.join("; ")}`
    })
  })))
  for (const operation of plan.cleanup) yield* runOperation(operation)
})

export interface PatchOptions {
  readonly cwd: string
  readonly components: ReadonlySet<Component>
  readonly preferredTypescriptPackage?: string
  readonly skipMissing: boolean
}

export interface UnpatchOptions {
  readonly cwd: string
  readonly components: ReadonlySet<Component>
  readonly preferredTypescriptPackage?: string
}

const discoverSelected = (
  cwd: string,
  components: ReadonlySet<Component>,
  preferredTypescriptPackage?: string
) => Effect.gen(function*() {
  const discovered = yield* discoverBinaries(cwd, preferredTypescriptPackage)
  const selectedComponents = new Set(components)
  if (selectedComponents.has("oxlint")) selectedComponents.add("oxlint-dts")
  return yield* requireComponents(selectComponents(discovered, selectedComponents), selectedComponents)
})

const changedTargets = (targets: ReadonlyArray<DiscoveredBinary>, skipped: ReadonlyArray<SkippedTarget>) => {
  const skippedPaths = new Set(skipped.map(({ target }) => target.binaryPath))
  return targets.filter((target) => !skippedPaths.has(target.binaryPath))
}

export const patch = (options: PatchOptions) => Effect.gen(function*() {
  const targets = yield* discoverSelected(options.cwd, options.components, options.preferredTypescriptPackage)
  const prepared = yield* preparePatch(targets, { skipMissing: options.skipMissing })
  yield* applyPlan(prepared)
  return { changed: changedTargets(targets, prepared.skipped), skipped: prepared.skipped }
})

export const unpatch = (options: UnpatchOptions) => Effect.gen(function*() {
  const targets = yield* discoverSelected(options.cwd, options.components, options.preferredTypescriptPackage)
  const prepared = yield* prepareUnpatch(targets)
  yield* applyPlan(prepared)
  return { changed: changedTargets(targets, prepared.skipped), skipped: prepared.skipped }
})
