// The Rust port (crates/, a Cargo workspace over the patched tsrs checkout at tsrs/): `repoctl tsrs check|lint|test`.
// `repoctl check|lint|test` run no Rust; only `repoctl build local` builds the tsrs binary when it can.
import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as Exit from "effect/Exit"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { runCommand, runCommandString } from "./process.ts"
import { checkTsrsReleaseParity } from "./tsrsRelease.ts"

export const tsrsSetupCommand = "pnpm exec repoctl submodules setup --component tsrs"

// Only `repoctl submodules setup --component tsrs` checks out tsrs; without it the workspace's path dependencies do
// not resolve. The default setup (`pnpm setup-repo`) removes it.
export const tsrsCheckedOut = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  return yield* fs.exists(path.join(repositoryRoot, "tsrs", "Cargo.toml"))
})

// The `[package]` name and version of a Cargo.toml (`version.workspace = true` takes the workspace's), or undefined.
export const parseCargoPackage = (cargoToml: string, workspaceVersion: string | undefined) => {
  const section = (name: string) => {
    const lines = cargoToml.split(/\r?\n/)
    const start = lines.findIndex((line) => line.trim() === `[${name}]`)
    if (start < 0) return new Map<string, string>()
    const fields = new Map<string, string>()
    for (const line of lines.slice(start + 1)) {
      if (line.trim().startsWith("[")) break
      const field = /^\s*([A-Za-z_.-]+)\s*=\s*(.+?)\s*$/.exec(line)
      if (field !== null) fields.set(field[1]!, field[2]!)
    }
    return fields
  }
  const fields = section("package")
  const name = /^"([^"]+)"/.exec(fields.get("name") ?? "")?.[1]
  const version = fields.get("version.workspace") === "true"
    ? workspaceVersion
    : /^"([^"]+)"/.exec(fields.get("version") ?? "")?.[1]
  return name === undefined || version === undefined ? undefined : { name, version }
}

// The workspace package version of a Cargo.toml (`[workspace.package]` version), or undefined.
export const parseCargoWorkspaceVersion = (cargoToml: string) =>
  parseCargoPackage(cargoToml.replace("[workspace.package]", "[package]\nname = \"workspace\""), undefined)?.version

// Cargo.lock with the version of each path package (no `source`) named in `versions` set to it.
export const syncCargoLockVersions = (cargoLock: string, versions: ReadonlyMap<string, string>) =>
  cargoLock.split("\n[[package]]\n").map((block) => {
    const name = /^name = "([^"]+)"$/m.exec(block)?.[1]
    const version = name === undefined ? undefined : versions.get(name)
    return version === undefined || /^source = /m.test(block)
      ? block
      : block.replace(/^version = "[^"]*"$/m, `version = "${version}"`)
  }).join("\n[[package]]\n")

// Cargo.lock records the version of every tsrs crate (path dependencies into tsrs/crates), which differs between the
// recorded tsrs releases (0.25.0, 7.1.0-beta.tsrs2, ...). The tsrs setup brings them to the checked-out tsrs, so that
// `cargo --locked` accepts the lock with any recorded tsrs; for the tsrs the Rust port follows this is the committed
// lock (commit no other).
export const syncTsrsCargoLock = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const tsrs = path.join(repositoryRoot, "tsrs")
  const workspaceVersion = parseCargoWorkspaceVersion(yield* fs.readFileString(path.join(tsrs, "Cargo.toml")))
  const versions = new Map<string, string>()
  for (const crate of (yield* fs.readDirectory(path.join(tsrs, "crates"))).sort()) {
    const manifest = path.join(tsrs, "crates", crate, "Cargo.toml")
    if (!(yield* fs.exists(manifest))) continue
    const cratePackage = parseCargoPackage(yield* fs.readFileString(manifest), workspaceVersion)
    if (cratePackage !== undefined) versions.set(cratePackage.name, cratePackage.version)
  }
  const lockPath = path.join(repositoryRoot, "Cargo.lock")
  const lock = yield* fs.readFileString(lockPath)
  const synced = syncCargoLockVersions(lock, versions)
  if (synced !== lock) {
    yield* Console.log("Updated the tsrs crate versions in Cargo.lock to the tsrs checkout")
    yield* fs.writeFileString(lockPath, synced)
  }
})

export class TsrsCheckoutMissingError extends Data.TaggedError("TsrsCheckoutMissingError")<{}> {
  get message(): string {
    return `tsrs/ is not checked out: run \`${tsrsSetupCommand}\``
  }
}

// The cargo steps fail without the tsrs checkout.
export const requireTsrsCheckout = Effect.fnUntraced(function*(repositoryRoot: string) {
  if (!(yield* tsrsCheckedOut(repositoryRoot))) {
    return yield* new TsrsCheckoutMissingError()
  }
})

// Generates crates/ets_shim from _tools/gen_shims/rustshim/config.json (Go only; the output is committed). Part of
// every setup, it runs only with the tsrs checkout and otherwise leaves crates/ets_shim as committed.
export const generateRustShims = Effect.fnUntraced(function*(repositoryRoot: string) {
  const path = yield* Path.Path
  if (!(yield* tsrsCheckedOut(repositoryRoot))) {
    yield* Console.log("Not generating crates/ets_shim: tsrs/ is not checked out")
    return
  }
  yield* Console.log("Generating Rust shims")
  yield* runCommand("go", path.join(repositoryRoot, "_tools", "gen_shims"), [
    "run",
    "./rustshim",
    "--repository-root",
    repositoryRoot
  ], false, { GOWORK: "off" })
})

// Clippy with warnings denied, over the workspace and its tests: rustc warnings (unused imports, needless borrows)
// fail here, before a test run that would otherwise pass with them.
const runClippy = Effect.fnUntraced(function*(repositoryRoot: string) {
  yield* Console.log("Running Clippy")
  yield* runCommand("cargo", repositoryRoot, [
    "clippy",
    "--workspace",
    "--all-targets",
    "--locked",
    "--",
    "-D",
    "warnings"
  ])
})

// `repoctl tsrs check`: the release build still mirrors tsrs's release workflow (tsrsRelease.ts), then cargo.
export const runRustChecks = Effect.fnUntraced(function*(repositoryRoot: string) {
  yield* requireTsrsCheckout(repositoryRoot)
  yield* checkTsrsReleaseParity(repositoryRoot)
  yield* Console.log("Checking Rust workspace")
  yield* runCommand("cargo", repositoryRoot, ["check", "--workspace", "--all-targets", "--locked"])
})

// `repoctl tsrs test`: Clippy (warnings denied), then the crate and unit tests, including the baseline runner
// (ets_effecttest: every case of testdata/tests against testdata/baselines/reference; any mismatch fails, as in the Go
// runner; it prints a summary of the baselines), then the tsrs binary.
export const runRustTests = Effect.fnUntraced(function*(repositoryRoot: string) {
  yield* requireTsrsCheckout(repositoryRoot)
  yield* runClippy(repositoryRoot)
  yield* Console.log("Running Rust tests")
  yield* runCommand("cargo", repositoryRoot, ["test", "--workspace", "--locked"])
  yield* runRustBinarySmokeTest(repositoryRoot)
})

// Why the Rust executable cannot be built here, or undefined when it can: it needs cargo, and tsrs's CLI links
// mimalloc, whose build needs CMake.
export const missingRustBinaryTools = Effect.fnUntraced(function*(repositoryRoot: string) {
  const cargo = yield* Effect.exit(runCommandString("cargo", repositoryRoot, ["--version"]))
  if (Exit.isFailure(cargo)) {
    return "cargo is not installed"
  }
  const cmake = yield* Effect.exit(runCommandString("cmake", repositoryRoot, ["--version"]))
  if (Exit.isFailure(cmake)) {
    return "CMake is not installed (tsrs's CLI links mimalloc, built with CMake)"
  }
  return undefined
})

export interface RustBinaryBuildOptions {
  // The `release` profile (Cargo.toml: tsrs's, fat LTO, one codegen unit, stripped) instead of `dev`.
  readonly release: boolean
  // An explicit Rust target triple; the binary then lands in target/<triple>/<profile>/.
  readonly rustTarget?: string
}

// Builds the Rust executable (crates/ets_tsrs, binary `tsrs`) with tsrs's CLI and returns its path.
export const buildRustBinary = Effect.fnUntraced(function*(repositoryRoot: string, options: RustBinaryBuildOptions) {
  const path = yield* Path.Path
  yield* Console.log(`Building the tsrs binary (${options.release ? "release" : "dev"} profile)`)
  yield* runCommand("cargo", repositoryRoot, [
    "build",
    "-p",
    "ets_tsrs",
    "--features",
    options.release ? "cli,tsrs_cli/noembed" : "cli",
    "--locked",
    ...(options.release ? ["--release"] : []),
    ...(options.rustTarget === undefined ? [] : ["--target", options.rustTarget])
  ])
  const targetDirectory = process.env.CARGO_TARGET_DIR ?? path.join(repositoryRoot, "target")
  return path.join(
    targetDirectory,
    ...(options.rustTarget === undefined ? [] : [options.rustTarget]),
    options.release ? "release" : "debug",
    process.platform === "win32" ? "tsrs.exe" : "tsrs"
  )
})

// Builds the Rust executable and runs `--version`. Without cargo or CMake the step is skipped with a message (the
// Validate tsrs workflow installs CMake).
export const runRustBinarySmokeTest = Effect.fnUntraced(function*(repositoryRoot: string) {
  const missing = yield* missingRustBinaryTools(repositoryRoot)
  if (missing !== undefined) {
    yield* Console.log(`Skipping the tsrs binary: ${missing}`)
    return
  }
  const binary = yield* buildRustBinary(repositoryRoot, { release: false })
  yield* runCommand(binary, repositoryRoot, ["--version"])
})

// `repoctl tsrs lint`.
export const runRustLint = Effect.fnUntraced(function*(repositoryRoot: string) {
  yield* requireTsrsCheckout(repositoryRoot)
  yield* Console.log("Checking Rust formatting")
  yield* runCommand("cargo", repositoryRoot, ["fmt", "--check"])
  yield* runClippy(repositoryRoot)
})
