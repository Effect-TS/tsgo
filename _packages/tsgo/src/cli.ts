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
import * as Schema from "effect/Schema"
import * as Command from "effect/unstable/cli/Command"
import * as Flag from "effect/unstable/cli/Flag"
import { configCommand } from "./config.js"
import { setupCommand } from "./setup/index.js"
import { defaultTypescriptPackageNames, isNativeTypescriptVersion } from "./setup/consts.js"
import * as pkgJson from "../package.json" with { type: "json" }

class NativeBackendNotInstalledError extends Data.TaggedError("NativeBackendNotInstalledError")<{
  readonly packageNames: ReadonlyArray<string>
}> {
  get message(): string {
    return (
      "No native TypeScript backend is installed. " +
      "Install `typescript` >= 7 first (e.g. `typescript@latest` or `typescript@next`) " +
      "or `@typescript/native` (e.g. `@typescript/native@npm:typescript@latest`). " +
      "Tried: " + this.packageNames.join(", ") + "."
    )
  }
}

class PackageNotFoundError extends Data.TaggedError("PackageNotFoundError")<{
  readonly packageName: string
}> {
  get message(): string {
    return `Unable to resolve ${this.packageName}.`
  }
}

class ParsePackageJsonError extends Data.TaggedError("ParsePackageJsonError")<{
  readonly packageName: string
  readonly packageJsonPath: string
  readonly reason: string
}> {
  get message(): string {
    return `Unable to parse ${this.packageName} package.json at ${this.packageJsonPath}: ${this.reason}`
  }
}

class BinaryMetadataNotFoundError extends Data.TaggedError("BinaryMetadataNotFoundError")<{
  readonly metadataPath: string
}> {
  get message(): string {
    return `Binary metadata not found at ${this.metadataPath}.`
  }
}

class ParseBinaryMetadataError extends Data.TaggedError("ParseBinaryMetadataError")<{
  readonly metadataPath: string
  readonly reason: string
}> {
  get message(): string {
    return `Unable to parse binary metadata at ${this.metadataPath}: ${this.reason}`
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

class PackagedBinaryVersionMismatchError extends Data.TaggedError("PackagedBinaryVersionMismatchError")<{
  readonly installedVersion: string
  readonly installedGitHead: string
  readonly candidates: ReadonlyArray<PackagedBinaryCandidateInfo>
}> {
  get message(): string {
    const tried = this.candidates.length === 0
      ? "  none"
      : this.candidates.map((candidate) => {
        if (candidate.tsVersion !== undefined && candidate.tsGitHead !== undefined) {
          return `  ${candidate.binaryName}: TypeScript ${candidate.tsVersion}, gitHead ${candidate.tsGitHead}`
        }
        return `  ${candidate.binaryName}: ${candidate.reason ?? "metadata unavailable"}`
      }).join("\n")

    return [
      `No packaged Effect TypeScript binary matches installed TypeScript ${this.installedVersion} gitHead ${this.installedGitHead}.`,
      "",
      "Tried:",
      tried,
      "",
      "Install a matching @effect/tsgo release or a matching TypeScript version, or rerun with --force to use the newest packaged binary."
    ].join("\n")
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
  | PackageNotFoundError
  | ParsePackageJsonError
  | BinaryMetadataNotFoundError
  | ParseBinaryMetadataError
  | UnsupportedPlatformPackageError
  | MissingTargetBinaryError
  | ResolvePackagedBinaryError
  | PackagedBinaryVersionMismatchError
  | BackupRestoreError
  | CopyBinaryError
  | ChmodBinaryError
  | VerificationFailedError

interface PackagedBinaryMetadata {
  readonly tsVersion: string
  readonly tsGitHead: string
}

interface PackagedBinaryCandidateInfo {
  readonly binaryName: string
  readonly tsVersion?: string
  readonly tsGitHead?: string
  readonly reason?: string
}

interface PackagedBinaryCandidate extends PackagedBinaryCandidateInfo {
  readonly path?: string
  readonly metadata?: PackagedBinaryMetadata
}

interface PackageJsonMetadata {
  readonly name: string
  readonly version: string
  readonly gitHead: string
}

interface ResolvedPackageJson extends PackageJsonMetadata {
  readonly requestedPackageName: string
  readonly packageJsonPath: string
}

interface OfficialTypeScriptBinary {
  readonly packageName: string
  readonly packageJsonPath: string
  readonly packageVersion: string
  readonly packageGitHead: string
  readonly platformPackageName: string
  readonly platformPackageJsonPath: string
  readonly platformGitHead: string
  readonly binaryPath: string
}

const PackageJsonMetadataSchema = Schema.Struct({
  name: Schema.String,
  version: Schema.String,
  gitHead: Schema.String
})
const PackageJsonMetadataFromStringSchema = Schema.fromJsonString(PackageJsonMetadataSchema)

const BinaryMetadataSchema = Schema.Struct({
  tsVersion: Schema.String,
  tsGitHead: Schema.String
})
const BinaryMetadataFromStringSchema = Schema.fromJsonString(BinaryMetadataSchema)

const packagedTypeScriptBinaryNames = ["tsc", "tsc-next"] as const


const decodePackageJsonText = (packageName: string, packageJsonPath: string, text: string) =>
  Schema.decodeUnknownEffect(PackageJsonMetadataFromStringSchema)(text).pipe(
    Effect.mapError((error) => new ParsePackageJsonError({
      packageName,
      packageJsonPath,
      reason: error.message
    }))
  )

const readPackageJsonFile = (packageName: string, packageJsonPath: string) =>
  Effect.gen(function*() {
    const fs = yield* FileSystem.FileSystem
    const text = yield* fs.readFileString(packageJsonPath).pipe(
      Effect.mapError((error) => new ParsePackageJsonError({
        packageName,
        packageJsonPath,
        reason: error.message
      }))
    )
    const metadata = yield* decodePackageJsonText(packageName, packageJsonPath, text)
    return { requestedPackageName: packageName, packageJsonPath, ...metadata }
  })

const resolvePackageJson = (cwd: string, packageName: string) =>
  Effect.gen(function*() {
    const path = yield* Path.Path
    const cwdRequire = nodeModule.createRequire(path.join(cwd, "noop.js"))
    const packageJsonPath = yield* Effect.try({
      try: () => cwdRequire.resolve(packageName + "/package.json"),
      catch: () => new PackageNotFoundError({ packageName })
    })
    return yield* readPackageJsonFile(packageName, packageJsonPath)
  })

const decodeBinaryMetadataText = (metadataPath: string, text: string) =>
  Schema.decodeUnknownEffect(BinaryMetadataFromStringSchema)(text).pipe(
    Effect.mapError((error) => new ParseBinaryMetadataError({
      metadataPath,
      reason: error.message
    }))
  )

const readBinaryMetadata = (binaryPath: string) =>
  Effect.gen(function*() {
    const fs = yield* FileSystem.FileSystem
    const metadataPath = binaryPath + ".json"
    const exists = yield* fs.exists(metadataPath)
    if (!exists) {
      return yield* Effect.fail(new BinaryMetadataNotFoundError({ metadataPath }))
    }

    const text = yield* fs.readFileString(metadataPath).pipe(
      Effect.mapError((error) => new ParseBinaryMetadataError({
        metadataPath,
        reason: error.message
      }))
    )
    return yield* decodeBinaryMetadataText(metadataPath, text)
  })

const resolveOfficialTypeScriptBinary = (typescriptPackage: ResolvedPackageJson) =>
  Effect.gen(function*() {
    const path = yield* Path.Path
    const platformPackageName = "@typescript/typescript-" + process.platform + "-" + process.arch
    const packageRequire = nodeModule.createRequire(typescriptPackage.packageJsonPath)
    const platformPackageJsonPath = yield* Effect.try({
      try: () => packageRequire.resolve(platformPackageName + "/package.json"),
      catch: () => new UnsupportedPlatformPackageError({ packageName: platformPackageName })
    })
    const platformPackage = yield* readPackageJsonFile(platformPackageName, platformPackageJsonPath)
    const binaryName = "tsc" + (process.platform === "win32" ? ".exe" : "")

    return {
      packageName: typescriptPackage.requestedPackageName,
      packageJsonPath: typescriptPackage.packageJsonPath,
      packageVersion: typescriptPackage.version,
      packageGitHead: typescriptPackage.gitHead,
      platformPackageName,
      platformPackageJsonPath,
      platformGitHead: platformPackage.gitHead,
      binaryPath: path.join(path.dirname(platformPackageJsonPath), "lib", binaryName)
    } satisfies OfficialTypeScriptBinary
  })

const resolveInstalledTypeScriptBinary = (packageNames: ReadonlyArray<string>) =>
  Effect.gen(function*() {
    for (const packageName of packageNames) {
      const packageResult = yield* resolvePackageJson(process.cwd(), packageName).pipe(
        Effect.catchTag("PackageNotFoundError", () => Effect.succeed(undefined))
      )
      if (packageResult === undefined || !isNativeTypescriptVersion(packageResult.version)) {
        continue
      }
      return yield* resolveOfficialTypeScriptBinary(packageResult)
    }

    return yield* Effect.fail(new NativeBackendNotInstalledError({ packageNames }))
  })

const packageNamesWithPreferred = (preferredPackageName: Option.Option<string>): ReadonlyArray<string> => {
  const preferred = Option.getOrUndefined(preferredPackageName)
  return preferred === undefined || preferred === ""
    ? defaultTypescriptPackageNames
    : Array.from(new Set([preferred, ...defaultTypescriptPackageNames]))
}

/**
 * Resolve the Effect-patched binary to copy over the native target. The
 * `@effect/tsgo-*` platform package ships `lib/tsc` (built from
 * `generated/latest`) and `lib/tsc-next` (built from `main`). The adjacent JSON
 * metadata files identify the TypeScript gitHead each binary was built from.
 */
const getPackagedBinaryPath = (installedTypeScript: OfficialTypeScriptBinary, force: boolean) =>
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
    const candidates: Array<PackagedBinaryCandidate> = []

    for (const binaryName of packagedTypeScriptBinaryNames) {
      const exeName = binaryName + (process.platform === "win32" ? ".exe" : "")
      const exePath = path.join(packageDir, "lib", exeName)
      const exists = yield* fs.exists(exePath)
      if (!exists) {
        candidates.push({ binaryName, reason: "binary not packaged" })
        continue
      }

      const metadataResult = yield* readBinaryMetadata(exePath).pipe(
        Effect.match({
          onFailure: (error) => ({ _tag: "failure" as const, error }),
          onSuccess: (metadata) => ({ _tag: "success" as const, metadata })
        })
      )
      if (metadataResult._tag === "failure") {
        candidates.push({ binaryName, path: exePath, reason: metadataResult.error.message })
        continue
      }

      const metadata = metadataResult.metadata

      const candidate = { binaryName, path: exePath, metadata, ...metadata }
      if (metadata.tsGitHead === installedTypeScript.packageGitHead) {
        return exePath
      }
      candidates.push(candidate)
    }

    if (force) {
      const fallback = candidates.find((candidate) => candidate.binaryName === "tsc-next" && candidate.path !== undefined)
        ?? candidates.find((candidate) => candidate.binaryName === "tsc" && candidate.path !== undefined)
      if (fallback?.path !== undefined) {
        yield* Console.warn(new PackagedBinaryVersionMismatchError({
          installedVersion: installedTypeScript.packageVersion,
          installedGitHead: installedTypeScript.packageGitHead,
          candidates
        }).message)
        yield* Console.warn("Forcing patch with " + fallback.binaryName + ". This may be incompatible.")
        return fallback.path
      }
    }

    if (candidates.length === 0) {
      return yield* Effect.fail(
        new ResolvePackagedBinaryError({
          reason: "No packaged TypeScript binaries were found in " + path.join(packageDir, "lib"),
        })
      )
    }

    return yield* Effect.fail(new PackagedBinaryVersionMismatchError({
      installedVersion: installedTypeScript.packageVersion,
      installedGitHead: installedTypeScript.packageGitHead,
      candidates
    }))
  })

const patch = (force: boolean, packageNames: ReadonlyArray<string>) => Effect.gen(function*() {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const installedTypeScript = yield* resolveInstalledTypeScriptBinary(packageNames)
  const targetPath = installedTypeScript.binaryPath
  const backupPath = path.join(path.dirname(targetPath), path.basename(targetPath) + ".original")
  const ourBinaryPath = yield* getPackagedBinaryPath(installedTypeScript, force)

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
  const { binaryPath: targetPath } = yield* resolveInstalledTypeScriptBinary(defaultTypescriptPackageNames)
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

const patchCommand = Command.make("patch", {
  force: Flag.boolean("force"),
  typescriptPackage: Flag.optional(
    Flag.string("typescript-package").pipe(
      Flag.withDescription("Native TypeScript package name to try before the default package names")
    )
  )
}).pipe(
  Command.withDescription("Patch the Effect Language Service binary"),
  Command.withHandler(({ force, typescriptPackage }) => patch(force, packageNamesWithPreferred(typescriptPackage)))
)

const unpatchCommand = Command.make("unpatch").pipe(
  Command.withDescription("Unpatch and restore the original TypeScript-Go binary"),
  Command.withHandler(() => unpatch)
)

const getExePathCommand = Command.make("get-exe-path").pipe(
  Command.withDescription("Print the Effect Language Service executable path"),
  Command.withHandler(() =>
    resolveInstalledTypeScriptBinary(defaultTypescriptPackageNames).pipe(
      Effect.flatMap((installedTypeScript) => getPackagedBinaryPath(installedTypeScript, false)),
      Effect.flatMap((exePath) => Console.log(exePath))
    )
  )
)

const rootCommand = Command.make("tsgo").pipe(
  Command.withSubcommands([patchCommand, unpatchCommand, getExePathCommand, setupCommand, configCommand])
)


rootCommand.pipe(
  Command.run({ version: pkgJson.version }),
  Effect.provide(NodeServices.layer),
  NodeRuntime.runMain()
)
