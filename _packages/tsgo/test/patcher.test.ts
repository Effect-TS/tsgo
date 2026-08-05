import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Path from "effect/Path"
import * as Scope from "effect/Scope"
import { access, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, it } from "vitest"
import {
  applyPlan,
  type DiscoveredBinary,
  PatcherError,
  preparePatch,
  prepareUnpatch,
  renderOxlintDeclarations,
  ReplacementUnavailableError,
  resolveReplacement
} from "../src/patcher/index.js"

const temporaryDirectories: Array<string> = []

const makeTemporaryDirectory = async () => {
  const directory = await mkdtemp(join(tmpdir(), "effect-tsgo-patcher-"))
  temporaryDirectories.push(directory)
  return directory
}

const run = <A, E>(effect: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path | Scope.Scope>) =>
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
      { component: "oxlint-dts", packageName: "oxlint", packageVersion: "1", binaryPath: first },
      { component: "oxlint-tsgolint", packageName: "oxlint-tsgolint", packageVersion: "2", binaryPath: second }
    ]

    const plan = await run(preparePatch(targets, {
      skipMissing: false,
      resolveReplacement: (target) => Effect.succeed({
        path: target.component === "oxlint-dts" ? firstReplacement : secondReplacement
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
    await writeFile(declarationPath, [
      'type LintPluginOptionsSchema = "eslint" | "typescript";',
      "type RuleNoConfig = unknown;",
      "interface DummyRuleMap {",
      '  "eslint/no-unused-vars"?: RuleNoConfig;',
      "}",
      ""
    ].join("\n"))
    const target: DiscoveredBinary = {
      component: "oxlint-dts",
      packageName: "oxlint",
      packageVersion: "1.0.0",
      binaryPath: declarationPath
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
  })

  it("rejects declarations whose expected anchors changed", () => {
    expect(() => renderOxlintDeclarations("interface OtherRuleMap {}\n")).toThrow(
      "Unable to locate the Oxlint plugin declaration."
    )
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

  it("treats the persisted original backup as already patched", async () => {
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
      binaryPath: targetPath
    }
    const plan = await run(preparePatch([target], {
      skipMissing: false,
      resolveReplacement: () => Effect.succeed({ path: replacementPath })
    }))
    expect(plan.operations).toEqual([])
    expect(plan.skipped[0]?.reason).toBe("already-patched")
    expect(plan.skipped[0]?.message).toBe(
      `oxlint-tsgolint skipped because backup already exists at ${targetPath}.original.`
    )
    await expect(access(`${targetPath}.original.1`)).rejects.toThrow()
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
      binaryPath: targetPath
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
      binaryPath: targetPath
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
    await expect(run(preparePatch([{ ...target, binaryPath: join(directory, "absent") }], {
      skipMissing: true,
      resolveReplacement: () => Effect.fail(new PatcherError({ reason: "must not be reached" }))
    }))).rejects.toThrow(/Installed binary does not exist/)
  })
})
