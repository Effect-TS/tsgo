import * as nodeModule from "node:module"
import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { randomUUID } from "node:crypto"
import * as upstreamJson from "../../upstream.json" with { type: "json" }

export interface ResolvedPatchTarget {
  readonly label: string
  readonly targetPath: string
  readonly replacementPath: string
  readonly executable: boolean
}

export class ExperimentalOxlintPatchError extends Data.TaggedError("ExperimentalOxlintPatchError")<{
  readonly reason: string
}> {
  get message(): string {
    return `Unable to patch the experimental Oxlint integration: ${this.reason}`
  }
}

interface PackageMetadata {
  readonly packageJsonPath: string
  readonly version: string
}

const oxlintProfile = upstreamJson.profiles.find((profile) => profile.kind === "oxlint")
if (oxlintProfile === undefined || oxlintProfile.kind !== "oxlint") {
  throw new Error("Missing Oxlint profile in upstream.json")
}
const supportedOxlintVersion = oxlintProfile.oxlint!.npmVersion
const supportedTsgolintVersion = oxlintProfile.tsgolint!.npmVersion

const readPackageMetadata = (cwd: string, packageName: string) => Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const require = nodeModule.createRequire(path.join(cwd, "noop.js"))
  const packageJsonPath = yield* Effect.try({
    try: () => require.resolve(`${packageName}/package.json`),
    catch: () => new ExperimentalOxlintPatchError({ reason: `Unable to resolve ${packageName}.` })
  })
  const text = yield* fs.readFileString(packageJsonPath).pipe(
    Effect.mapError(() => new ExperimentalOxlintPatchError({ reason: `Unable to read ${packageJsonPath}.` }))
  )
  const metadata = yield* Effect.try({
    try: () => JSON.parse(text) as { readonly version?: unknown },
    catch: () => new ExperimentalOxlintPatchError({ reason: `Unable to parse ${packageJsonPath}.` })
  })
  if (typeof metadata.version !== "string") {
    return yield* new ExperimentalOxlintPatchError({ reason: `${packageJsonPath} does not contain a version.` })
  }
  return { packageJsonPath, version: metadata.version } satisfies PackageMetadata
})

const isGlibc = () => {
  if (process.platform !== "linux") return false
  const report = process.report?.getReport()
  const header = (report as { readonly header?: { readonly glibcVersionRuntime?: unknown } } | undefined)?.header
  return typeof header?.glibcVersionRuntime === "string"
}

export const experimentalOxlintTarget = (
  platform: NodeJS.Platform,
  arch: string,
  glibc: boolean
) => {
  if (arch !== "x64" && arch !== "arm64") {
    throw new ExperimentalOxlintPatchError({ reason: `Unsupported architecture ${arch}.` })
  }
  if (platform === "linux" && !glibc) {
    throw new ExperimentalOxlintPatchError({ reason: "Linux musl is not supported by the packaged Oxlint integration." })
  }
  if (platform !== "linux" && platform !== "darwin" && platform !== "win32") {
    throw new ExperimentalOxlintPatchError({ reason: `Unsupported platform ${platform}.` })
  }

  const packageTarget = `${platform}-${arch}`
  const codeTarget = platform === "linux" ? `${packageTarget}-gnu` : packageTarget
  return {
    codeTarget,
    effectPackage: `@effect/tsgo-${packageTarget}`,
    oxlintPackage: `@oxlint/${codeTarget}`,
    tsgolintPackage: `@oxlint-tsgolint/${packageTarget}`,
    tsgolintExecutable: platform === "win32" ? "tsgolint.exe" : "tsgolint"
  }
}

const nextBackupPath = (targetPath: string) => Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  const base = `${targetPath}.original`
  if (!(yield* fs.exists(base))) return base
  for (let counter = 1; counter <= 100; counter++) {
    const candidate = `${base}.${counter}`
    if (!(yield* fs.exists(candidate))) return candidate
  }
  return yield* new ExperimentalOxlintPatchError({
    reason: `Too many backup files exist for ${targetPath}.`
  })
})

export const patchResolvedTargets = Effect.fnUntraced(function*(targets: ReadonlyArray<ResolvedPatchTarget>) {
  const fs = yield* FileSystem.FileSystem
  const plans: Array<ResolvedPatchTarget & { readonly backupPath: string }> = []
  for (const target of targets) {
    if (!(yield* fs.exists(target.targetPath))) {
      return yield* new ExperimentalOxlintPatchError({ reason: `Target does not exist: ${target.targetPath}` })
    }
    if (!(yield* fs.exists(target.replacementPath))) {
      return yield* new ExperimentalOxlintPatchError({ reason: `Replacement does not exist: ${target.replacementPath}` })
    }
    plans.push({ ...target, backupPath: yield* nextBackupPath(target.targetPath) })
  }

  const applied: typeof plans = []
  yield* Effect.gen(function*() {
    for (const plan of plans) {
      yield* fs.rename(plan.targetPath, plan.backupPath)
      applied.push(plan)
      yield* fs.copyFile(plan.replacementPath, plan.targetPath)
      if (plan.executable) yield* fs.chmod(plan.targetPath, 0o755)
      yield* Console.log(`Patched ${plan.label} at ${plan.targetPath}`)
    }
  }).pipe(
    Effect.mapError((error) => new ExperimentalOxlintPatchError({ reason: String(error) })),
    Effect.catchCause((cause) =>
      Effect.gen(function*() {
        for (const plan of [...applied].reverse()) {
          yield* fs.remove(plan.targetPath, { force: true }).pipe(Effect.ignore)
          yield* fs.rename(plan.backupPath, plan.targetPath).pipe(Effect.ignore)
        }
        return yield* Effect.failCause(cause)
      }))
  )
})

export const unpatchResolvedTargets = Effect.fnUntraced(function*(targets: ReadonlyArray<ResolvedPatchTarget>) {
  const fs = yield* FileSystem.FileSystem
  for (const target of targets) {
    const backupPath = `${target.targetPath}.original`
    if (!(yield* fs.exists(backupPath))) {
      yield* Console.error(`No backup found at ${backupPath}. Nothing to restore.`)
      continue
    }
    if (yield* fs.exists(target.targetPath)) {
      yield* fs.rename(target.targetPath, `${target.targetPath}.${randomUUID()}.patched`).pipe(
        Effect.mapError((error) => new ExperimentalOxlintPatchError({ reason: String(error) }))
      )
    }
    yield* fs.rename(backupPath, target.targetPath).pipe(
      Effect.mapError((error) => new ExperimentalOxlintPatchError({ reason: String(error) }))
    )
    yield* Console.log(`Restored original ${target.label} at ${target.targetPath}`)
  }
})

export const resolveExperimentalOxlintTargets = (cwd: string) => Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const target = experimentalOxlintTarget(process.platform, process.arch, isGlibc())
  const installedOxlint = yield* readPackageMetadata(cwd, "oxlint")
  const installedTsgolint = yield* readPackageMetadata(cwd, "oxlint-tsgolint")
  if (installedOxlint.version !== supportedOxlintVersion) {
    return yield* new ExperimentalOxlintPatchError({
      reason: `Installed oxlint ${installedOxlint.version} does not match supported ${supportedOxlintVersion}.`
    })
  }
  if (installedTsgolint.version !== supportedTsgolintVersion) {
    return yield* new ExperimentalOxlintPatchError({
      reason: `Installed oxlint-tsgolint ${installedTsgolint.version} does not match supported ${supportedTsgolintVersion}.`
    })
  }

  const oxlintRequire = nodeModule.createRequire(installedOxlint.packageJsonPath)
  const tsgolintRequire = nodeModule.createRequire(installedTsgolint.packageJsonPath)
  const selfRequire = nodeModule.createRequire(import.meta.url)
  const oxlintBinding = yield* Effect.try({
    try: () => oxlintRequire.resolve(target.oxlintPackage),
    catch: () => new ExperimentalOxlintPatchError({ reason: `Unable to resolve ${target.oxlintPackage}.` })
  })
  const tsgolintPackageJson = yield* Effect.try({
    try: () => tsgolintRequire.resolve(`${target.tsgolintPackage}/package.json`),
    catch: () => new ExperimentalOxlintPatchError({ reason: `Unable to resolve ${target.tsgolintPackage}.` })
  })
  const effectPackageJson = yield* Effect.try({
    try: () => selfRequire.resolve(`${target.effectPackage}/package.json`),
    catch: () => new ExperimentalOxlintPatchError({ reason: `Unable to resolve ${target.effectPackage}.` })
  })
  const effectLib = path.join(path.dirname(effectPackageJson), "lib")
  const targets: ReadonlyArray<ResolvedPatchTarget> = [
    {
      label: "Oxlint binding",
      targetPath: oxlintBinding,
      replacementPath: path.join(effectLib, `oxlint.${target.codeTarget}.node`),
      executable: false
    },
    {
      label: "tsgolint",
      targetPath: path.join(path.dirname(tsgolintPackageJson), target.tsgolintExecutable),
      replacementPath: path.join(effectLib, target.tsgolintExecutable),
      executable: true
    }
  ]
  for (const resolved of targets) {
    if (!(yield* fs.exists(resolved.replacementPath))) {
      return yield* new ExperimentalOxlintPatchError({ reason: `Missing packaged artifact ${resolved.replacementPath}.` })
    }
  }
  return targets
})

export const patchExperimentalOxlint = Effect.gen(function*() {
  const targets = yield* resolveExperimentalOxlintTargets(process.cwd())
  yield* patchResolvedTargets(targets)
})

export const unpatchExperimentalOxlint = Effect.gen(function*() {
  const targets = yield* resolveExperimentalOxlintTargets(process.cwd())
  yield* unpatchResolvedTargets(targets)
})
