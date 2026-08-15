import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Crypto from "effect/Crypto"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import * as Scope from "effect/Scope"
import { createHash } from "node:crypto"
import { access, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, it } from "vitest"
import * as pkgJson from "../package.json" with { type: "json" }
import {
  applyPlan,
  type DiscoveredBinary,
  PatcherError,
  preparePatch,
  prepareUnpatch,
  renderOxlintDeclarations,
  ReplacementUnavailableError,
  resolveReplacement,
  UnsupportedTargetPackageVersionError
} from "../src/patcher/index.js"

const temporaryDirectories: Array<string> = []

const hash = (value: string) => createHash("sha256").update(value).digest("hex")

const makeTemporaryDirectory = async () => {
  const directory = await mkdtemp(join(tmpdir(), "effect-tsgo-patcher-"))
  temporaryDirectories.push(directory)
  return directory
}

const run = <A, E>(effect: Effect.Effect<A, E, Crypto.Crypto | FileSystem.FileSystem | Path.Path | Scope.Scope>) =>
  Effect.runPromise(Effect.scoped(effect).pipe(Effect.provide(NodeServices.layer)))

afterEach(async () => {
  await Promise.all(temporaryDirectories.splice(0).map((directory) => rm(directory, { recursive: true, force: true })))
})

describe("patcher", () => {
  it("prepares ordered rename and copy operations for every target", async () => {
    const directory = await makeTemporaryDirectory()
    const first = join(directory, "first.d.ts")
    const second = join(directory, "second")
    const firstReplacement = join(directory, "first-effect")
    const secondReplacement = join(directory, "second-effect")
    await Promise.all([
      writeFile(first, "first"),
      writeFile(second, "second"),
      writeFile(firstReplacement, "first-effect"),
      writeFile(secondReplacement, "second-effect")
    ])
    const targets: ReadonlyArray<DiscoveredBinary> = [
      {
        component: "oxlint-dts",
        packageName: "oxlint",
        packageVersion: "1",
        binaryPath: first,
        fileHash: hash("first")
      },
      {
        component: "oxlint-tsgolint",
        packageName: "oxlint-tsgolint",
        packageVersion: "2",
        binaryPath: second,
        fileHash: hash("second")
      }
    ]

    const plan = await run(preparePatch(targets, {
      skipMissing: false,
      resolveReplacement: (target) => Effect.succeed({
        path: target.component === "oxlint-dts" ? firstReplacement : secondReplacement,
        fileHash: hash(target.component === "oxlint-dts" ? "first-effect" : "second-effect")
      })
    }))
    expect(plan.operations).toEqual([
      { _tag: "Rename", sourcePath: first, destinationPath: `${first}.original` },
      { _tag: "Copy", sourcePath: firstReplacement, destinationPath: first },
      { _tag: "Rename", sourcePath: second, destinationPath: `${second}.original` },
      { _tag: "Copy", sourcePath: secondReplacement, destinationPath: second },
      { _tag: "Chmod", path: second, mode: 0o755 }
    ])
  })

  it("generates an Oxlint declaration replacement in the temporary directory", async () => {
    const directory = await makeTemporaryDirectory()
    const declarationPath = join(directory, "index.d.ts")
    const declarations = [
      'type LintPluginOptionsSchema = "eslint" | "typescript";',
      "type RuleNoConfig = unknown;",
      "interface DummyRuleMap {",
      '  "eslint/no-unused-vars"?: RuleNoConfig;',
      "}",
      ""
    ].join("\n")
    await writeFile(declarationPath, declarations)
    const target: DiscoveredBinary = {
      component: "oxlint-dts",
      packageName: "oxlint",
      packageVersion: "1.0.0",
      binaryPath: declarationPath,
      fileHash: hash(await readFile(declarationPath, "utf8"))
    }

    const replacement = await run(Effect.gen(function*() {
      const fs = yield* FileSystem.FileSystem
      const resolved = yield* resolveReplacement(target)
      return { path: resolved.path, source: yield* fs.readFileString(resolved.path) }
    }))
    expect(replacement.path).not.toBe(declarationPath)
    expect(replacement.source).toContain(
      'type LintPluginOptionsSchema = "effecttsgo" | "eslint" | "typescript";'
    )
    expect(replacement.source).toContain('"effecttsgo/floating-effect"?: RuleNoConfig;')
    expect(replacement.source).toContain('"eslint/no-unused-vars"?: RuleNoConfig;')
    await expect(access(replacement.path)).rejects.toThrow()

    await writeFile(`${declarationPath}.original`, declarations)
    await writeFile(declarationPath, replacement.source)
    const refreshed = await run(Effect.gen(function*() {
      const fs = yield* FileSystem.FileSystem
      const resolved = yield* resolveReplacement(target)
      return yield* fs.readFileString(resolved.path)
    }))
    expect(refreshed).toBe(replacement.source)
  })

  it("rejects declarations whose expected anchors changed", () => {
    expect(() => renderOxlintDeclarations("interface OtherRuleMap {}\n")).toThrow(
      "Unable to locate the Oxlint plugin declaration."
    )
  })

  it("explains how to resolve an unsupported package version", () => {
    const target: DiscoveredBinary = {
      component: "typescript",
      packageName: "@typescript/typescript-linux-x64",
      packageVersion: "7.2.0",
      binaryPath: "/typescript/lib/tsc",
      fileHash: hash("typescript")
    }
    const error = new UnsupportedTargetPackageVersionError({
      target,
      supportedVersions: ["7.0.2", "7.1.0-dev.20260812.1"]
    })
    expect(error._tag).toBe("UnsupportedTargetPackageVersionError")
    expect(error.message).toBe(
      `Package @typescript/typescript-linux-x64 version 7.2.0 is not supported by @effect/tsgo version ${pkgJson.version}. `
      + "Either change @effect/tsgo to a compatible version or set @typescript/typescript-linux-x64 to one of the "
      + "supported versions: 7.0.2, 7.1.0-dev.20260812.1."
    )
  })

  it("rejects unsupported target package versions with a distinct error", async () => {
    const target: DiscoveredBinary = {
      component: "typescript",
      packageName: "@typescript/typescript-linux-x64",
      packageVersion: "unsupported",
      binaryPath: "/typescript/lib/tsc",
      fileHash: hash("typescript")
    }
    await expect(run(resolveReplacement(target))).rejects.toBeInstanceOf(UnsupportedTargetPackageVersionError)
  })

  it("preflights the whole plan without mutation", async () => {
    const directory = await makeTemporaryDirectory()
    const target = join(directory, "target")
    await writeFile(target, "original")
    await expect(run(applyPlan({
      operations: [
        { _tag: "Rename", sourcePath: target, destinationPath: `${target}.original` },
        { _tag: "Copy", sourcePath: join(directory, "missing"), destinationPath: target }
      ],
      cleanup: [],
      skipped: []
    }))).rejects.toThrow(/source does not exist/)
    expect(await readFile(target, "utf8")).toBe("original")
    await expect(access(`${target}.original`)).rejects.toThrow()
  })

  it("refreshes a stale patch while preserving the original backup", async () => {
    const directory = await makeTemporaryDirectory()
    const targetPath = join(directory, "binary")
    const replacementPath = join(directory, "replacement")
    await writeFile(targetPath, "patched")
    await writeFile(`${targetPath}.original`, "original")
    await writeFile(replacementPath, "replacement")
    const target: DiscoveredBinary = {
      component: "oxlint-tsgolint",
      packageName: "oxlint-tsgolint",
      packageVersion: "1",
      binaryPath: targetPath,
      fileHash: hash("patched")
    }
    const plan = await run(preparePatch([target], {
      skipMissing: false,
      resolveReplacement: () => Effect.succeed({ path: replacementPath, fileHash: hash("replacement") })
    }))
    expect(plan.operations.map(({ _tag }) => _tag)).toEqual(["Rename", "Copy", "Chmod"])
    expect(plan.cleanup).toHaveLength(1)
    expect(plan.skipped).toEqual([])
    await run(applyPlan(plan))
    expect(await readFile(targetPath, "utf8")).toBe("replacement")
    expect(await readFile(`${targetPath}.original`, "utf8")).toBe("original")
    expect((await readdir(directory)).filter((name) => name.endsWith(".patched"))).toEqual([])
  })

  it("skips a patch whose target already matches the replacement", async () => {
    const directory = await makeTemporaryDirectory()
    const targetPath = join(directory, "binary")
    const replacementPath = join(directory, "replacement")
    await writeFile(targetPath, "replacement")
    await writeFile(`${targetPath}.original`, "original")
    await writeFile(replacementPath, "replacement")
    const target: DiscoveredBinary = {
      component: "oxlint-tsgolint",
      packageName: "oxlint-tsgolint",
      packageVersion: "1",
      binaryPath: targetPath,
      fileHash: hash("replacement")
    }
    const plan = await run(preparePatch([target], {
      skipMissing: false,
      resolveReplacement: () => Effect.succeed({ path: replacementPath, fileHash: hash("replacement") })
    }))
    expect(plan.operations).toEqual([])
    expect(plan.skipped[0]?.reason).toBe("already-patched")
    expect(plan.skipped[0]?.message).toBe(
      `oxlint-tsgolint skipped because its hash matches the replacement.`
    )
  })

  it("rolls back prior reversible operations when mutation fails", async () => {
    const directory = await makeTemporaryDirectory()
    const target = join(directory, "target")
    const invalidReplacement = join(directory, "replacement-directory")
    await writeFile(target, "original")
    await mkdir(invalidReplacement)
    await expect(run(applyPlan({
      operations: [
        { _tag: "Rename", sourcePath: target, destinationPath: `${target}.original` },
        { _tag: "Copy", sourcePath: invalidReplacement, destinationPath: target }
      ],
      cleanup: [],
      skipped: []
    }))).rejects.toThrow(/Filesystem transaction failed/)
    expect(await readFile(target, "utf8")).toBe("original")
    await expect(access(`${target}.original`)).rejects.toThrow()
  })

  it("prepares and applies unpatch without resolving packaged artifacts", async () => {
    const directory = await makeTemporaryDirectory()
    const targetPath = join(directory, "binary")
    const target: DiscoveredBinary = {
      component: "typescript",
      packageName: "typescript",
      packageVersion: "7.0.0",
      binaryPath: targetPath,
      fileHash: hash("patched")
    }
    await writeFile(targetPath, "patched")
    await writeFile(`${targetPath}.original`, "original")
    const plan = await run(prepareUnpatch([target]))
    expect(plan.operations.map(({ _tag }) => _tag)).toEqual(["Rename", "Rename"])
    expect(plan.cleanup).toHaveLength(1)
    await run(applyPlan(plan))
    expect(await readFile(targetPath, "utf8")).toBe("original")
    expect((await readdir(directory)).filter((name) => name.endsWith(".patched"))).toEqual([])
  })

  it("skips only unavailable replacements when skipMissing is enabled", async () => {
    const directory = await makeTemporaryDirectory()
    const targetPath = join(directory, "binary")
    await writeFile(targetPath, "original")
    const target: DiscoveredBinary = {
      component: "oxlint",
      packageName: "oxlint",
      packageVersion: "missing",
      binaryPath: targetPath,
      fileHash: hash("original")
    }
    const unavailable = () => Effect.fail(new ReplacementUnavailableError({ target, reason: "not packaged" }))
    const plan = await run(preparePatch([target], {
      skipMissing: true,
      resolveReplacement: unavailable
    }))
    expect(plan.operations).toEqual([])
    expect(plan.skipped).toEqual([{ target, reason: "replacement-unavailable", message: "not packaged" }])
    await run(applyPlan(plan))
    expect(await readFile(targetPath, "utf8")).toBe("original")

    await expect(run(preparePatch([target], {
      skipMissing: false,
      resolveReplacement: unavailable
    }))).rejects.toThrow("not packaged")

    const unsupported = () => Effect.fail(new UnsupportedTargetPackageVersionError({
      target,
      supportedVersions: ["1", "2"]
    }))
    const unsupportedPlan = await run(preparePatch([target], {
      skipMissing: true,
      resolveReplacement: unsupported
    }))
    expect(unsupportedPlan.skipped[0]?.message).toContain("is not supported by @effect/tsgo")

    await expect(run(preparePatch([{ ...target, binaryPath: join(directory, "absent") }], {
      skipMissing: true,
      resolveReplacement: () => Effect.fail(new PatcherError({ reason: "must not be reached" }))
    }))).rejects.toThrow(/Installed binary does not exist/)
  })
})
