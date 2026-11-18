import * as NodeCrypto from "@effect/platform-node/NodeCrypto"
import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Digest from "effect/crypto/Digest"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import * as Scope from "effect/Scope"
import { createHash } from "node:crypto"
import { existsSync } from "node:fs"
import { mkdir, mkdtemp, readFile, rm, rmdir, stat, writeFile } from "node:fs/promises"
import { createRequire } from "node:module"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"
import { afterEach, describe, expect, it, vi } from "vitest"
import upstreamJson from "../upstream.json" with { type: "json" }
import {
  discoverBinaries,
  type DiscoveredBinary,
  isGlibc,
  packagedArtifactPath,
  patch,
  preparePatch,
  ReplacementUnavailableError,
  requireComponents,
  resolveReplacement,
  selectComponents,
  tsrsReplacementTargets,
  tsrsTarget,
  unpatch,
  UnsupportedTargetPackageVersionError
} from "../src/patcher/index.js"

const temporaryDirectories: Array<string> = []
const cleanups: Array<() => Promise<void>> = []

const hash = (value: string) => createHash("sha256").update(value).digest("hex")
const supportedTsrsVersion = Object.keys(upstreamJson.components.tsrs).sort().at(-1)!

const run = <A, E>(effect: Effect.Effect<A, E, Digest.Digest | FileSystem.FileSystem | Path.Path | Scope.Scope>) =>
  Effect.runPromise(Effect.scoped(effect).pipe(Effect.provide([NodeServices.layer, NodeCrypto.layerDigest])))

const makeTemporaryDirectory = async () => {
  const directory = await mkdtemp(join(tmpdir(), "effect-tsgo-tsrs-"))
  temporaryDirectories.push(directory)
  return directory
}

const writePackage = async (directory: string, packageName: string, packageJson: Record<string, unknown>) => {
  const packageDirectory = join(directory, "node_modules", ...packageName.split("/"))
  await mkdir(packageDirectory, { recursive: true })
  await writeFile(join(packageDirectory, "package.json"), JSON.stringify(packageJson))
  return packageDirectory
}

const installVitePlus = (directory: string) => writePackage(directory, "vite-plus", { name: "vite-plus", version: "1.7.0" })

// A project with `tsrs` and its platform package installed (the binary plus one of the lib*.d.ts files next to it), at
// the root or, with `vitePlus`, as a dependency of vite-plus that does not resolve from the root.
const installTsrs = async (
  target: string,
  version: string,
  options: { readonly nested?: boolean; readonly vitePlus?: boolean; readonly directory?: string } = {}
) => {
  const directory = options.directory ?? await makeTemporaryDirectory()
  const scope = options.vitePlus ? await installVitePlus(directory) : directory
  const tsrsDirectory = await writePackage(scope, "tsrs", { name: "tsrs", version })
  const platformDirectory = await writePackage(options.nested ? tsrsDirectory : scope, `@tsrs/${target}`, {
    name: `@tsrs/${target}`,
    version
  })
  const binaryPath = join(platformDirectory, "tsrs")
  await writeFile(binaryPath, "stock tsrs")
  await writeFile(join(platformDirectory, "lib.d.ts"), "lib")
  return { directory, platformDirectory, binaryPath }
}

const hostTarget = tsrsTarget(process.platform, process.arch, isGlibc())
const hostReplacementAvailable = (tsrsReplacementTargets as ReadonlyArray<string | undefined>).includes(hostTarget)

// The patched tsrs artifact in this host's @effect/tsgo-<platform> package, written for the test unless a local build
// already provides one. Only the artifact and the directories above it are removed, and those only once empty, as other
// test files write their artifacts into the same package concurrently.
const providePackagedTsrs = async (version: string) => {
  const packageJson = createRequire(import.meta.url).resolve(
    `@effect/tsgo-${process.platform}-${process.arch}/package.json`,
    { paths: [join(import.meta.dirname, "..", "src", "patcher")] }
  )
  const binary = join(dirname(packageJson), "artifacts", "tsrs", version, "tsrs")
  if (existsSync(binary)) return { binary, contents: await readFile(binary, "utf8") }
  const artifacts = join(dirname(packageJson), "artifacts")
  await mkdir(dirname(binary), { recursive: true })
  await writeFile(binary, "patched tsrs")
  cleanups.push(async () => {
    await rm(binary, { force: true })
    for (const directory of [dirname(binary), join(artifacts, "tsrs"), artifacts]) {
      await rmdir(directory).catch(() => undefined)
    }
  })
  return { binary, contents: "patched tsrs" }
}

afterEach(async () => {
  vi.restoreAllMocks()
  for (const cleanup of cleanups.splice(0)) await cleanup()
  await Promise.all(temporaryDirectories.splice(0).map((directory) => rm(directory, { recursive: true, force: true })))
})

describe("tsrs component", () => {
  it("maps hosts to the platform packages tsrs publishes", () => {
    expect(tsrsTarget("darwin", "arm64", false)).toBe("darwin-arm64")
    expect(tsrsTarget("linux", "x64", true)).toBe("linux-x64-gnu")
    expect(tsrsTarget("linux", "x64", false)).toBe("linux-x64-musl")
    expect(tsrsTarget("linux", "arm64", true)).toBe("linux-arm64-gnu")
    expect(tsrsTarget("linux", "arm64", false)).toBe("linux-arm64-musl")
    expect(tsrsTarget("darwin", "x64", false)).toBe("darwin-x64")
    expect(tsrsTarget("win32", "x64", false)).toBe("win32-x64")
    expect(tsrsTarget("win32", "arm64", false)).toBe("win32-arm64")
    expect(tsrsTarget("linux", "arm", true)).toBeUndefined()
  })

  it.runIf(hostTarget !== undefined).each([false, true])(
    "discovers the installed tsrs binary of the platform package (nested: %s)",
    async (nested) => {
      const { binaryPath, directory } = await installTsrs(hostTarget!, "0.18.0-test", { nested })
      const discovered = await run(discoverBinaries(directory))
      expect(discovered).toEqual([{
        component: "tsrs",
        packageName: `@tsrs/${hostTarget}`,
        packageVersion: "0.18.0-test",
        binaryPath,
        fileHash: hash("stock tsrs")
      }])
      // The TypeScript integration does not see tsrs.
      expect(selectComponents(discovered, new Set(["typescript"]))).toEqual([])
    }
  )

  it.runIf(hostTarget !== undefined)("discovers the tsrs binary installed through vite-plus", async () => {
    const { binaryPath, directory } = await installTsrs(hostTarget!, "0.18.0-test", { vitePlus: true })
    expect(await run(discoverBinaries(directory))).toEqual([{
      component: "tsrs",
      packageName: `@tsrs/${hostTarget}`,
      packageVersion: "0.18.0-test",
      binaryPath,
      fileHash: hash("stock tsrs")
    }])
  })

  it.runIf(hostTarget !== undefined)("discovers the tsrs binaries of the project and of vite-plus, root first", async () => {
    const root = await installTsrs(hostTarget!, "0.18.0-test")
    const vitePlus = await installTsrs(hostTarget!, "0.19.0-test", { vitePlus: true, directory: root.directory })
    const discovered = await run(discoverBinaries(root.directory))
    expect(discovered.map(({ binaryPath, packageVersion }) => ({ binaryPath, packageVersion }))).toEqual([
      { binaryPath: root.binaryPath, packageVersion: "0.18.0-test" },
      { binaryPath: vitePlus.binaryPath, packageVersion: "0.19.0-test" }
    ])
  })

  it.runIf(hostTarget !== undefined)("discovers the tsrs binary once when vite-plus resolves the project's", async () => {
    const { binaryPath, directory } = await installTsrs(hostTarget!, "0.18.0-test")
    await installVitePlus(directory)
    expect((await run(discoverBinaries(directory))).map((binary) => binary.binaryPath)).toEqual([binaryPath])
  })

  it("discovers nothing without the tsrs package or its platform package", async () => {
    const directory = await makeTemporaryDirectory()
    expect(await run(discoverBinaries(directory))).toEqual([])
    await writePackage(directory, "tsrs", { name: "tsrs", version: "0.18.0-test" })
    const discovered = await run(discoverBinaries(directory))
    expect(discovered).toEqual([])
    await expect(run(requireComponents(discovered, new Set(["tsrs"])))).rejects.toThrow(
      "Unable to discover an installed tsrs binary. Install the tsrs package; its binary is published for "
        + "darwin-arm64, darwin-x64, linux-x64-gnu, linux-x64-musl, linux-arm64-gnu, linux-arm64-musl, win32-x64, win32-arm64."
    )
  })

  it("discovers nothing when vite-plus has no tsrs package", async () => {
    const directory = await makeTemporaryDirectory()
    await installVitePlus(directory)
    const discovered = await run(discoverBinaries(directory))
    expect(discovered).toEqual([])
    await expect(run(requireComponents(discovered, new Set(["tsrs"])))).rejects.toThrow(
      "Unable to discover an installed tsrs binary. Install the tsrs package; its binary is published for "
        + "darwin-arm64, darwin-x64, linux-x64-gnu, linux-x64-musl, linux-arm64-gnu, linux-arm64-musl, win32-x64, win32-arm64."
    )
  })

  it("locates the patched build by the installed tsrs version", async () => {
    const target: DiscoveredBinary = {
      component: "tsrs",
      packageName: "@tsrs/linux-x64-gnu",
      packageVersion: supportedTsrsVersion,
      binaryPath: "/project/node_modules/@tsrs/linux-x64-gnu/tsrs",
      fileHash: hash("stock tsrs")
    }
    expect(await run(packagedArtifactPath("/platform", target))).toBe(
      join("/platform", "artifacts", "tsrs", supportedTsrsVersion, "tsrs")
    )
  })

  it.runIf(hostReplacementAvailable)("skips an unsupported tsrs version with the supported versions", async () => {
    const { binaryPath, directory } = await installTsrs(hostTarget!, "0.0.0-unsupported")
    const [target] = await run(discoverBinaries(directory))
    await expect(run(resolveReplacement(target!))).rejects.toBeInstanceOf(UnsupportedTargetPackageVersionError)
    await expect(run(patch({ cwd: directory, components: new Set(["tsrs"]), skipMissing: false })))
      .rejects.toThrow(`Either change @effect/tsgo to a compatible version or set @tsrs/${hostTarget}`)

    const result = await run(patch({ cwd: directory, components: new Set(["tsrs"]), skipMissing: true }))
    expect(result.changed).toEqual([])
    expect(result.skipped).toHaveLength(1)
    expect(result.skipped[0]?.reason).toBe("replacement-unavailable")
    expect(result.skipped[0]?.message).toContain(
      `Package @tsrs/${hostTarget} version 0.0.0-unsupported is not supported by @effect/tsgo`
    )
    expect(result.skipped[0]?.message).toContain(supportedTsrsVersion)
    expect(await readFile(binaryPath, "utf8")).toBe("stock tsrs")
  })

  it.runIf(hostReplacementAvailable)("skips an unsupported tsrs version installed through vite-plus", async () => {
    const { binaryPath, directory } = await installTsrs(hostTarget!, "0.0.0-unsupported", { vitePlus: true })
    const result = await run(patch({ cwd: directory, components: new Set(["tsrs"]), skipMissing: true }))
    expect(result.changed).toEqual([])
    expect(result.skipped.map(({ reason }) => reason)).toEqual(["replacement-unavailable"])
    expect(result.skipped[0]?.message).toContain(`Package @tsrs/${hostTarget} version 0.0.0-unsupported is not supported`)
    expect(await readFile(binaryPath, "utf8")).toBe("stock tsrs")
  })

  it.skipIf(process.platform !== "linux" || (process.arch !== "x64" && process.arch !== "arm64"))(
    "discovers a musl install and skips it: the patched build is glibc only",
    async () => {
      vi.spyOn(process.report, "getReport").mockReturnValue({ header: {} } as ReturnType<typeof process.report.getReport>)
      const { binaryPath, directory } = await installTsrs(`linux-${process.arch}-musl`, supportedTsrsVersion)
      const [target] = await run(discoverBinaries(directory))
      expect(target?.packageName).toBe(`@tsrs/linux-${process.arch}-musl`)
      await expect(run(resolveReplacement(target!))).rejects.toMatchObject({
        _tag: "ReplacementUnavailableError",
        reason: "Linux musl is not supported by the packaged tsrs integration; it is built for "
          + "darwin-arm64, darwin-x64, linux-x64-gnu, linux-arm64-gnu, win32-x64, win32-arm64."
      })
      await expect(run(preparePatch([target!], { skipMissing: false }))).rejects.toBeInstanceOf(
        ReplacementUnavailableError
      )
      const result = await run(patch({ cwd: directory, components: new Set(["tsrs"]), skipMissing: true }))
      expect(result.changed).toEqual([])
      expect(result.skipped.map(({ reason }) => reason)).toEqual(["replacement-unavailable"])
      expect(await readFile(binaryPath, "utf8")).toBe("stock tsrs")
    }
  )

  it.runIf(hostReplacementAvailable)(
    "swaps the installed tsrs binary for the patched build of its version and restores it",
    async () => {
      const packaged = await providePackagedTsrs(supportedTsrsVersion)
      const { binaryPath, directory, platformDirectory } = await installTsrs(hostTarget!, supportedTsrsVersion)
      const components = new Set(["tsrs"] as const)

      const patched = await run(patch({ cwd: directory, components, skipMissing: false }))
      expect(patched.changed.map(({ binaryPath }) => binaryPath)).toEqual([binaryPath])
      expect(await readFile(binaryPath, "utf8")).toBe(packaged.contents)
      expect(await readFile(`${binaryPath}.original`, "utf8")).toBe("stock tsrs")
      expect((await stat(binaryPath)).mode & 0o111).not.toBe(0)
      // Only the binary is replaced; the lib*.d.ts files next to it stay tsrs's own.
      expect(await readFile(join(platformDirectory, "lib.d.ts"), "utf8")).toBe("lib")

      const again = await run(patch({ cwd: directory, components, skipMissing: false }))
      expect(again.changed).toEqual([])
      expect(again.skipped.map(({ reason }) => reason)).toEqual(["already-patched"])

      const restored = await run(unpatch({ cwd: directory, components }))
      expect(restored.changed.map(({ binaryPath }) => binaryPath)).toEqual([binaryPath])
      expect(await readFile(binaryPath, "utf8")).toBe("stock tsrs")
      expect(existsSync(`${binaryPath}.original`)).toBe(false)
    }
  )

  it.runIf(hostReplacementAvailable)(
    "swaps the tsrs binary installed through vite-plus for the patched build and restores it",
    async () => {
      const packaged = await providePackagedTsrs(supportedTsrsVersion)
      const { binaryPath, directory } = await installTsrs(hostTarget!, supportedTsrsVersion, { vitePlus: true })
      const components = new Set(["tsrs"] as const)

      const patched = await run(patch({ cwd: directory, components, skipMissing: false }))
      expect(patched.changed.map(({ binaryPath }) => binaryPath)).toEqual([binaryPath])
      expect(await readFile(binaryPath, "utf8")).toBe(packaged.contents)
      expect(await readFile(`${binaryPath}.original`, "utf8")).toBe("stock tsrs")

      const restored = await run(unpatch({ cwd: directory, components }))
      expect(restored.changed.map(({ binaryPath }) => binaryPath)).toEqual([binaryPath])
      expect(await readFile(binaryPath, "utf8")).toBe("stock tsrs")
      expect(existsSync(`${binaryPath}.original`)).toBe(false)
    }
  )
})
