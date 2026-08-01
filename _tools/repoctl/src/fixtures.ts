import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import { createHash } from "node:crypto"
import { mkdir, readFile, readdir, realpath, stat, writeFile } from "node:fs/promises"
import { dirname, join, posix, relative, sep } from "node:path"
import { gzipSync } from "node:zlib"

const fixtureProfiles = ["effect-v3", "effect-v4"] as const
const archiveRelativePath = join("internal", "bundledeffect", "testfixtures.tar.gz")
const compareText = (left: string, right: string): number => left < right ? -1 : left > right ? 1 : 0

interface PackageJson {
  readonly dependencies?: Record<string, unknown>
  readonly version?: unknown
}

export interface FixtureProfileManifest {
  readonly requested: Record<string, string>
  readonly resolved: Record<string, string>
}

export interface FixtureManifest {
  readonly schemaVersion: 1
  readonly profiles: Record<string, FixtureProfileManifest>
  readonly treeSha256: string
}

interface ArchiveEntry {
  readonly path: string
  readonly content: Buffer
}

export class FixtureGenerationError extends Data.TaggedError("FixtureGenerationError")<{
  readonly reason: string
}> {
  get message(): string {
    return `Fixture generation failed: ${this.reason}`
  }
}

const sortedRecord = <A>(entries: Iterable<readonly [string, A]>): Record<string, A> =>
  Object.fromEntries(Array.from(entries).sort(([left], [right]) => compareText(left, right)))

const parsePackageJson = async(path: string): Promise<PackageJson> => {
  try {
    return JSON.parse(await readFile(path, "utf8")) as PackageJson
  } catch (error) {
    throw new FixtureGenerationError({ reason: `Cannot read ${path}: ${String(error)}` })
  }
}

const collectDirectory = async(
  directory: string,
  archiveDirectory: string,
  entries: Array<ArchiveEntry>,
  ancestors: ReadonlySet<string> = new Set()
): Promise<void> => {
  const resolved = await realpath(directory)
  if (ancestors.has(resolved)) {
    throw new FixtureGenerationError({ reason: `Fixture symlink cycle at ${directory}` })
  }
  const nextAncestors = new Set(ancestors)
  nextAncestors.add(resolved)
  const children = (await readdir(directory)).sort()
  for (const child of children) {
    const source = join(directory, child)
    const info = await stat(source)
    const archivePath = posix.join(archiveDirectory, child)
    if (info.isDirectory()) {
      await collectDirectory(source, archivePath, entries, nextAncestors)
    } else if (info.isFile()) {
      entries.push({ path: archivePath, content: await readFile(source) })
    }
  }
}

const treeSha256 = (entries: ReadonlyArray<ArchiveEntry>): string => {
  const hash = createHash("sha256")
  for (const entry of entries) {
    hash.update(entry.path)
    hash.update("\0")
    hash.update(createHash("sha256").update(entry.content).digest())
  }
  return hash.digest("hex")
}

const writeString = (buffer: Buffer, offset: number, length: number, value: string): void => {
  buffer.write(value, offset, Math.min(length, Buffer.byteLength(value)), "utf8")
}

const writeOctal = (buffer: Buffer, offset: number, length: number, value: number): void => {
  writeString(buffer, offset, length, `${value.toString(8).padStart(length - 1, "0")}\0`)
}

const tarHeader = (name: string, size: number, type: "0" | "L" = "0"): Buffer => {
  const header = Buffer.alloc(512)
  writeString(header, 0, 100, name)
  writeOctal(header, 100, 8, 0o644)
  writeOctal(header, 108, 8, 0)
  writeOctal(header, 116, 8, 0)
  writeOctal(header, 124, 12, size)
  writeOctal(header, 136, 12, 0)
  header.fill(0x20, 148, 156)
  writeString(header, 156, 1, type)
  writeString(header, 257, 6, "ustar\0")
  writeString(header, 263, 2, "00")
  const checksum = header.reduce((sum, byte) => sum + byte, 0)
  writeString(header, 148, 8, `${checksum.toString(8).padStart(6, "0")}\0 `)
  return header
}

const tarEntry = (entry: ArchiveEntry): Array<Buffer> => {
  const result: Array<Buffer> = []
  const pathBytes = Buffer.from(entry.path)
  if (pathBytes.length > 100) {
    const longName = Buffer.concat([pathBytes, Buffer.from([0])])
    result.push(tarHeader("././@LongLink", longName.length, "L"), longName)
    const longNamePadding = (512 - longName.length % 512) % 512
    if (longNamePadding > 0) result.push(Buffer.alloc(longNamePadding))
  }
  result.push(tarHeader(entry.path, entry.content.length), entry.content)
  const padding = (512 - entry.content.length % 512) % 512
  if (padding > 0) result.push(Buffer.alloc(padding))
  return result
}

const createTarGzip = (entries: ReadonlyArray<ArchiveEntry>): Buffer => {
  const chunks = entries.flatMap(tarEntry)
  chunks.push(Buffer.alloc(1024))
  const archive = gzipSync(Buffer.concat(chunks), { level: 9 })
  archive[9] = 0xff
  return archive
}

export const createFixtureArchive = async(repositoryRoot: string): Promise<{
  readonly archive: Buffer
  readonly manifest: FixtureManifest
}> => {
  const entries: Array<ArchiveEntry> = []
  const profiles: Array<readonly [string, FixtureProfileManifest]> = []

  for (const profile of fixtureProfiles) {
    const profileRoot = join(repositoryRoot, "testdata", "tests", profile)
    const packageJsonPath = join(profileRoot, "package.json")
    const packageJsonText = await readFile(packageJsonPath)
    const packageJson = await parsePackageJson(packageJsonPath)
    const dependencyEntries = Object.entries(packageJson.dependencies ?? {})
    const requested: Array<readonly [string, string]> = []
    const resolved: Array<readonly [string, string]> = []
    entries.push({ path: `${profile}/package.json`, content: packageJsonText })

    for (const [packageName, specifier] of dependencyEntries.sort(([left], [right]) => compareText(left, right))) {
      if (typeof specifier !== "string") {
        throw new FixtureGenerationError({ reason: `${packageJsonPath} has a non-string dependency ${packageName}` })
      }
      const packageRoot = join(profileRoot, "node_modules", ...packageName.split("/"))
      const resolvedPackageJson = await parsePackageJson(join(packageRoot, "package.json"))
      if (typeof resolvedPackageJson.version !== "string") {
        throw new FixtureGenerationError({ reason: `${packageName} has no resolved version in ${packageRoot}` })
      }
      requested.push([packageName, specifier])
      resolved.push([packageName, resolvedPackageJson.version])
      await collectDirectory(packageRoot, posix.join(profile, "node_modules", packageName), entries)
    }
    profiles.push([profile, {
      requested: sortedRecord(requested),
      resolved: sortedRecord(resolved)
    }])
  }

  entries.sort((left, right) => compareText(left.path, right.path))
  const manifest: FixtureManifest = {
    schemaVersion: 1,
    profiles: sortedRecord(profiles),
    treeSha256: treeSha256(entries)
  }
  entries.push({ path: "manifest.json", content: Buffer.from(`${JSON.stringify(manifest, null, 2)}\n`) })
  return { archive: createTarGzip(entries), manifest }
}

export const ensureEffectFixtures = Effect.fnUntraced(function*(repositoryRoot: string) {
  const output = join(repositoryRoot, archiveRelativePath)
  const result = yield* Effect.tryPromise({
    try: () => createFixtureArchive(repositoryRoot),
    catch: (error) => error instanceof FixtureGenerationError
      ? error
      : new FixtureGenerationError({ reason: String(error) })
  })
  const current = yield* Effect.tryPromise({
    try: async() => {
      try {
        return await readFile(output)
      } catch (error) {
        return (error as NodeJS.ErrnoException).code === "ENOENT" ? undefined : Promise.reject(error)
      }
    },
    catch: (error) => new FixtureGenerationError({ reason: `Cannot read ${output}: ${String(error)}` })
  })
  if (current?.equals(result.archive)) return result.manifest

  yield* Console.log(`Generating ${relative(repositoryRoot, output).split(sep).join("/")}`)
  yield* Effect.tryPromise({
    try: async() => {
      await mkdir(dirname(output), { recursive: true })
      await writeFile(output, result.archive)
    },
    catch: (error) => new FixtureGenerationError({ reason: `Cannot write ${output}: ${String(error)}` })
  })
  return result.manifest
})
