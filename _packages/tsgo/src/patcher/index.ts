import { randomUUID } from "node:crypto"
import * as nodeModule from "node:module"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import * as Scope from "effect/Scope"
import metadataJson from "../metadata.json" with { type: "json" }
import { discoverBinaries, requireComponents, selectComponents } from "./discovery.js"
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

const exists = (fs: FileSystem.FileSystem, filePath: string) => fs.exists(filePath).pipe(
  Effect.mapError((error) => new PatcherError({ reason: `Unable to inspect ${filePath}: ${error.message}` }))
)

export interface ResolvedReplacement {
  readonly path: string
}

export type ReplacementResolver = (
  target: DiscoveredBinary
) => Effect.Effect<
  ResolvedReplacement,
  PatcherError | ReplacementUnavailableError,
  FileSystem.FileSystem | Path.Path | Scope.Scope
>

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
  const source = yield* fs.readFileString(target.binaryPath).pipe(
    Effect.mapError((error) => new PatcherError({
      reason: `Unable to read Oxlint declarations at ${target.binaryPath}: ${error.message}`
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
  return { path: replacementPath }
})

const resolvePlatformPackage = (target: DiscoveredBinary) => Effect.gen(function*() {
  const path = yield* Path.Path
  const packageName = `@effect/tsgo-${process.platform}-${process.arch}`
  const selfRequire = nodeModule.createRequire(import.meta.url)
  const packageJsonPath = yield* Effect.try({
    try: () => selfRequire.resolve(`${packageName}/package.json`),
    catch: () => new ReplacementUnavailableError({
      target,
      reason: `Unable to resolve ${packageName}; no packaged replacement is available.`
    })
  })
  return { packageDir: path.dirname(packageJsonPath) }
})

export const resolveReplacement: ReplacementResolver = (target) => Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  if (target.component === "oxlint-dts") return yield* resolveOxlintDeclarations(target)
  const platform = yield* resolvePlatformPackage(target)
  const replacementPath = path.join(
    platform.packageDir,
    "artifacts",
    target.component,
    target.packageVersion,
    path.basename(target.binaryPath)
  )
  if (!(yield* exists(fs, replacementPath))) {
    return yield* new ReplacementUnavailableError({
      target,
      reason: `Missing packaged artifact ${replacementPath}.`
    })
  }
  return { path: replacementPath }
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
  const skipped: Array<SkippedTarget> = []
  const available: Array<{ readonly target: DiscoveredBinary; readonly replacementPath: string }> = []

  for (const target of targets) {
    const backupPath = `${target.binaryPath}.original`
    const targetExists = yield* exists(fs, target.binaryPath)
    const backupExists = yield* exists(fs, backupPath)
    if (!targetExists) {
      return yield* new PatcherError({ reason: `Installed binary does not exist: ${target.binaryPath}` })
    }
    if (backupExists) {
      skipped.push({
        target,
        reason: "already-patched",
        message: `Backup already exists at ${backupPath}; ${target.component} is already patched.`
      })
      continue
    }
    const replacement = yield* resolver(target).pipe(
      Effect.catchTag("ReplacementUnavailableError", (error) => {
        if (!options.skipMissing) return Effect.fail(error)
        skipped.push({ target, reason: "replacement-unavailable", message: error.message })
        return Effect.succeed(undefined)
      })
    )
    if (replacement === undefined) continue
    available.push({ target, replacementPath: replacement.path })
  }

  for (const { replacementPath, target } of available) {
    const backupPath = `${target.binaryPath}.original`
    operations.push({ _tag: "Rename", sourcePath: target.binaryPath, destinationPath: backupPath })
    operations.push({
      _tag: "Copy",
      sourcePath: replacementPath,
      destinationPath: target.binaryPath
    })
    if (!target.binaryPath.endsWith(".node") && target.component !== "oxlint-dts") {
      operations.push({ _tag: "Chmod", path: target.binaryPath, mode: 0o755 })
    }
  }

  return { operations, cleanup: [], skipped } satisfies PreparedPatch
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
