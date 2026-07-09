import * as childProcess from "node:child_process"
import * as crypto from "node:crypto"
import * as nodeModule from "node:module"
import * as NodeRuntime from "@effect/platform-node/NodeRuntime"
import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Option from "effect/Option"
import * as Path from "effect/Path"
import * as Command from "effect/unstable/cli/Command"
import { configCommand } from "./config.js"
import { setupCommand } from "./setup/index.js"
import { nativePreviewBackend, typescriptBackend, type NativeBackend } from "./setup/consts.js"
import * as pkgJson from "../package.json" with { type: "json" }

class NativeBackendNotInstalledError extends Data.TaggedError("NativeBackendNotInstalledError")<{
  readonly details: string
}> {
  get message(): string {
    return (
      "No native TypeScript backend is installed. " +
      "Install one of the following first: `@typescript/native-preview` or `typescript` (>= 7, e.g. `typescript@latest` or `typescript@next`)."
    )
  }
}

class UnsupportedPlatformPackageError extends Data.TaggedError("UnsupportedPlatformPackageError")<{
  readonly packageName: string
}> {
  get message(): string {
    return (
      `Unable to resolve ${this.packageName}. ` +
      "Your platform may not be supported by the installed native TypeScript backend."
    )
  }
}

class MissingTargetBinaryError extends Data.TaggedError("MissingTargetBinaryError")<{
  readonly targetPath: string
}> {
  get message(): string {
    return (
      "Native TypeScript binary not found at " +
      this.targetPath +
      ". Is the native TypeScript backend installed correctly?"
    )
  }
}

class ResolvePackagedBinaryError extends Data.TaggedError("ResolvePackagedBinaryError")<{
  readonly reason: string
}> {
  get message(): string {
    return this.reason
  }
}

class BackupRestoreError extends Data.TaggedError("BackupRestoreError")<{
  readonly reason: string
}> {
  get message(): string {
    return this.reason
  }
}

class CopyBinaryError extends Data.TaggedError("CopyBinaryError")<{
  readonly sourcePath: string
  readonly targetPath: string
}> {
  get message(): string {
    return `Failed to copy binary from ${this.sourcePath} to ${this.targetPath}.`
  }
}

class ChmodBinaryError extends Data.TaggedError("ChmodBinaryError")<{
  readonly targetPath: string
}> {
  get message(): string {
    return `Failed to set executable permissions on ${this.targetPath}.`
  }
}

class VerificationFailedError extends Data.TaggedError("VerificationFailedError")<{
  readonly targetPath: string
}> {
  get message(): string {
    return (
      "Warning: verification failed for " +
      this.targetPath +
      ", but binary was patched. The binary may still work correctly."
    )
  }
}

type CliDomainError =
  | NativeBackendNotInstalledError
  | UnsupportedPlatformPackageError
  | MissingTargetBinaryError
  | ResolvePackagedBinaryError
  | BackupRestoreError
  | CopyBinaryError
  | ChmodBinaryError
  | VerificationFailedError


/**
 * Outcome of probing a single native backend.
 * - `resolved`: the platform binary path was found.
 * - `notInstalled`: the main package is absent or fails the version check; the
 *   caller should try the next backend.
 * - `unsupportedPlatform`: the main package is installed but its platform
 *   sub-package is missing; a concrete, actionable error.
 */
type BackendProbe =
  | { readonly _tag: "resolved"; readonly path: string; readonly binaryName: string }
  | { readonly _tag: "notInstalled" }
  | { readonly _tag: "unsupportedPlatform"; readonly packageName: string }

const probeBackend = (backend: NativeBackend, cwdRequire: NodeRequire, path: Path.Path): BackendProbe => {
  const isWin = process.platform === "win32"

  let mainPkg: { version?: string }
  try {
    mainPkg = cwdRequire(backend.packageName + "/package.json")
  } catch {
    return { _tag: "notInstalled" }
  }

  if (backend.versionCheck !== undefined && !backend.versionCheck(mainPkg)) {
    return { _tag: "notInstalled" }
  }

  let mainPackageJsonPath: string
  try {
    mainPackageJsonPath = cwdRequire.resolve(backend.packageName + "/package.json")
  } catch {
    return { _tag: "notInstalled" }
  }

  const backendRequire = nodeModule.createRequire(mainPackageJsonPath)
  const platformPackageName = backend.platformPackagePrefix + "-" + process.platform + "-" + process.arch
  let platformPackageJsonPath: string
  try {
    platformPackageJsonPath = backendRequire.resolve(platformPackageName + "/package.json")
  } catch {
    return { _tag: "unsupportedPlatform", packageName: platformPackageName }
  }

  const platformDir = path.dirname(platformPackageJsonPath)
  const binaryName = backend.binaryName + (isWin ? ".exe" : "")
  return { _tag: "resolved", path: path.join(platformDir, "lib", binaryName), binaryName: backend.binaryName }
}

/**
 * Resolve the native TypeScript binary to patch. The `@typescript/native-preview`
 * backend is tried first (back-compat), then the `typescript` >= 7 package so
 * that the TypeScript 7 RC/stable release is supported out of the box.
 *
 * Returns the target binary path plus the backend's base binary name (`tsgo` or
 * `tsc`); the latter selects the matching Effect-patched artifact from
 * `@effect/tsgo-*` (which ships separate `lib/tsgo` and `lib/tsc` builds).
 */
const getNativeBackendBinaryPath = Effect.gen(function*() {
  const path = yield* Path.Path
  const cwdRequire = nodeModule.createRequire(path.join(process.cwd(), "noop.js"))

  const nativePreviewResult = probeBackend(nativePreviewBackend, cwdRequire, path)
  if (nativePreviewResult._tag === "resolved") {
    return { targetPath: nativePreviewResult.path, binaryName: nativePreviewResult.binaryName }
  }
  if (nativePreviewResult._tag === "unsupportedPlatform") {
    return yield* Effect.fail(new UnsupportedPlatformPackageError({ packageName: nativePreviewResult.packageName }))
  }

  const typescriptResult = probeBackend(typescriptBackend, cwdRequire, path)
  if (typescriptResult._tag === "resolved") {
    return { targetPath: typescriptResult.path, binaryName: typescriptResult.binaryName }
  }
  if (typescriptResult._tag === "unsupportedPlatform") {
    return yield* Effect.fail(new UnsupportedPlatformPackageError({ packageName: typescriptResult.packageName }))
  }

  return yield* Effect.fail(new NativeBackendNotInstalledError({ details: "no native backend found" }))
})

/**
 * Resolve the Effect-patched binary to copy over the native target. The
 * `@effect/tsgo-*` platform package ships separate `lib/tsgo` (built from
 * `main`) and `lib/tsc` (built from `generated/latest`) artifacts; `binaryName`
 * selects the one matching the detected native backend so the correct build is
 * installed. Defaults to `tsgo` for the `get-exe-path` command.
 */
const getPackagedBinaryPath = (binaryName: string = "tsgo") =>
  Effect.gen(function*() {
    const fs = yield* FileSystem.FileSystem
    const path = yield* Path.Path
    const packageName = "@effect/tsgo-" + process.platform + "-" + process.arch
    const selfRequire = nodeModule.createRequire(import.meta.url)
    const packageJsonPath: string = yield* Effect.try({
      try: () => selfRequire.resolve(packageName + "/package.json"),
      catch: () =>
        new ResolvePackagedBinaryError({
          reason:
            `Unable to resolve ${packageName}. ` +
            "Either your platform is unsupported, or the platform package is not installed.",
        }),
    })

    const packageDir = path.dirname(packageJsonPath)
    const exeName = binaryName + (process.platform === "win32" ? ".exe" : "")
    const exePath = path.join(packageDir, "lib", exeName)
    const exists = yield* fs.exists(exePath)
    if (!exists) {
      return yield* Effect.fail(
        new ResolvePackagedBinaryError({
          reason: "Executable not found: " + exePath,
        })
      )
    }

    return exePath
  })

const patch = Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const { targetPath, binaryName } = yield* getNativeBackendBinaryPath
  const backupPath = path.join(path.dirname(targetPath), path.basename(targetPath) + ".original")
  const ourBinaryPath = yield* getPackagedBinaryPath(binaryName)

  const targetExists = yield* fs.exists(targetPath)
  if (!targetExists) {
    return yield* Effect.fail(new MissingTargetBinaryError({ targetPath }))
  }

  let actualBackupPath = backupPath
  let counter = 1
  while (yield* fs.exists(actualBackupPath)) {
    if (counter > 100) {
      return yield* Effect.fail(new BackupRestoreError({
        reason: `Too many backup files exist (over 100). Please clean up old backups in ${path.dirname(targetPath)}.`,
      }))
    }
    actualBackupPath = backupPath + "." + counter
    counter++
  }

  yield* fs.rename(targetPath, actualBackupPath).pipe(
    Effect.mapError(() =>
      new BackupRestoreError({
        reason: `Failed to back up original binary from ${targetPath} to ${actualBackupPath}.`,
      })
    )
  )
  yield* Console.log("Backed up original binary to " + actualBackupPath)

  yield* fs.copyFile(ourBinaryPath, targetPath).pipe(
    Effect.mapError(() => new CopyBinaryError({ sourcePath: ourBinaryPath, targetPath }))
  )

  yield* fs.chmod(targetPath, 0o755).pipe(
    Effect.mapError(() => new ChmodBinaryError({ targetPath }))
  )

  yield* Console.log("Patched Effect Language Service binary to " + targetPath)

  const verify = Effect.try({
    try: () => {
      childProcess.execFileSync(targetPath, ["--version"], {
        stdio: "pipe",
        timeout: 10000,
      })
    },
    catch: () => new VerificationFailedError({ targetPath }),
  }).pipe(
    Effect.tap(() => Console.log("Verification succeeded.")),
    Effect.catchTag("VerificationFailedError", (error) => Console.warn(error.message))
  )

  yield* verify
})

const unpatch = Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const { targetPath } = yield* getNativeBackendBinaryPath
  const backupPath = path.join(path.dirname(targetPath), path.basename(targetPath) + ".original")

  const backupExists = yield* fs.exists(backupPath)
  if (!backupExists) {
    yield* Console.error("No backup found at " + backupPath + ". Nothing to restore.")
    return
  }

  const targetExists = yield* fs.exists(targetPath)
  if (targetExists) {
    const dir = path.dirname(targetPath)
    const basename = path.basename(targetPath)
    const uid = crypto.randomUUID()
    const renamedPath = path.join(dir, basename + "." + uid + ".patched")
    yield* fs.rename(targetPath, renamedPath).pipe(
      Effect.mapError(() =>
        new BackupRestoreError({
          reason: `Failed to rename patched binary at ${targetPath} to ${renamedPath}.`,
        })
      )
    )
    yield* Console.log("Renamed patched binary to " + renamedPath)
  }

  yield* fs.rename(backupPath, targetPath).pipe(
    Effect.mapError(() =>
      new BackupRestoreError({
        reason: `Failed to restore backup from ${backupPath} to ${targetPath}.`,
      })
    )
  )

  yield* Console.log("Restored original binary at " + targetPath)
})

const patchCommand = Command.make("patch").pipe(
  Command.withDescription("Patch the Effect Language Service binary"),
  Command.withHandler(() => patch)
)

const unpatchCommand = Command.make("unpatch").pipe(
  Command.withDescription("Unpatch and restore the original TypeScript-Go binary"),
  Command.withHandler(() => unpatch)
)

const getExePathCommand = Command.make("get-exe-path").pipe(
  Command.withDescription("Print the Effect Language Service executable path"),
  Command.withHandler(() => getPackagedBinaryPath().pipe(Effect.flatMap((exePath) => Console.log(exePath))))
)

const rootCommand = Command.make("tsgo").pipe(
  Command.withSubcommands([patchCommand, unpatchCommand, getExePathCommand, setupCommand, configCommand])
)


rootCommand.pipe(
  Command.run({ version: pkgJson.version }),
  Effect.provide(NodeServices.layer),
  NodeRuntime.runMain()
)
