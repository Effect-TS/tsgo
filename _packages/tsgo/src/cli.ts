import * as childProcess from "node:child_process"
import * as crypto from "node:crypto"
import * as nodeModule from "node:module"
import * as NodeRuntime from "@effect/platform-node/NodeRuntime"
import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import * as Command from "effect/unstable/cli/Command"
import { setupCommand } from "./setup/index.js"

class NativePreviewNotInstalledError extends Data.TaggedError("NativePreviewNotInstalledError")<{
  readonly details: string
}> {
  get message(): string {
    return (
      "@typescript/native-preview is not installed. " +
      "Please install it first: npm install @typescript/native-preview"
    )
  }
}

class UnsupportedPlatformPackageError extends Data.TaggedError("UnsupportedPlatformPackageError")<{
  readonly packageName: string
}> {
  get message(): string {
    return (
      `Unable to resolve ${this.packageName}. ` +
      "Your platform may not be supported by @typescript/native-preview."
    )
  }
}

class MissingTargetBinaryError extends Data.TaggedError("MissingTargetBinaryError")<{
  readonly targetPath: string
}> {
  get message(): string {
    return (
      "TypeScript-Go binary not found at " +
      this.targetPath +
      ". Is @typescript/native-preview installed correctly?"
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
  | NativePreviewNotInstalledError
  | UnsupportedPlatformPackageError
  | MissingTargetBinaryError
  | ResolvePackagedBinaryError
  | BackupRestoreError
  | CopyBinaryError
  | ChmodBinaryError
  | VerificationFailedError


const getNativePreviewBinaryPath = Effect.gen(function*() {
  const path = yield* Path.Path
  const cwdRequire = nodeModule.createRequire(path.join(process.cwd(), "noop.js"))

  const nativePreviewPackageJsonPath: string = yield* Effect.try({
    try: () => cwdRequire.resolve("@typescript/native-preview/package.json"),
    catch: () => new NativePreviewNotInstalledError({ details: "missing package" }),
  })

  const nativePreviewRequire = nodeModule.createRequire(nativePreviewPackageJsonPath)
  const expectedPackage = "native-preview-" + process.platform + "-" + process.arch
  const platformPackageName = "@typescript/" + expectedPackage
  const platformPackageJsonPath: string = yield* Effect.try({
    try: () => nativePreviewRequire.resolve(platformPackageName + "/package.json"),
    catch: () => new UnsupportedPlatformPackageError({ packageName: platformPackageName }),
  })

  const platformDir = path.dirname(platformPackageJsonPath)
  const binaryName = process.platform === "win32" ? "tsgo.exe" : "tsgo"
  return path.join(platformDir, "lib", binaryName)
})

const getPackagedBinaryPath = Effect.gen(function*() {
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
  const binaryName = process.platform === "win32" ? "tsgo.exe" : "tsgo"
  const exePath = path.join(packageDir, "lib", binaryName)
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
  const targetPath = yield* getNativePreviewBinaryPath
  const backupPath = path.join(path.dirname(targetPath), path.basename(targetPath) + ".original")
  const ourBinaryPath = yield* getPackagedBinaryPath

  const backupExists = yield* fs.exists(backupPath)
  if (backupExists) {
    yield* Console.log("Binary is already patched (backup exists at " + backupPath + "). Skipping.")
    return
  } else {
    const targetExists = yield* fs.exists(targetPath)
    if (!targetExists) {
      return yield* Effect.fail(new MissingTargetBinaryError({ targetPath }))
    }

    yield* fs.rename(targetPath, backupPath).pipe(
      Effect.mapError(() =>
        new BackupRestoreError({
          reason: `Failed to back up original binary from ${targetPath} to ${backupPath}.`,
        })
      )
    )
    yield* Console.log("Backed up original binary to " + backupPath)
  }

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
  const targetPath = yield* getNativePreviewBinaryPath
  const backupPath = path.join(path.dirname(targetPath), path.basename(targetPath) + ".original")

  const backupExists = yield* fs.exists(backupPath)
  if (!backupExists) {
    yield* Console.log("No backup found at " + backupPath + ". Nothing to restore.")
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
  Command.withHandler(() => getPackagedBinaryPath.pipe(Effect.flatMap((exePath) => Console.log(exePath))))
)

const rootCommand = Command.make("tsgo").pipe(
  Command.withSubcommands([patchCommand, unpatchCommand, getExePathCommand, setupCommand])
)


rootCommand.pipe(
  Command.run({ version: "0.0.0" }),
  Effect.provide(NodeServices.layer),
  NodeRuntime.runMain()
)
