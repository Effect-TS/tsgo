// The release build of our tsrs binary (crates/ets_tsrs) with tsrs's own release pipeline: the `build` and
// `build-windows` jobs of tsrs/.github/workflows/release.yml at the pinned tsrs commit. Every target is a PGO build,
// Linux targets are BOLT-optimized on top (notes/perf-pgo.md, notes/perf-build-level.md in the tsrs checkout), with
// the same RUSTFLAGS, profile settings, C compiler and strip behaviour, and tsrs's training scripts run unchanged
// (bolt.sh with _patches/tsrs/160-bolt-extra-workload.patch).
//
// Two differences, both needed to train our binary rather than tsrs's:
// - tsrs-test and fourslash_server (the in-process conformance and fourslash training) are built from this workspace,
//   in the same cargo invocation as ets_tsrs (crates/ets_pgotrain): profile entries are keyed by symbol names, which
//   carry each crate's `-C metadata`, which depends on the workspace that builds it.
// - The training adds perf CI's Effect workload (perfCi.ts, every rule on), so that PGO and BOLT do not lay out the
//   Effect rules as cold code.
//
// `checkTsrsReleaseParity` (`repoctl tsrs check`) fails when tsrs's release workflow no longer has the settings
// mirrored here.
import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { createHash } from "node:crypto"
import type { BuildTarget } from "./build.ts"
import { prepareWorkload } from "./perfCi.ts"
import { runCommand, runCommandExitCode, runCommandString } from "./process.ts"

// How a target's release binary is optimized: `none` is the plain release profile (fat LTO, Cargo.toml).
export const tsrsOptimizations = ["none", "pgo", "pgo-bolt"] as const
export type TsrsOptimization = typeof tsrsOptimizations[number]

// The patched tsrs binary (crates/ets_tsrs, binary `tsrs`), released per tsrs version (upstream.json components.tsrs)
// for the targets tsrs publishes a glibc, macOS or Windows binary for, each built natively on the runner tsrs's
// release uses and optimized as there (`build`: PGO, plus BOLT on Linux; `build-windows`: PGO). ubuntu-22.04 keeps the
// glibc requirement at 2.35. `effect-tsgo patch --tsrs` swaps it in for the binary of the user's installed
// `@tsrs/<target>` package. tsrs's musl targets (`build-musl`: static, no PGO) are not built, see
// `tsrsSkippedUpstreamTargets`.
export const tsrsBuildTargets = {
  "darwin-arm64": { rustTarget: "aarch64-apple-darwin", runner: "macos-latest", optimization: "pgo" },
  "darwin-x64": { rustTarget: "x86_64-apple-darwin", runner: "macos-26-intel", optimization: "pgo" },
  "linux-x64": { rustTarget: "x86_64-unknown-linux-gnu", runner: "ubuntu-22.04", optimization: "pgo-bolt" },
  "linux-arm64": { rustTarget: "aarch64-unknown-linux-gnu", runner: "ubuntu-22.04-arm", optimization: "pgo-bolt" },
  "win32-x64": { rustTarget: "x86_64-pc-windows-msvc", runner: "windows-latest", optimization: "pgo" }
} as const satisfies Partial<
  Record<BuildTarget, { readonly rustTarget: string; readonly runner: string; readonly optimization: TsrsOptimization }>
>

export type TsrsBuildTarget = keyof typeof tsrsBuildTargets

// tsrs's release targets without a counterpart here: @effect/tsgo has no musl platform packages, and the CLI reports
// musl hosts as unsupported.
export const tsrsSkippedUpstreamTargets = ["x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl"] as const

export class TsrsReleaseError extends Data.TaggedError("TsrsReleaseError")<{
  readonly reason: string
}> {
  get message(): string {
    return `tsrs release build failed: ${this.reason}`
  }
}

export const tsrsBinaryFileName = (target: TsrsBuildTarget) => target.startsWith("win32-") ? "tsrs.exe" : "tsrs"

// The build's working state, outside the final build's target directory (as tsrs's `$RUNNER_TEMP/pgo`).
export const tsrsReleaseLayout = (workDirectory: string, separator = "/") => {
  const join = (...parts: ReadonlyArray<string>) => [workDirectory, ...parts].join(separator)
  return {
    work: workDirectory,
    instrumentedTarget: join("pgo", "target"),
    rawProfiles: join("pgo", "raw"),
    bench: join("pgo", "bench"),
    effectWorkload: join("pgo", "effect"),
    pnpm: join("pnpm"),
    bolt: join("bolt"),
    llvm: join("llvm"),
    determinismTarget: join("determinism-target")
  }
}
export type TsrsReleaseLayout = ReturnType<typeof tsrsReleaseLayout>

const usesBolt = (optimization: TsrsOptimization) => optimization === "pgo-bolt"

// RUSTFLAGS as cargo's CARGO_ENCODED_RUSTFLAGS (tsrs's Windows job; paths may contain spaces).
const encodedRustflags = (flags: ReadonlyArray<string>) => flags.join("\x1f")

// The environment of both cargo builds. Cargo hashes the profile (strip included) into every crate's `-C metadata`,
// so both builds must agree on it; BOLT needs the symbols until bolt.sh strips its output. RUSTFLAGS are not part of
// the metadata: `-Clto=thin` overrides the profile's fat LTO for the instrumented build only. cc passes
// `-Cprofile-*` on to clang (macOS) but not to gcc (Linux) or MSVC; tsrs keeps the C code (mimalloc) uninstrumented
// on macOS as well, with CFLAGS.
export const tsrsCargoEnvironment = (options: {
  readonly target: TsrsBuildTarget
  readonly optimization: Exclude<TsrsOptimization, "none">
  readonly phase: { readonly kind: "generate"; readonly rawProfiles: string; readonly targetDirectory: string } | {
    readonly kind: "use"
    readonly profile: string
  }
}): Record<string, string> => {
  const macOS = options.target.startsWith("darwin-")
  const bolt = usesBolt(options.optimization)
  const environment: Record<string, string> = {
    CARGO_PROFILE_RELEASE_STRIP: bolt ? "none" : "symbols"
  }
  if (options.phase.kind === "generate") {
    environment.CARGO_ENCODED_RUSTFLAGS = encodedRustflags([
      `-Cprofile-generate=${options.phase.rawProfiles}`,
      "-Clto=thin"
    ])
    environment.CARGO_TARGET_DIR = options.phase.targetDirectory
    if (macOS) environment.CFLAGS = "-fno-profile-generate"
  } else {
    // --emit-relocs keeps the relocations BOLT needs to move code (the code itself is unchanged).
    environment.CARGO_ENCODED_RUSTFLAGS = encodedRustflags([
      `-Cprofile-use=${options.phase.profile}`,
      ...(bolt ? ["-Clink-arg=-Wl,--emit-relocs"] : [])
    ])
    if (macOS) environment.CFLAGS = "-fno-profile-use"
  }
  return environment
}

// `cargo build` of the tsrs binary alone (as `buildRustBinary`'s release build), or together with the training
// binaries: the instrumented build, and on BOLT targets the final build too (bolt.sh's gates run the BOLT-optimized
// tsrs-test and fourslash_server). The Windows training has no fourslash run (tsrs's `build-windows`).
export const tsrsCargoBuildArgs = (target: TsrsBuildTarget, withTrainingBinaries: boolean) => [
  "build",
  "--release",
  "--locked",
  "-p",
  "ets_tsrs",
  ...(withTrainingBinaries
    ? [
      "-p",
      "ets_pgotrain",
      "-p",
      "tsrs_testrunner",
      ...(target.startsWith("win32-") ? [] : ["-p", "tsrs_fourslash_server"]),
      "--bins",
      "--features",
      "ets_tsrs/cli,tsrs_cli/noembed,ets_pgotrain/train"
    ]
    : ["--features", "cli,tsrs_cli/noembed"]),
  "--target",
  tsrsBuildTargets[target].rustTarget
]

// The LLVM release tarball with BOLT for rustc's LLVM (tsrs's "Install BOLT" step).
export const boltToolsUrl = (llvmVersion: string, arch: string) => {
  const name = `LLVM-${llvmVersion}-Linux-${arch === "arm64" ? "ARM64" : "X64"}`
  return {
    name,
    url: `https://github.com/llvm/llvm-project/releases/download/llvmorg-${llvmVersion}/${name}.tar.zst`,
    files: [`${name}/bin/llvm-bolt`, `${name}/bin/merge-fdata`, `${name}/lib/libbolt_rt_instr.a`]
  }
}

export const parseLlvmVersion = (rustcVersion: string) => /^LLVM version: (\S+)$/m.exec(rustcVersion)?.[1]

// The TypeScript commit tsrs ports (Cargo.toml `[workspace.metadata.typescript]`).
export const parseTsrsTypeScriptCommit = (cargoToml: string) =>
  /^\[workspace\.metadata\.typescript\]\n(?:[^[\n].*\n)*?commit = "([0-9a-f]{40})"/m.exec(cargoToml)?.[1]

const shellQuote = (value: string) => `'${value.replaceAll("'", `'\\''`)}'`

// BOLT_EXTRA_WORKLOAD for bolt.sh: `<script> <tsrs> <output file> [tsrs arguments...]` builds the Effect workload
// from clean and exits with tsrs's status.
export const effectWorkloadScript = (workloadDirectory: string) =>
  [
    "#!/usr/bin/env bash",
    "# Generated by repoctl (tsrsRelease.ts): BOLT_EXTRA_WORKLOAD for tsrs/.github/scripts/bolt.sh.",
    "set -uo pipefail",
    "exe=$1 out=$2",
    "shift 2",
    `cd ${shellQuote(workloadDirectory)} || exit 3`,
    "pnpm clean > /dev/null || exit 3",
    `"$exe" -b tsconfig.json --pretty false "$@" > "$out"`,
    ""
  ].join("\n")

const nonEmptyFiles = Effect.fnUntraced(function*(directory: string, prefix: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const files: Array<string> = []
  for (const name of (yield* fs.readDirectory(directory)).sort()) {
    if (!name.startsWith(prefix) || !name.endsWith(".profraw")) continue
    const file = path.join(directory, name)
    if ((yield* fs.stat(file)).size > 0n) files.push(file)
  }
  return files
})

// The pnpm version of tsrs/package.json's `packageManager`, without its integrity hash.
export const parseTsrsPnpmVersion = (packageJson: string): string | undefined => {
  const packageManager: unknown = JSON.parse(packageJson).packageManager
  return typeof packageManager === "string" ? /^pnpm@([^+]+)/.exec(packageManager)?.[1] : undefined
}

// pgo-train.sh installs the bench projects with `pnpm --pm-on-fail=ignore`, which this repository's pnpm does not
// know: it runs with tsrs's pinned pnpm, as in tsrs's job (setup-node installs the packageManager one). Returns its
// bin directory.
const installTsrsPnpm = Effect.fnUntraced(function*(tsrsDirectory: string, layout: TsrsReleaseLayout) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const version = parseTsrsPnpmVersion(yield* fs.readFileString(path.join(tsrsDirectory, "package.json")))
  if (version === undefined) {
    return yield* new TsrsReleaseError({ reason: "No pnpm packageManager in tsrs/package.json" })
  }
  yield* fs.makeDirectory(layout.pnpm, { recursive: true })
  yield* runCommand("npm", layout.pnpm, ["install", "--prefix", layout.pnpm, "--no-audit", "--no-fund", `pnpm@${version}`])
  return path.join(layout.pnpm, "node_modules", ".bin")
})

// tsrs-test reads the conformance fixtures from `<tsrs>/ts-ref/tsc/testdata` and the Go fourslash runner archives
// `tsc` from the `ts-ref` repository: the TypeScript checkout setup materializes for the tsrs component is that commit
// (our TypeScript patches change no testdata, and `git archive` reads the commit, not the working tree).
const prepareTypeScriptReference = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const tsrsDirectory = path.join(repositoryRoot, "tsrs")
  const typescriptDirectory = path.join(repositoryRoot, "typescript")
  const commit = parseTsrsTypeScriptCommit(yield* fs.readFileString(path.join(tsrsDirectory, "Cargo.toml")))
  if (commit === undefined) {
    return yield* new TsrsReleaseError({ reason: "No [workspace.metadata.typescript] commit in tsrs/Cargo.toml" })
  }
  const head = (yield* runCommandString("git", typescriptDirectory, ["rev-parse", "HEAD"])).trim()
  if (head !== commit) {
    return yield* new TsrsReleaseError({
      reason: `tsrs ports TypeScript ${commit}, but typescript/ is at ${head} (setup with --component tsrs)`
    })
  }
  const testdataChanges = yield* runCommandString("git", typescriptDirectory, [
    "status",
    "--porcelain",
    "--",
    "tsc/testdata"
  ])
  if (testdataChanges.trim() !== "") {
    return yield* new TsrsReleaseError({ reason: `typescript/tsc/testdata has local changes:\n${testdataChanges}` })
  }
  const reference = path.join(tsrsDirectory, "ts-ref")
  if (process.platform !== "win32") {
    const existing = yield* fs.readLink(reference).pipe(Effect.orElseSucceed(() => undefined))
    if (existing === undefined) {
      if (yield* fs.exists(reference)) {
        return yield* new TsrsReleaseError({ reason: `${reference} exists and is not a link to typescript/` })
      }
      yield* fs.symlink(path.join("..", "typescript"), reference)
    } else if (path.resolve(tsrsDirectory, existing) !== typescriptDirectory) {
      return yield* new TsrsReleaseError({ reason: `${reference} links to ${existing}, not typescript/` })
    }
  }
  return { TSRS_TESTDATA: path.join(typescriptDirectory, "tsc", "testdata") }
})

// pgo-train.sh's Effect counterpart: the instrumented tsrs builds the Effect workload once from clean.
const trainEffectWorkload = Effect.fnUntraced(function*(
  layout: TsrsReleaseLayout,
  tsrs: string,
  environment: Record<string, string>
) {
  const path = yield* Path.Path
  yield* prepareWorkload(layout.effectWorkload)
  yield* runCommand("pnpm", layout.effectWorkload, ["clean"], true)
  yield* Console.log("Training on the Effect workload")
  const exitCode = yield* runCommandExitCode(tsrs, layout.effectWorkload, [
    "-b",
    "tsconfig.json",
    "--pretty",
    "false"
  ], { ...environment, LLVM_PROFILE_FILE: path.join(layout.rawProfiles, "effect-%p.profraw") })
  // 0/1/2 are tsc exit codes, anything else is a crash.
  if (exitCode > 2) {
    return yield* new TsrsReleaseError({ reason: `instrumented tsrs exited with ${exitCode} on the Effect workload` })
  }
  if ((yield* nonEmptyFiles(layout.rawProfiles, "effect-")).length === 0) {
    return yield* new TsrsReleaseError({ reason: `no profile with counts from the Effect workload in ${layout.rawProfiles}` })
  }
})

// tsrs's "Merge profiles" step: the profile's hash is part of its path, and so of RUSTFLAGS, so that cargo rebuilds
// the dependencies whenever the profile changes.
const mergeProfiles = Effect.fnUntraced(function*(repositoryRoot: string, layout: TsrsReleaseLayout) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const sysroot = (yield* runCommandString("rustc", repositoryRoot, ["--print", "sysroot"])).trim()
  const host = /^host: (\S+)$/m.exec(yield* runCommandString("rustc", repositoryRoot, ["-vV"]))?.[1]
  if (host === undefined) return yield* new TsrsReleaseError({ reason: "rustc -vV printed no host" })
  const profdata = path.join(
    sysroot,
    "lib",
    "rustlib",
    host,
    "bin",
    process.platform === "win32" ? "llvm-profdata.exe" : "llvm-profdata"
  )
  const profiles = yield* nonEmptyFiles(layout.rawProfiles, "")
  if (profiles.length === 0) return yield* new TsrsReleaseError({ reason: `no profiles in ${layout.rawProfiles}` })
  const merged = path.join(layout.work, "pgo", "merged.profdata")
  yield* runCommand(profdata, repositoryRoot, ["merge", "-o", merged, ...profiles])
  const summary = (yield* runCommandString(profdata, repositoryRoot, ["show", merged])).trimEnd().split("\n")
  yield* Console.log(summary.slice(-5).join("\n"))
  const hash = createHash("sha256").update(yield* fs.readFile(merged)).digest("hex").slice(0, 16)
  const profile = path.join(layout.work, "pgo", `${hash}.profdata`)
  yield* fs.rename(merged, profile)
  return profile
})

// tsrs's "Install BOLT" step, into `<work>/llvm` (cached by the release workflow).
const ensureBoltTools = Effect.fnUntraced(function*(repositoryRoot: string, layout: TsrsReleaseLayout) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const binaries = [path.join(layout.llvm, "bin", "llvm-bolt"), path.join(layout.llvm, "bin", "merge-fdata")]
  const runtime = path.join(layout.llvm, "lib", "libbolt_rt_instr.a")
  const present = yield* Effect.forEach([...binaries, runtime], (file) => fs.exists(file))
  if (present.every(Boolean)) return path.join(layout.llvm, "bin")
  const llvmVersion = parseLlvmVersion(yield* runCommandString("rustc", repositoryRoot, ["-vV"]))
  if (llvmVersion === undefined) return yield* new TsrsReleaseError({ reason: "rustc -vV printed no LLVM version" })
  const tools = boltToolsUrl(llvmVersion, process.arch)
  yield* Console.log(`Fetching BOLT from ${tools.url}`)
  yield* fs.makeDirectory(layout.llvm, { recursive: true })
  yield* runCommand("bash", repositoryRoot, [
    "-c",
    `curl -fsSL "$0" | tar -x --zstd -C "$1" --strip-components=1 --wildcards ${tools.files.map(shellQuote).join(" ")}`,
    tools.url,
    layout.llvm
  ])
  return path.join(layout.llvm, "bin")
})

// bolt.sh's conformance gate runs tsrs_determinism; it is built here without --locked (the patched checkout's
// Cargo.lock lacks the Effect crates), and the checkout's Cargo.lock is restored afterwards.
const buildDeterminism = Effect.fnUntraced(function*(repositoryRoot: string, layout: TsrsReleaseLayout) {
  const path = yield* Path.Path
  const tsrsDirectory = path.join(repositoryRoot, "tsrs")
  yield* Effect.ensuring(
    runCommand("cargo", tsrsDirectory, ["build", "--profile", "dev", "-q", "-p", "tsrs_determinism"], false, {
      CARGO_TARGET_DIR: layout.determinismTarget
    }),
    runCommand("git", tsrsDirectory, ["checkout", "--", "Cargo.lock"], true).pipe(Effect.ignore)
  )
  return path.join(layout.determinismTarget, "debug", "tsrs_determinism")
})

const releaseWorkDirectory = Effect.fnUntraced(function*(repositoryRoot: string) {
  const path = yield* Path.Path
  return process.env.RUNNER_TEMP === undefined
    ? path.join(repositoryRoot, "target", "tsrs-release")
    : path.join(process.env.RUNNER_TEMP, "tsrs-release")
})

// The PGO (and on BOLT targets, BOLT-optimized) tsrs binary for the host target; returns its path. Needs network
// (the bench projects, the Effect workload and BOLT), Node.js, pnpm and Go (the fourslash training), as tsrs's job.
export const buildOptimizedTsrsBinary = Effect.fnUntraced(function*(
  repositoryRoot: string,
  target: TsrsBuildTarget,
  optimization: Exclude<TsrsOptimization, "none">
) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const tsrsDirectory = path.join(repositoryRoot, "tsrs")
  const rustTarget = tsrsBuildTargets[target].rustTarget
  const layout = tsrsReleaseLayout(yield* releaseWorkDirectory(repositoryRoot), path.sep)
  const windows = target.startsWith("win32-")
  const exe = windows ? ".exe" : ""
  if (usesBolt(optimization) && !target.startsWith("linux-")) {
    return yield* new TsrsReleaseError({ reason: `BOLT is for the Linux targets only, not ${target}` })
  }
  yield* Console.log(`Building the ${optimization} tsrs binary for ${target} (work directory ${layout.work})`)
  yield* runCommand("rustup", repositoryRoot, ["component", "add", "llvm-tools"])
  yield* runCommand("rustup", repositoryRoot, ["target", "add", rustTarget])
  const reference = yield* prepareTypeScriptReference(repositoryRoot)
  const libraries = { TSRS_LIB_PATH: path.join(tsrsDirectory, "crates", "tsrs_vfs", "libs") }

  // Instrumented build, training and merge, from fresh profiles.
  yield* fs.remove(layout.rawProfiles, { recursive: true, force: true })
  yield* fs.makeDirectory(layout.rawProfiles, { recursive: true })
  yield* Console.log("Instrumented build")
  yield* runCommand(
    "cargo",
    repositoryRoot,
    tsrsCargoBuildArgs(target, true),
    false,
    tsrsCargoEnvironment({
      target,
      optimization,
      phase: { kind: "generate", rawProfiles: layout.rawProfiles, targetDirectory: layout.instrumentedTarget }
    })
  )
  const instrumented = path.join(layout.instrumentedTarget, rustTarget, "release")
  yield* Console.log("Training run")
  if (windows) {
    // pgo-train-windows.ps1 reads its paths from the GitHub variables of tsrs's job.
    yield* runCommand("pwsh", tsrsDirectory, [
      "-NoProfile",
      "-File",
      path.join(".github", "scripts", "pgo-train-windows.ps1"),
      "-BinDir",
      instrumented,
      "-RawDir",
      layout.rawProfiles
    ], false, { ...reference, GITHUB_WORKSPACE: tsrsDirectory, RUNNER_TEMP: layout.work })
  } else {
    const pnpmBin = yield* installTsrsPnpm(tsrsDirectory, layout)
    yield* runCommand("bash", tsrsDirectory, [
      path.join(".github", "scripts", "pgo-train.sh"),
      instrumented,
      layout.rawProfiles,
      layout.bench
    ], false, { ...reference, ...libraries, PATH: `${pnpmBin}:${process.env.PATH ?? ""}` })
  }
  yield* trainEffectWorkload(layout, path.join(instrumented, `tsrs${exe}`), libraries)
  const profile = yield* mergeProfiles(repositoryRoot, layout)

  yield* Console.log("Final build")
  yield* runCommand(
    "cargo",
    repositoryRoot,
    tsrsCargoBuildArgs(target, usesBolt(optimization)),
    false,
    tsrsCargoEnvironment({ target, optimization, phase: { kind: "use", profile } })
  )
  const targetDirectory = process.env.CARGO_TARGET_DIR ?? path.join(repositoryRoot, "target")
  const dist = path.join(targetDirectory, rustTarget, "release")

  if (usesBolt(optimization)) {
    const llvmBin = yield* ensureBoltTools(repositoryRoot, layout)
    const determinism = yield* buildDeterminism(repositoryRoot, layout)
    yield* fs.makeDirectory(layout.bolt, { recursive: true })
    const workload = path.join(layout.bolt, "effect-workload.sh")
    yield* fs.writeFileString(workload, effectWorkloadScript(layout.effectWorkload))
    yield* fs.chmod(workload, 0o755)
    yield* Console.log("BOLT and gates on the BOLT-optimized binaries")
    yield* runCommand("bash", tsrsDirectory, [
      path.join(".github", "scripts", "bolt.sh"),
      dist,
      layout.bolt,
      layout.bench
    ], false, {
      ...reference,
      ...libraries,
      PATH: `${llvmBin}:${process.env.PATH ?? ""}`,
      BOLT_EXTRA_WORKLOAD: workload,
      TSRS_DETERMINISM: determinism
    })
  }
  return path.join(dist, `tsrs${exe}`)
})

// The settings of tsrs's release workflow that `tsrsBuildTargets`, `tsrsCargoEnvironment`, `tsrsCargoBuildArgs` and
// `buildOptimizedTsrsBinary` mirror, verbatim: when tsrs changes one of them, `repoctl tsrs check` fails and this
// module is updated to match (then this list).
export const tsrsReleaseWorkflowSnippets = [
  // `build`
  "TSRS_CARGO_FEATURES: tsrs_cli/noembed",
  "CARGO_PROFILE_RELEASE_STRIP: ${{ contains(matrix.target, 'linux') && 'none' || 'symbols' }}",
  "RUSTFLAGS: -Cprofile-generate=${{ runner.temp }}/pgo/raw -Clto=thin",
  "CFLAGS: ${{ runner.os == 'macOS' && '-fno-profile-generate' || '' }}",
  "run: cargo build --release --locked -p tsrs_cli -p tsrs_testrunner -p tsrs_fourslash_server --bins --features \"$TSRS_CARGO_FEATURES\" --target ${{ matrix.target }}",
  "run: .github/scripts/pgo-train.sh \"$RUNNER_TEMP/pgo/target/${{ matrix.target }}/release\" \"$RUNNER_TEMP/pgo/raw\" \"$RUNNER_TEMP/pgo/bench\"",
  "\"$profdata\" merge -o \"$RUNNER_TEMP/pgo/merged.profdata\" \"$RUNNER_TEMP\"/pgo/raw/*.profraw",
  "RUSTFLAGS: -Cprofile-use=${{ steps.profile.outputs.path }}${{ runner.os == 'Linux' && ' -Clink-arg=-Wl,--emit-relocs' || '' }}",
  "CFLAGS: ${{ runner.os == 'macOS' && '-fno-profile-use' || '' }}",
  "targets=(-p tsrs_cli)",
  "[ \"$RUNNER_OS\" = Linux ] && targets+=(-p tsrs_testrunner -p tsrs_fourslash_server --bins)",
  "cargo build --release --locked \"${targets[@]}\" --features \"$TSRS_CARGO_FEATURES\" --target ${{ matrix.target }}",
  "llvm=$(rustc -vV | sed -n 's/^LLVM version: //p')",
  "curl -fsSL \"https://github.com/llvm/llvm-project/releases/download/llvmorg-$LLVM_VERSION/$name.tar.zst\" |",
  "run: .github/scripts/bolt.sh \"target/${{ matrix.target }}/release\" \"$RUNNER_TEMP/bolt\" \"$RUNNER_TEMP/pgo/bench\"",
  // `build-windows`
  "name: build x86_64-pc-windows-msvc",
  "runs-on: windows-latest",
  "CARGO_PROFILE_RELEASE_STRIP: symbols",
  "$env:CARGO_ENCODED_RUSTFLAGS = @(\"-Cprofile-generate=$raw\", \"-Clto=thin\") -join [char]31",
  "cargo build --release --locked -p tsrs_cli -p tsrs_testrunner --bins --features tsrs_cli/noembed --target x86_64-pc-windows-msvc",
  "& ./.github/scripts/pgo-train-windows.ps1 -BinDir $bin -RawDir $raw",
  "$env:CARGO_ENCODED_RUSTFLAGS = \"-Cprofile-use=$env:PROFILE_PATH\"",
  "cargo build --release --locked -p tsrs_cli --features tsrs_cli/noembed --target x86_64-pc-windows-msvc"
] as const

// The `{ target: ..., os: ... }` rows of the `build` and `build-musl` matrices and the target of `build-windows`.
export const parseTsrsReleaseTargets = (workflow: string) => {
  const targets = new Map<string, string>()
  for (const match of workflow.matchAll(/^\s*- \{ target: ([\w-]+), os: ([\w.-]+)/gm)) {
    targets.set(match[1]!, match[2]!)
  }
  const windows = /^ {2}build-windows:\n(?: {4}.*\n|\n)*/m.exec(workflow)?.[0]
  const windowsTarget = windows === undefined ? undefined : /name: build ([\w-]+)/.exec(windows)?.[1]
  const windowsRunner = windows === undefined ? undefined : /runs-on: ([\w.-]+)/.exec(windows)?.[1]
  if (windowsTarget !== undefined && windowsRunner !== undefined) targets.set(windowsTarget, windowsRunner)
  return targets
}

// The differences between tsrs's release workflow and what this module mirrors; empty when they agree.
export const tsrsReleaseParityProblems = (workflow: string) => {
  const problems: Array<string> = []
  for (const snippet of tsrsReleaseWorkflowSnippets) {
    if (!workflow.includes(snippet)) problems.push(`no longer contains: ${snippet}`)
  }
  // tsrs's jobs build C code with the runner's default compiler.
  if (/^\s+(?:TARGET_)?CC:/m.test(workflow)) problems.push("sets a C compiler (CC or TARGET_CC)")
  const upstream = parseTsrsReleaseTargets(workflow)
  const ours = new Map<string, string>(
    Object.values(tsrsBuildTargets).map(({ rustTarget, runner }) => [rustTarget, runner])
  )
  for (const [target, runner] of upstream) {
    if ((tsrsSkippedUpstreamTargets as ReadonlyArray<string>).includes(target)) continue
    const own = ours.get(target)
    if (own === undefined) problems.push(`builds ${target} (on ${runner}), which tsrsBuildTargets lacks`)
    else if (own !== runner) problems.push(`builds ${target} on ${runner}, tsrsBuildTargets on ${own}`)
  }
  for (const target of ours.keys()) {
    if (!upstream.has(target)) problems.push(`no longer builds ${target}`)
  }
  return problems
}

// `repoctl tsrs check`: the release build still matches tsrs's release workflow at the checked-out commit.
export const checkTsrsReleaseParity = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const file = path.join(repositoryRoot, "tsrs", ".github", "workflows", "release.yml")
  const problems = tsrsReleaseParityProblems(yield* fs.readFileString(file))
  if (problems.length > 0) {
    return yield* new TsrsReleaseError({
      reason: `tsrs's release workflow (${file}) differs from _tools/repoctl/src/tsrsRelease.ts:\n${
        problems.map((problem) => `- ${problem}`).join("\n")
      }`
    })
  }
  yield* Console.log("The tsrs release build matches tsrs's release workflow")
})
