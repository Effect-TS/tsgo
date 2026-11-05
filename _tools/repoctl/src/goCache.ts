import * as ByteSize from "effect/ByteSize"
import * as Console from "effect/Console"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Option from "effect/Option"
import * as Path from "effect/Path"
import { runCommandString } from "./process.ts"

// Files Go keeps at the cache root to recognize the directory as a build cache; never removed.
const protectedRootFiles = new Set(["README", "trim.txt"])

export interface CacheFile {
  readonly path: string
  readonly size: bigint
  readonly mtimeMs: number
}

export interface TrimResult {
  readonly bytesBefore: bigint
  readonly bytesAfter: bigint
  readonly filesBefore: number
  readonly filesRemoved: number
}

// Keeps the newest files until the byte budget is used and returns the rest. Go bumps an entry's mtime when it
// reads an entry older than one hour, and actions/cache restores the archived mtimes, so at the end of a job the
// entries it used are the newest ones: this is an LRU size cap. Removing any entry is safe, a miss recompiles.
export const selectEvictions = (
  files: ReadonlyArray<CacheFile>,
  maxBytes: bigint
): ReadonlyArray<CacheFile> => {
  const sorted = [...files].sort((a, b) => b.mtimeMs - a.mtimeMs || a.path.localeCompare(b.path))
  const evicted: Array<CacheFile> = []
  let kept = 0n
  for (const file of sorted) {
    if (kept + file.size <= maxBytes) {
      kept += file.size
    } else {
      evicted.push(file)
    }
  }
  return evicted
}

export const trimDirectory = Effect.fnUntraced(function*(root: string, maxBytes: bigint) {
  const fs = yield* FileSystem.FileSystem
  const path = yield* Path.Path
  const entries = yield* fs.readDirectory(root, { recursive: true })
  const stats = yield* Effect.forEach(
    entries,
    (entry) => Effect.map(fs.stat(path.join(root, entry)), (info) => ({ entry, info })),
    { concurrency: 64 }
  )
  const files = stats.flatMap(({ entry, info }) =>
    info.type === "File"
      ? [{
        path: entry,
        size: BigInt(info.size),
        mtimeMs: Option.match(info.mtime, { onNone: () => 0, onSome: (date) => date.getTime() })
      }]
      : []
  )
  const bytesBefore = files.reduce((total, file) => total + file.size, 0n)
  const isProtected = (file: CacheFile) => protectedRootFiles.has(file.path)
  const protectedBytes = files.filter(isProtected).reduce((total, file) => total + file.size, 0n)
  const budget = maxBytes > protectedBytes ? maxBytes - protectedBytes : 0n
  const evicted = selectEvictions(files.filter((file) => !isProtected(file)), budget)
  yield* Effect.forEach(evicted, (file) => fs.remove(path.join(root, file.path)), {
    concurrency: 64,
    discard: true
  })
  const removedBytes = evicted.reduce((total, file) => total + file.size, 0n)
  return {
    bytesBefore,
    bytesAfter: bytesBefore - removedBytes,
    filesBefore: files.length,
    filesRemoved: evicted.length
  } satisfies TrimResult
})

export const parseByteSize = (input: string): Option.Option<bigint> =>
  ByteSize.fromString(input)

const formatBytes = (bytes: bigint) => ByteSize.format(ByteSize.bytes(bytes), { system: "decimal" })

// Go only trims GOCACHE entries unused for five days; CI saves the cache after each main run, so cap its size.
export const trimGoCache = Effect.fnUntraced(function*(repositoryRoot: string, maxBytes: bigint) {
  const fs = yield* FileSystem.FileSystem
  const cache = (yield* runCommandString("go", repositoryRoot, ["env", "GOCACHE"])).trim()
  if (cache === "" || cache === "off" || !(yield* fs.exists(cache))) {
    yield* Console.log(`Go build cache ${cache === "" ? "(unset)" : cache} does not exist; nothing to trim`)
    return
  }
  const result = yield* trimDirectory(cache, maxBytes)
  yield* Console.log(
    `Go build cache ${cache}: ${formatBytes(result.bytesBefore)} in ${result.filesBefore} files before, ` +
      `${formatBytes(result.bytesAfter)} after removing ${result.filesRemoved} files (limit ${formatBytes(maxBytes)})`
  )
})
