import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import * as ts from "typescript"
import * as Command from "effect/unstable/cli/Command"
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, it } from "vitest"
import { setupCommand } from "../../src/cli/setup/index.js"

const originalCwd = process.cwd()

afterEach(() => {
  process.chdir(originalCwd)
})

const runSetup = (args: ReadonlyArray<string>) =>
  Effect.runPromise(
    Command.runWith(setupCommand, { version: "test" })(args).pipe(Effect.provide(NodeServices.layer))
  )

describe("setup command", () => {
  it("previews or applies recommended defaults without prompting", async () => {
    const projectDir = await mkdtemp(join(tmpdir(), "effect-tsgo-setup-"))
    const packageJsonPath = join(projectDir, "package.json")
    const tsconfigPath = join(projectDir, "tsconfig.json")
    await writeFile(packageJsonPath, JSON.stringify({ name: "test-project", devDependencies: {} }, null, 2))
    await writeFile(tsconfigPath, JSON.stringify({ compilerOptions: {} }, null, 2))
    process.chdir(projectDir)

    try {
      const args = ["--non-interactive", "--project", "tsconfig.json", "--accept-defaults"]
      await runSetup(args)
      expect(await readFile(packageJsonPath, "utf8")).not.toContain("@effect/tsgo")

      await runSetup([...args, "--apply"])
      expect(await readFile(packageJsonPath, "utf8")).toContain("@effect/tsgo")
      expect(await readFile(tsconfigPath, "utf8")).toContain("@effect/language-service")
    } finally {
      process.chdir(originalCwd)
      await rm(projectDir, { recursive: true, force: true })
    }
  })

  it("migrates existing Zed JSONC without clobbering settings and is idempotent", async () => {
    const projectDir = await mkdtemp(join(tmpdir(), "effect-tsgo-setup-"))
    const packageJsonPath = join(projectDir, "package.json")
    const tsconfigPath = join(projectDir, "tsconfig.json")
    const zedSettingsPath = join(projectDir, ".zed", "settings.json")
    const cleanup = async () => {
      process.chdir(originalCwd)
      await rm(projectDir, { recursive: true, force: true })
    }
    try {
      await writeFile(packageJsonPath, JSON.stringify({ name: "test-project", devDependencies: {} }, null, 2))
      await writeFile(tsconfigPath, JSON.stringify({ compilerOptions: {} }, null, 2))
      await mkdir(join(projectDir, ".zed"))
      await writeFile(zedSettingsPath, `{
  "$schema": "zed://schemas/settings",
  // Keep non-TypeScript servers and formatting.
  "lsp": {
    "oxlint": {
      "binary": { "path": "oxlint", },
      "settings": { "run": "onSave", },
    },
    "oxfmt": {
      "binary": { "path": "oxfmt", },
    },
    "typescript-language-server": {
      "initialization_options": {
        "preferences": { "quotePreference": "single", },
      },
    },
  },
  "languages": {
    "TypeScript": {
      // Keep formatter and code actions.
      "formatter": "oxfmt",
      "code_actions_on_format": { "source.fixAll.oxlint": true, },
      "language_servers": ["typescript-language-server", "vtsls", "oxlint", "...",],
    },
    "TSX": {
      "formatter": "oxfmt",
      "language_servers": ["vtsls", "!typescript-language-server", "oxfmt", "...",],
    },
  },
}
`)
      process.chdir(projectDir)

      const args = [
        "--non-interactive",
        "--project",
        "tsconfig.json",
        "--typescript",
        "--no-oxlint",
        "--dependency-type",
        "devDependencies",
        "--no-presets",
        "--no-vscode",
        "--zed",
        "--apply"
      ]
      await runSetup(args)
      const migrated = await readFile(zedSettingsPath, "utf8")
      const sourceFile = ts.parseJsonText(zedSettingsPath, migrated)
      expect((sourceFile as ts.JsonSourceFile & {
        readonly parseDiagnostics: ReadonlyArray<ts.Diagnostic>
      }).parseDiagnostics).toEqual([])
      const conversionDiagnostics: Array<ts.Diagnostic> = []
      const parsed = ts.convertToObject(sourceFile, conversionDiagnostics) as {
        lsp: Record<string, unknown>
        languages: Record<string, unknown>
      }
      expect(conversionDiagnostics).toEqual([])

      const binaryPath =
        `./node_modules/@typescript/typescript-${process.platform}-${process.arch}/lib/${process.platform === "win32" ? "tsc.exe" : "tsc"}`
      expect(parsed.lsp["typescript-ls"]).toEqual({
        binary: {
          path: binaryPath,
          arguments: ["--lsp", "--stdio"]
        }
      })
      expect(parsed.languages.TypeScript).toEqual({
        formatter: "oxfmt",
        code_actions_on_format: { "source.fixAll.oxlint": true },
        language_servers: ["typescript-ls", "!typescript-language-server", "!vtsls", "oxlint", "..."]
      })
      expect(parsed.languages.TSX).toEqual({
        formatter: "oxfmt",
        language_servers: ["typescript-ls", "!typescript-language-server", "!vtsls", "oxfmt", "..."]
      })
      expect(parsed.lsp["typescript-language-server"]).toEqual({
        initialization_options: {
          preferences: { quotePreference: "single" }
        }
      })
      expect(parsed.lsp.oxlint).toEqual({
        binary: { path: "oxlint" },
        settings: { run: "onSave" }
      })
      expect(parsed.lsp.oxfmt).toEqual({ binary: { path: "oxfmt" } })
      expect(migrated).toContain("// Keep non-TypeScript servers and formatting.")
      expect(migrated).toContain("// Keep formatter and code actions.")
      expect(migrated).toContain('"binary": { "path": "oxlint", }')
      expect(migrated).toContain('"code_actions_on_format": { "source.fixAll.oxlint": true, }')
      expect(migrated).toContain('"language_servers": ["typescript-ls", "!typescript-language-server", "!vtsls", "oxlint", "..."],')

      await runSetup(args)
      expect(await readFile(zedSettingsPath, "utf8")).toBe(migrated)
    } finally {
      await cleanup()
    }
  })

  it("rejects malformed Zed JSONC without overwriting it or printing restart guidance", async () => {
    const projectDir = await mkdtemp(join(tmpdir(), "effect-tsgo-setup-"))
    const packageJsonPath = join(projectDir, "package.json")
    const tsconfigPath = join(projectDir, "tsconfig.json")
    const zedSettingsPath = join(projectDir, ".zed", "settings.json")
    const malformedSettings = `{
  "lsp": {
    "typescript-ls": {
  }
}
`
    const output: Array<string> = []
    const originalLog = console.log
    try {
      await writeFile(packageJsonPath, JSON.stringify({ name: "test-project", devDependencies: {} }, null, 2))
      await writeFile(tsconfigPath, JSON.stringify({ compilerOptions: {} }, null, 2))
      await mkdir(join(projectDir, ".zed"))
      await writeFile(zedSettingsPath, malformedSettings)
      process.chdir(projectDir)
      console.log = (...args: ReadonlyArray<unknown>) => {
        output.push(args.map(String).join(" "))
      }

      const failure = await runSetup([
        "--non-interactive",
        "--project",
        "tsconfig.json",
        "--typescript",
        "--no-oxlint",
        "--dependency-type",
        "devDependencies",
        "--no-presets",
        "--no-vscode",
        "--zed",
        "--apply"
      ]).then(
        () => undefined,
        (error: unknown) => error
      )
      expect(failure).toMatchObject({
        _tag: "EditorSettingsParseError",
        diagnostics: expect.arrayContaining([expect.anything()])
      })
      expect(String(failure)).toMatch(/Invalid editor settings at .*\/\.zed\/settings\.json\./)
      expect(await readFile(zedSettingsPath, "utf8")).toBe(malformedSettings)
      expect(output.join("\n")).not.toContain("Restart Zed")
    } finally {
      console.log = originalLog
      process.chdir(originalCwd)
      await rm(projectDir, { recursive: true, force: true })
    }
  })

  it("fails instead of prompting when the project is missing", async () => {
    await expect(runSetup(["--non-interactive", "--accept-defaults"]))
      .rejects.toThrow("Non-interactive setup requires --project")
  })

  it("rejects --zed without non-interactive setup mode", async () => {
    await expect(runSetup(["--zed"]))
      .rejects.toThrow("Setup choice flags require --non-interactive")
  })
})
