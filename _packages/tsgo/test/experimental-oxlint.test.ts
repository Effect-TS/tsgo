import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import { createHash } from "node:crypto"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"
import { afterEach, describe, expect, it, vi } from "vitest"
import {
  discoverBinaries,
  experimentalOxlintTarget,
  preparePatch,
  ReplacementUnavailableError,
  resolveReplacement,
  selectComponents,
  unpatch
} from "../src/patcher/index.js"

const temporaryDirectories: Array<string> = []

const hash = (value: string) => createHash("sha256").update(value).digest("hex")

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

afterEach(async () => {
  vi.restoreAllMocks()
  await Promise.all(temporaryDirectories.splice(0).map((directory) => rm(directory, { recursive: true, force: true })))
})

describe("experimental Oxlint discovery", () => {
  it("maps supported platforms to installed package names", () => {
    expect(experimentalOxlintTarget("linux", "x64", true)).toEqual({
      codeTarget: "linux-x64-gnu",
      oxlintPackage: "@oxlint/binding-linux-x64-gnu",
      tsgolintPackage: "@oxlint-tsgolint/linux-x64",
      tsgolintExecutable: "tsgolint"
    })
    expect(experimentalOxlintTarget("win32", "arm64", false)).toEqual({
      codeTarget: "win32-arm64",
      oxlintPackage: "@oxlint/binding-win32-arm64-msvc",
      tsgolintPackage: "@oxlint-tsgolint/win32-arm64",
      tsgolintExecutable: "tsgolint.exe"
    })
    expect(experimentalOxlintTarget("linux", "x64", false)).toEqual({
      codeTarget: "linux-x64-musl",
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

      const discovered = await Effect.runPromise(discoverBinaries(directory).pipe(Effect.provide(NodeServices.layer)))
      const typescript = selectComponents(discovered, new Set(["typescript"]))
      expect(typescript).toHaveLength(1)
      expect(typescript[0]?.binaryPath).toBe(typescriptPath)
      const oxlint = selectComponents(discovered, new Set(["oxlint", "oxlint-dts", "oxlint-tsgolint"]))
      expect(oxlint).toHaveLength(3)
      expect(oxlint[0]?.packageName).toBe(`@oxlint/binding-linux-${process.arch}-musl`)
      for (const target of oxlint) {
        await expect(Effect.runPromise(
          Effect.scoped(resolveReplacement(target)).pipe(Effect.provide(NodeServices.layer))
        )).rejects.toMatchObject({
          _tag: "ReplacementUnavailableError",
          reason: "Linux musl is not supported by the packaged Oxlint integration."
        })
      }
      await expect(Effect.runPromise(
        Effect.scoped(preparePatch(oxlint, { skipMissing: false })).pipe(Effect.provide(NodeServices.layer))
      )).rejects.toBeInstanceOf(ReplacementUnavailableError)
      const skipped = await Effect.runPromise(
        Effect.scoped(preparePatch(oxlint, { skipMissing: true })).pipe(Effect.provide(NodeServices.layer))
      )
      expect(skipped.operations).toEqual([])
      expect(skipped.skipped).toHaveLength(3)

      await Promise.all(oxlint.map((target) => writeBinary(`${target.binaryPath}.original`, "original")))
      const restored = await Effect.runPromise(unpatch({
        cwd: directory,
        components: new Set(["oxlint", "oxlint-tsgolint"])
      }).pipe(Effect.provide(NodeServices.layer)))
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
      discoverBinaries(directory).pipe(Effect.provide(NodeServices.layer))
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
      discoverBinaries(directory).pipe(Effect.provide(NodeServices.layer))
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
      discoverBinaries(directory).pipe(Effect.provide(NodeServices.layer))
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
