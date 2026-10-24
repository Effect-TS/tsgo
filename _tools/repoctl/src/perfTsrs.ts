// The Rust engine in `repoctl perf compare --engine tsrs`: "stock" is tsrs's own CLI (tsrs_cli) built from a clean
// checkout of the pinned tsrs commit, without _patches/tsrs and without the Effect crates; "patched" is our ets_tsrs
// binary. Both are built with the [profile.release] of this repository's Cargo.toml, so the only difference between
// them is our patches and rules. tsrs's npm release binary is PGO/BOLT-optimized and is not a fair stock baseline.
import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import { runCommand, runCommandString } from "./process.ts"
import { buildRustBinary, missingRustBinaryTools } from "./rust.ts"
import { getTsrsComponent, readUpstream, tsrsRepository } from "./upstream.ts"

export class TsrsPerfError extends Data.TaggedError("TsrsPerfError")<{
  readonly reason: string
}> {
  get message(): string {
    return `Performance comparison failed: ${this.reason}`
  }
}

// The `[profile.release]` entries of a Cargo.toml, as `cargo --config` overrides (`profile.release.lto="fat"`).
// Only the flat `key = value` form the repository uses is supported.
export const releaseProfileOverrides = (cargoToml: string): ReadonlyArray<string> => {
  const overrides: Array<string> = []
  let inSection = false
  for (const rawLine of cargoToml.split(/\r?\n/)) {
    const line = rawLine.replace(/\s+#.*$/, "").trim()
    if (line === "" || line.startsWith("#")) continue
    if (line.startsWith("[")) {
      inSection = line === "[profile.release]"
      continue
    }
    if (!inSection) continue
    const match = /^([A-Za-z0-9_-]+)\s*=\s*(.+)$/.exec(line)
    if (match !== null) overrides.push(`profile.release.${match[1]}=${match[2]}`)
  }
  return overrides
}

// The stock checkout: a clean clone of the pinned tsrs commit under a gitignored directory, never patched.
export const stockTsrsDirectory = (repositoryRoot: string, gitHead: string, path: Path.Path) =>
  path.join(repositoryRoot, "_tools", ".perf-tsrs", gitHead)

const ensureStockCheckout = Effect.fnUntraced(function*(directory: string, gitHead: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  if (!(yield* fs.exists(path.join(directory, "Cargo.toml")))) {
    yield* Console.log(`Fetching stock tsrs ${tsrsRepository}@${gitHead} into ${directory}`)
    yield* fs.remove(directory, { recursive: true, force: true })
    yield* fs.makeDirectory(directory, { recursive: true })
    yield* runCommand("git", directory, ["init", "--quiet"])
    yield* runCommand("git", directory, ["remote", "add", "origin", tsrsRepository])
    yield* runCommand("git", directory, ["fetch", "--quiet", "--depth", "1", "origin", gitHead])
    yield* runCommand("git", directory, ["checkout", "--quiet", "FETCH_HEAD"])
  }
  const head = (yield* runCommandString("git", directory, ["rev-parse", "HEAD"])).trim()
  if (head !== gitHead) {
    return yield* new TsrsPerfError({ reason: `Stock tsrs checkout ${directory} is at ${head}, expected ${gitHead}` })
  }
  const status = (yield* runCommandString("git", directory, ["status", "--porcelain"])).trim()
  if (status !== "") {
    return yield* new TsrsPerfError({ reason: `Stock tsrs checkout ${directory} has local changes:\n${status}` })
  }
})

// Builds stock tsrs (plain tsrs_cli) at the pinned commit with our release profile and returns its path.
export const buildStockTsrs = Effect.fnUntraced(function*(repositoryRoot: string) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const component = yield* getTsrsComponent(yield* readUpstream(repositoryRoot))
  if (component === undefined) {
    return yield* new TsrsPerfError({ reason: "No tsrs component in _packages/tsgo/upstream.json" })
  }
  const directory = stockTsrsDirectory(repositoryRoot, component.gitHead, path)
  yield* ensureStockCheckout(directory, component.gitHead)
  const overrides = releaseProfileOverrides(yield* fs.readFileString(path.join(repositoryRoot, "Cargo.toml")))
  if (overrides.length === 0) {
    return yield* new TsrsPerfError({ reason: "No [profile.release] in Cargo.toml" })
  }
  const targetDirectory = path.join(directory, "target")
  yield* Console.log(`Building stock tsrs ${component.version} (tsrs_cli; ${overrides.join(" ")})`)
  yield* runCommand("cargo", directory, [
    "build",
    "-p",
    "tsrs_cli",
    "--release",
    "--locked",
    "--target-dir",
    targetDirectory,
    ...overrides.flatMap((override) => ["--config", override])
  ])
  return path.join(targetDirectory, "release", process.platform === "win32" ? "tsrs.exe" : "tsrs")
})

// The stock and patched tsrs binaries; explicit paths are used as they are, missing ones are built (cargo is a no-op
// when they are up to date).
export const resolveTsrsBinaries = Effect.fnUntraced(function*(
  repositoryRoot: string,
  patchedBin: string | undefined,
  stockBin: string | undefined
) {
  if (patchedBin === undefined || stockBin === undefined) {
    const missing = yield* missingRustBinaryTools(repositoryRoot)
    if (missing !== undefined) {
      return yield* new TsrsPerfError({ reason: `Cannot build the tsrs binaries: ${missing}` })
    }
  }
  const patched = patchedBin ?? (yield* buildRustBinary(repositoryRoot, { release: true }))
  const stock = stockBin ?? (yield* buildStockTsrs(repositoryRoot))
  return { patched, stock }
})

// The line GNU time writes with `-f peakRssFormat` (after "Command exited with non-zero status N" on failure).
export const peakRssFormat = "perf-resources maxrss=%M"

// The peak resident set size in kilobytes from GNU time's output, if present.
export const parsePeakRss = (output: string): number | undefined => {
  const match = [...output.matchAll(/^perf-resources maxrss=(\d+)$/gm)].at(-1)
  return match === undefined ? undefined : Number(match[1])
}

// GNU time (Linux) measures the peak resident set size of the compiler; elsewhere it is not measured.
export const gnuTime = Effect.fnUntraced(function*() {
  const fs = yield* FileSystem.FileSystem
  return process.platform === "linux" && (yield* fs.exists("/usr/bin/time")) ? "/usr/bin/time" : undefined
})
