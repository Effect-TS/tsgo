import * as NodeCrypto from "@effect/platform-node/NodeCrypto"
import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Digest from "effect/crypto/Digest"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import * as Scope from "effect/Scope"
import { createHash } from "node:crypto"
import { existsSync } from "node:fs"
import { mkdir, mkdtemp, readFile, realpath, rm, rmdir, stat, writeFile } from "node:fs/promises"
import { createRequire } from "node:module"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"
import { afterEach, describe, expect, it, vi } from "vitest"
import upstreamJson from "../upstream.json" with { type: "json" }
import {
  type Component,
  discoverBinaries,
  experimentalOxlintTarget,
  isGlibc,
  patch,
  preparePatch,
  renderOxlintDeclarations,
  ReplacementUnavailableError,
  resolveReplacement,
  selectComponents,
  unpatch,
  UnsupportedTargetPackageVersionError
} from "../src/patcher/index.js"

const temporaryDirectories: Array<string> = []
const cleanups: Array<() => Promise<void>> = []

const hash = (value: string) => createHash("sha256").update(value).digest("hex")
const supportedOxlintVersion = Object.keys(upstreamJson.components.oxlint).sort().at(-1)!
const supportedTsgolintVersion = Object.keys(upstreamJson.components["oxlint-tsgolint"]).sort().at(-1)!

const makeTemporaryDirectory = async () => {
  const directory = await mkdtemp(join(tmpdir(), "effect-tsgo-oxlint-"))
  temporaryDirectories.push(directory)
  return directory
}

const writePackage = async (directory: string, packageName: string, packageJson: Record<string, unknown>) => {
  const packageDirectory = join(directory, "node_modules", ...packageName.split("/"))
  await mkdir(packageDirectory, { recursive: true })
  await writeFile(join(packageDirectory, "package.json"), JSON.stringify(packageJson))
  return packageDirectory
}

const writeBinary = async (filePath: string, contents: string) => {
  await mkdir(dirname(filePath), { recursive: true })
  await writeFile(filePath, contents)
}

const hostTarget = (() => {
  try {
    return experimentalOxlintTarget(process.platform, process.arch, isGlibc())
  } catch {
    return undefined
  }
})()
const hostReplacementAvailable = hostTarget !== undefined && (process.platform !== "linux" || isGlibc())

const stockDeclarations = [
  "type LintPluginOptionsSchema = \"react\" | \"unicorn\";",
  "interface DummyRuleMap {",
  "  \"eslint/no-debugger\"?: RuleNoConfig;",
  "}",
  ""
].join("\n")

// A project with oxlint, its host binding and its declarations installed, directly or as dependencies of vite-plus, and
// oxlint-tsgolint with its host package where a version is given.
const installOxlint = async (
  options: { readonly nested?: boolean; readonly oxlint?: boolean; readonly tsgolintVersion?: string } = {}
) => {
  const directory = await realpath(await makeTemporaryDirectory())
  const root = options.nested ? await writePackage(directory, "vite-plus", { version: "1.7.0" }) : directory
  const modules = join(root, "node_modules")
  const binaryPath = join(modules, ...hostTarget!.oxlintPackage.split("/"), hostTarget!.oxlintBinding)
  const declarationsPath = join(modules, "oxlint", "dist", "index.d.ts")
  const tsgolintPath = join(modules, ...hostTarget!.tsgolintPackage.split("/"), hostTarget!.tsgolintExecutable)
  if (options.oxlint ?? true) {
    await writePackage(root, "oxlint", { version: supportedOxlintVersion })
    await writePackage(root, hostTarget!.oxlintPackage, {
      version: supportedOxlintVersion,
      main: hostTarget!.oxlintBinding
    })
    await writeBinary(binaryPath, "stock oxlint")
    await writeBinary(declarationsPath, stockDeclarations)
  }
  if (options.tsgolintVersion !== undefined) {
    await writePackage(root, "oxlint-tsgolint", { version: options.tsgolintVersion })
    await writePackage(root, hostTarget!.tsgolintPackage, { version: options.tsgolintVersion })
    await writeBinary(tsgolintPath, "stock tsgolint")
  }
  return { directory, binaryPath, declarationsPath, tsgolintPath }
}

// A packaged artifact in this host's @effect/tsgo-<platform> package, written for the test unless a local build already
// provides one. Only the artifact and the directories above it are removed, and those only once empty, as other test
// files write their artifacts into the same package concurrently.
const providePackagedArtifact = async (component: Component, version: string, fileName: string) => {
  const packageJson = createRequire(import.meta.url).resolve(
    `@effect/tsgo-${process.platform}-${process.arch}/package.json`,
    { paths: [join(import.meta.dirname, "..", "src", "patcher")] }
  )
  const artifacts = join(dirname(packageJson), "artifacts")
  const artifact = join(artifacts, component, version, fileName)
  if (existsSync(artifact)) return readFile(artifact, "utf8")
  await writeBinary(artifact, `patched ${component}`)
  cleanups.push(async () => {
    await rm(artifact, { force: true })
    for (const directory of [dirname(artifact), join(artifacts, component), artifacts]) {
      await rmdir(directory).catch(() => undefined)
    }
  })
  return `patched ${component}`
}

const run = <A, E>(effect: Effect.Effect<A, E, Digest.Digest | FileSystem.FileSystem | Path.Path | Scope.Scope>) =>
  Effect.runPromise(Effect.scoped(effect).pipe(Effect.provide([NodeServices.layer, NodeCrypto.layerDigest])))

afterEach(async () => {
  vi.restoreAllMocks()
  for (const cleanup of cleanups.splice(0).reverse()) await cleanup()
  await Promise.all(temporaryDirectories.splice(0).map((directory) => rm(directory, { recursive: true, force: true })))
})

describe("experimental Oxlint discovery", () => {
  it("maps supported platforms to installed package names", () => {
    expect(experimentalOxlintTarget("linux", "x64", true)).toEqual({
      codeTarget: "linux-x64-gnu",
      oxlintBinding: "oxlint.linux-x64-gnu.node",
      oxlintPackage: "@oxlint/binding-linux-x64-gnu",
      tsgolintPackage: "@oxlint-tsgolint/linux-x64",
      tsgolintExecutable: "tsgolint"
    })
    expect(experimentalOxlintTarget("win32", "arm64", false)).toEqual({
      codeTarget: "win32-arm64",
      oxlintBinding: "oxlint.win32-arm64-msvc.node",
      oxlintPackage: "@oxlint/binding-win32-arm64-msvc",
      tsgolintPackage: "@oxlint-tsgolint/win32-arm64",
      tsgolintExecutable: "tsgolint.exe"
    })
    expect(experimentalOxlintTarget("linux", "x64", false)).toEqual({
      codeTarget: "linux-x64-musl",
      oxlintBinding: "oxlint.linux-x64-musl.node",
      oxlintPackage: "@oxlint/binding-linux-x64-musl",
      tsgolintPackage: "@oxlint-tsgolint/linux-x64",
      tsgolintExecutable: "tsgolint"
    })
    expect(() => experimentalOxlintTarget("linux", "arm", true)).toThrow(/Unsupported/)
  })

  it.skipIf(process.platform !== "linux").each([false, true])(
    "discovers musl binaries and rejects only their replacements (vite-plus: %s)",
    async (nested) => {
      vi.spyOn(process.report, "getReport").mockReturnValue({ header: {} } as ReturnType<typeof process.report.getReport>)
      const directory = await makeTemporaryDirectory()
      await writePackage(directory, "typescript", { version: "7.0.0" })
      const typescriptDirectory = await writePackage(directory, `@typescript/typescript-linux-${process.arch}`, {
        version: "7.0.0"
      })
      const typescriptPath = join(typescriptDirectory, "lib", "tsc")
      await writeBinary(typescriptPath, "typescript")

      const oxlintRoot = nested ? await writePackage(directory, "vite-plus", { version: "1.0.0" }) : directory
      const oxlintDirectory = await writePackage(oxlintRoot, "oxlint", { version: "1.0.0" })
      await writePackage(oxlintRoot, "oxlint-tsgolint", { version: "2.0.0" })
      const bindingDirectory = await writePackage(oxlintRoot, `@oxlint/binding-linux-${process.arch}-musl`, {
        version: "1.0.0",
        main: "oxlint.node"
      })
      const tsgolintDirectory = await writePackage(oxlintRoot, `@oxlint-tsgolint/linux-${process.arch}`, {
        version: "2.0.0"
      })
      await Promise.all([
        writeBinary(join(bindingDirectory, "oxlint.node"), "oxlint"),
        writeBinary(join(oxlintDirectory, "dist", "index.d.ts"), "declarations"),
        writeBinary(join(tsgolintDirectory, "tsgolint"), "tsgolint")
      ])

      const discovered = await Effect.runPromise(discoverBinaries(directory).pipe(Effect.provide([NodeServices.layer, NodeCrypto.layerDigest])))
      const typescript = selectComponents(discovered, new Set(["typescript"]))
      expect(typescript).toHaveLength(1)
      expect(typescript[0]?.binaryPath).toBe(typescriptPath)
      const oxlint = selectComponents(discovered, new Set(["oxlint", "oxlint-dts", "oxlint-tsgolint"]))
      expect(oxlint).toHaveLength(3)
      expect(oxlint[0]?.packageName).toBe(`@oxlint/binding-linux-${process.arch}-musl`)
      for (const target of oxlint) {
        await expect(Effect.runPromise(
          Effect.scoped(resolveReplacement(target)).pipe(Effect.provide([NodeServices.layer, NodeCrypto.layerDigest]))
        )).rejects.toMatchObject({
          _tag: "ReplacementUnavailableError",
          reason: "Linux musl is not supported by the packaged Oxlint integration."
        })
      }
      await expect(Effect.runPromise(
        Effect.scoped(preparePatch(oxlint, { skipMissing: false })).pipe(Effect.provide([NodeServices.layer, NodeCrypto.layerDigest]))
      )).rejects.toBeInstanceOf(ReplacementUnavailableError)
      const skipped = await Effect.runPromise(
        Effect.scoped(preparePatch(oxlint, { skipMissing: true })).pipe(Effect.provide([NodeServices.layer, NodeCrypto.layerDigest]))
      )
      expect(skipped.operations).toEqual([])
      expect(skipped.skipped).toHaveLength(3)

      await Promise.all(oxlint.map((target) => writeBinary(`${target.binaryPath}.original`, "original")))
      const restored = await Effect.runPromise(unpatch({
        cwd: directory,
        components: new Set(["oxlint", "oxlint-tsgolint"])
      }).pipe(Effect.provide([NodeServices.layer, NodeCrypto.layerDigest])))
      expect(restored.changed).toHaveLength(3)
      expect(restored.skipped).toEqual([])
    }
  )

  it("returns normalized binaries without resolving Effect artifacts", async () => {
    const directory = await makeTemporaryDirectory()
    const platform = experimentalOxlintTarget(process.platform, process.arch, true)
    const oxlintDirectory = await writePackage(directory, "oxlint", { version: "1.0.0" })
    await writePackage(directory, "oxlint-tsgolint", { version: "2.0.0" })
    const bindingDirectory = await writePackage(directory, platform.oxlintPackage, {
      version: "1.0.1",
      main: "oxlint.node"
    })
    const tsgolintDirectory = await writePackage(directory, platform.tsgolintPackage, { version: "2.0.1" })
    await Promise.all([
      writeBinary(join(bindingDirectory, "oxlint.node"), "oxlint"),
      writeBinary(join(oxlintDirectory, "dist", "index.d.ts"), "declarations"),
      writeBinary(join(tsgolintDirectory, platform.tsgolintExecutable), "tsgolint")
    ])

    const discovered = await Effect.runPromise(
      discoverBinaries(directory).pipe(Effect.provide([NodeServices.layer, NodeCrypto.layerDigest]))
    )
    expect(discovered).toEqual([
      {
        component: "oxlint",
        packageName: platform.oxlintPackage,
        packageVersion: "1.0.1",
        binaryPath: join(bindingDirectory, "oxlint.node"),
        fileHash: hash("oxlint")
      },
      {
        component: "oxlint-dts",
        packageName: "oxlint",
        packageVersion: "1.0.0",
        binaryPath: join(oxlintDirectory, "dist", "index.d.ts"),
        fileHash: hash("declarations")
      },
      {
        component: "oxlint-tsgolint",
        packageName: platform.tsgolintPackage,
        packageVersion: "2.0.1",
        binaryPath: join(tsgolintDirectory, platform.tsgolintExecutable),
        fileHash: hash("tsgolint")
      }
    ])
  })

  it("uses the package containing the TypeScript binary as its identity", async () => {
    const directory = await makeTemporaryDirectory()
    await writePackage(directory, "typescript", { version: "7.0.0" })
    const platformPackage = `@typescript/typescript-${process.platform}-${process.arch}`
    const platformDirectory = await writePackage(directory, platformPackage, { version: "7.0.1" })
    const binaryPath = join(platformDirectory, "lib", process.platform === "win32" ? "tsc.exe" : "tsc")
    await writeBinary(binaryPath, "typescript")

    const discovered = await Effect.runPromise(
      discoverBinaries(directory).pipe(Effect.provide([NodeServices.layer, NodeCrypto.layerDigest]))
    )
    expect(discovered).toContainEqual({
      component: "typescript",
      packageName: platformPackage,
      packageVersion: "7.0.1",
      binaryPath,
      fileHash: hash("typescript")
    })
  })

  it("discovers Oxlint binaries installed as dependencies of vite-plus", async () => {
    const directory = await makeTemporaryDirectory()
    const platform = experimentalOxlintTarget(process.platform, process.arch, true)
    const vitePlusDirectory = await writePackage(directory, "vite-plus", { version: "1.0.0" })
    const oxlintDirectory = await writePackage(vitePlusDirectory, "oxlint", { version: "1.0.0" })
    await writePackage(vitePlusDirectory, "oxlint-tsgolint", { version: "2.0.0" })
    const bindingDirectory = await writePackage(vitePlusDirectory, platform.oxlintPackage, {
      version: "1.0.1",
      main: "oxlint.node"
    })
    const tsgolintDirectory = await writePackage(
      vitePlusDirectory,
      platform.tsgolintPackage,
      { version: "2.0.1" }
    )
    await Promise.all([
      writeBinary(join(bindingDirectory, "oxlint.node"), "oxlint"),
      writeBinary(join(oxlintDirectory, "dist", "index.d.ts"), "declarations"),
      writeBinary(join(tsgolintDirectory, platform.tsgolintExecutable), "tsgolint")
    ])

    const discovered = await Effect.runPromise(
      discoverBinaries(directory).pipe(Effect.provide([NodeServices.layer, NodeCrypto.layerDigest]))
    )
    expect(discovered).toEqual([
      {
        component: "oxlint",
        packageName: platform.oxlintPackage,
        packageVersion: "1.0.1",
        binaryPath: join(bindingDirectory, "oxlint.node"),
        fileHash: hash("oxlint")
      },
      {
        component: "oxlint-dts",
        packageName: "oxlint",
        packageVersion: "1.0.0",
        binaryPath: join(oxlintDirectory, "dist", "index.d.ts"),
        fileHash: hash("declarations")
      },
      {
        component: "oxlint-tsgolint",
        packageName: platform.tsgolintPackage,
        packageVersion: "2.0.1",
        binaryPath: join(tsgolintDirectory, platform.tsgolintExecutable),
        fileHash: hash("tsgolint")
      }
    ])
  })
})

describe("experimental Oxlint integration", () => {
  const changedPaths = (result: { readonly changed: ReadonlyArray<{ readonly binaryPath: string }> }) =>
    result.changed.map(({ binaryPath }) => binaryPath)

  it.runIf(hostReplacementAvailable).each([
    { nested: false, skipMissing: false },
    { nested: true, skipMissing: false },
    { nested: false, skipMissing: true }
  ])(
    "patches the binding and declarations without oxlint-tsgolint (vite-plus: $nested, skipMissing: $skipMissing)",
    async ({ nested, skipMissing }) => {
      const packaged = await providePackagedArtifact("oxlint", supportedOxlintVersion, hostTarget!.oxlintBinding)
      const { binaryPath, declarationsPath, directory } = await installOxlint({ nested })

      const result = await run(patch({ cwd: directory, components: new Set(["oxlint"]), skipMissing }))
      expect(changedPaths(result)).toEqual([binaryPath, declarationsPath])
      expect(result.skipped).toEqual([])
      expect(await readFile(binaryPath, "utf8")).toBe(packaged)
      const declarations = await readFile(declarationsPath, "utf8")
      expect(declarations).toBe(renderOxlintDeclarations(stockDeclarations))
      expect(declarations).toContain("type LintPluginOptionsSchema = \"effecttsgo\" | ")
      expect(await readFile(`${binaryPath}.original`, "utf8")).toBe("stock oxlint")
      expect(await readFile(`${declarationsPath}.original`, "utf8")).toBe(stockDeclarations)
    }
  )

  it.runIf(hostReplacementAvailable)("restores the binding and declarations without oxlint-tsgolint", async () => {
    await providePackagedArtifact("oxlint", supportedOxlintVersion, hostTarget!.oxlintBinding)
    const { binaryPath, declarationsPath, directory } = await installOxlint()
    const components = new Set(["oxlint"] as const)
    await run(patch({ cwd: directory, components, skipMissing: false }))

    const restored = await run(unpatch({ cwd: directory, components }))
    expect(changedPaths(restored)).toEqual([binaryPath, declarationsPath])
    expect(restored.skipped).toEqual([])
    expect(await readFile(binaryPath, "utf8")).toBe("stock oxlint")
    expect(await readFile(declarationsPath, "utf8")).toBe(stockDeclarations)
    expect(existsSync(`${binaryPath}.original`)).toBe(false)
    expect(existsSync(`${declarationsPath}.original`)).toBe(false)
  })

  it.runIf(hostReplacementAvailable)("patches and restores an installed oxlint-tsgolint along with Oxlint", async () => {
    const packagedOxlint = await providePackagedArtifact("oxlint", supportedOxlintVersion, hostTarget!.oxlintBinding)
    const packagedTsgolint = await providePackagedArtifact(
      "oxlint-tsgolint",
      supportedTsgolintVersion,
      hostTarget!.tsgolintExecutable
    )
    const { binaryPath, declarationsPath, directory, tsgolintPath } = await installOxlint({
      tsgolintVersion: supportedTsgolintVersion
    })
    const components = new Set(["oxlint"] as const)

    const patched = await run(patch({ cwd: directory, components, skipMissing: false }))
    expect(changedPaths(patched)).toEqual([binaryPath, declarationsPath, tsgolintPath])
    expect(patched.skipped).toEqual([])
    expect(await readFile(binaryPath, "utf8")).toBe(packagedOxlint)
    expect(await readFile(declarationsPath, "utf8")).toBe(renderOxlintDeclarations(stockDeclarations))
    expect(await readFile(tsgolintPath, "utf8")).toBe(packagedTsgolint)
    expect((await stat(tsgolintPath)).mode & 0o111).not.toBe(0)

    const restored = await run(unpatch({ cwd: directory, components }))
    expect(changedPaths(restored)).toEqual([binaryPath, declarationsPath, tsgolintPath])
    expect(restored.skipped).toEqual([])
    expect(await readFile(binaryPath, "utf8")).toBe("stock oxlint")
    expect(await readFile(declarationsPath, "utf8")).toBe(stockDeclarations)
    expect(await readFile(tsgolintPath, "utf8")).toBe("stock tsgolint")
    for (const path of [binaryPath, declarationsPath, tsgolintPath]) expect(existsSync(`${path}.original`)).toBe(false)
  })

  it.runIf(hostTarget !== undefined).each([undefined, supportedTsgolintVersion])(
    "requires an installed oxlint (oxlint-tsgolint: %s)",
    async (tsgolintVersion) => {
      const { directory } = await installOxlint({ oxlint: false, tsgolintVersion })
      await expect(run(patch({ cwd: directory, components: new Set(["oxlint"]), skipMissing: false })))
        .rejects.toMatchObject({ _tag: "DiscoveryError", reason: "Unable to discover an installed oxlint binary." })
    }
  )

  it.runIf(hostReplacementAvailable)("rejects or skips an installed oxlint-tsgolint of an unsupported version", async () => {
    const packaged = await providePackagedArtifact("oxlint", supportedOxlintVersion, hostTarget!.oxlintBinding)
    const { binaryPath, declarationsPath, directory, tsgolintPath } = await installOxlint({
      tsgolintVersion: "0.0.0-unsupported"
    })
    const components = new Set(["oxlint"] as const)
    await expect(run(patch({ cwd: directory, components, skipMissing: false })))
      .rejects.toBeInstanceOf(UnsupportedTargetPackageVersionError)
    expect(await readFile(binaryPath, "utf8")).toBe("stock oxlint")

    const result = await run(patch({ cwd: directory, components, skipMissing: true }))
    expect(changedPaths(result)).toEqual([binaryPath, declarationsPath])
    expect(result.skipped.map(({ reason, target }) => [target.component, reason]))
      .toEqual([["oxlint-tsgolint", "replacement-unavailable"]])
    expect(await readFile(binaryPath, "utf8")).toBe(packaged)
    expect(await readFile(declarationsPath, "utf8")).toBe(renderOxlintDeclarations(stockDeclarations))
    expect(await readFile(tsgolintPath, "utf8")).toBe("stock tsgolint")
  })

  it.runIf(hostTarget !== undefined)("requires oxlint-tsgolint when it is selected explicitly", async () => {
    const { directory } = await installOxlint()
    const components = new Set(["oxlint", "oxlint-tsgolint"] as const)
    const missing = { _tag: "DiscoveryError", reason: "Unable to discover an installed oxlint-tsgolint binary." }
    await expect(run(patch({ cwd: directory, components, skipMissing: false }))).rejects.toMatchObject(missing)
    await expect(run(unpatch({ cwd: directory, components }))).rejects.toMatchObject(missing)
  })
})
