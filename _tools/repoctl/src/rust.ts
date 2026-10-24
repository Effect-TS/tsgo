// The Rust port (crates/, a Cargo workspace over the patched tsrs checkout at tsrs/): `repoctl tsrs check|lint|test`.
// `repoctl check|lint|test` run no Rust; only `repoctl build local` builds the tsrs binary when it can.
import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as Exit from "effect/Exit"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { runCommand, runCommandString } from "./process.ts"

export const tsrsSetupCommand = "pnpm exec repoctl submodules setup --component tsrs"

// Only `repoctl submodules setup --component tsrs` checks out tsrs; without it the workspace's path dependencies do
// not resolve. The default setup (`pnpm setup-repo`) removes it.
export const tsrsCheckedOut = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  return yield* fs.exists(path.join(repositoryRoot, "tsrs", "Cargo.toml"))
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

// `repoctl tsrs check`.
export const runRustChecks = Effect.fnUntraced(function*(repositoryRoot: string) {
  yield* requireTsrsCheckout(repositoryRoot)
  yield* Console.log("Checking Rust workspace")
  yield* runCommand("cargo", repositoryRoot, ["check", "--workspace", "--all-targets", "--locked"])
})

// `repoctl tsrs test`: the crate and unit tests, including the baseline runner (ets_effecttest: every case of
// testdata/tests against testdata/baselines/reference; any mismatch fails, as in the Go runner; it prints a summary of
// the baselines), then the tsrs binary.
export const runRustTests = Effect.fnUntraced(function*(repositoryRoot: string) {
  yield* requireTsrsCheckout(repositoryRoot)
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
    "cli",
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
