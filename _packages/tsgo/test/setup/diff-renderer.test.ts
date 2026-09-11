import { describe, it, expect } from "vitest"
import * as Effect from "effect/Effect"
import * as Option from "effect/Option"
import * as Console from "effect/Console"
import * as Layer from "effect/Layer"
import * as ts from "typescript"
import { renderCodeActions } from "../../src/cli/setup/diff-renderer.js"
import type { ComputeChangesResult } from "../../src/cli/setup/changes.js"
import type { Assessment } from "../../src/cli/setup/types.js"

/**
 * Run renderCodeActions and capture all console output lines
 */
function runAndCapture(
  result: ComputeChangesResult,
  assessmentState: Assessment.State
): Array<string> {
  const captured: Array<string> = []
  const testConsole: Console.Console = {
    assert: () => {},
    clear: () => {},
    count: () => {},
    countReset: () => {},
    debug: () => {},
    dir: () => {},
    dirxml: () => {},
    error: () => {},
    group: () => {},
    groupCollapsed: () => {},
    groupEnd: () => {},
    info: () => {},
    log(...args: ReadonlyArray<unknown>): void {
      captured.push(args.map(String).join(" "))
    },
    table: () => {},
    time: () => {},
    timeEnd: () => {},
    timeLog: () => {},
    trace: () => {},
    warn: () => {}
  }
  const layer = Layer.succeed(Console.Console, testConsole)

  Effect.runSync(
    renderCodeActions(result, assessmentState).pipe(
      Effect.provide(layer)
    )
  )

  return captured
}

/**
 * Helper to create a minimal Assessment.State for testing
 */
function makeAssessmentState(opts?: {
  vscodeSettings?: {
    path: string
    text: string
  }
  zedSettings?: {
    path: string
    text: string
  }
}): Assessment.State {
  const pkgJsonText = JSON.stringify({ name: "test", version: "1.0.0" }, null, 2)
  const tsconfigText = JSON.stringify({ compilerOptions: {} }, null, 2)

  const pkgJsonSourceFile = ts.parseJsonText("/test/package.json", pkgJsonText) as ts.JsonSourceFile
  const tsconfigSourceFile = ts.parseJsonText("/test/tsconfig.json", tsconfigText) as ts.JsonSourceFile

  const vscodeSettings = opts?.vscodeSettings
    ? Option.some({
      path: opts.vscodeSettings.path,
      sourceFile: ts.parseJsonText(opts.vscodeSettings.path, opts.vscodeSettings.text) as ts.JsonSourceFile,
      parsed: JSON.parse(opts.vscodeSettings.text) as Record<string, unknown>,
      text: opts.vscodeSettings.text
    })
    : Option.none<Assessment.VSCodeSettings>()

  const zedSettings = opts?.zedSettings
    ? Option.some({
      path: opts.zedSettings.path,
      sourceFile: ts.parseJsonText(opts.zedSettings.path, opts.zedSettings.text) as ts.JsonSourceFile,
      parsed: ts.convertToObject(
        ts.parseJsonText(opts.zedSettings.path, opts.zedSettings.text),
        []
      ) as Record<string, unknown>,
      text: opts.zedSettings.text
    })
    : Option.none<Assessment.ZedSettings>()

  return {
    packageJson: {
      path: "/test/package.json",
      sourceFile: pkgJsonSourceFile,
      parsed: JSON.parse(pkgJsonText),
      text: pkgJsonText,
      lspVersion: Option.none(),
      typescriptVersion: Option.none(),
      oxlintVersion: Option.none(),
      oxlintTsgolintVersion: Option.none(),
      vitePlusVersion: Option.none(),
      prepareScript: Option.none()
    },
    tsconfig: {
      path: "/test/tsconfig.json",
      sourceFile: tsconfigSourceFile,
      parsed: JSON.parse(tsconfigText),
      text: tsconfigText,
      hasPlugins: false,
      hasLspPlugin: false,
      currentSchemaPath: Option.none(),
      currentDiagnosticSeverities: Option.none()
    },
    oxlintConfig: Option.none(),
    vscodeSettings,
    zedSettings
  }
}

describe("renderCodeActions", () => {
  it("should render new-file code actions as all-green + lines", () => {
    const newFileContent = JSON.stringify({
      "typescript.tsserver.experimental.enableProjectDiagnostics": true
    }, null, 2) + "\n"

    const result: ComputeChangesResult = {
      codeActions: [{
        description: "Create .vscode/settings.json",
        changes: [{
          fileName: "/test/.vscode/settings.json",
          isNewFile: true,
          textChanges: [{
            span: { start: 0, length: 0 },
            newText: newFileContent
          }]
        }]
      }],
      messages: []
    }

    const state = makeAssessmentState()
    const output = runAndCapture(result, state)

    // Should contain the description
    const hasDescription = output.some((line) => line.includes("Create .vscode/settings.json"))
    expect(hasDescription).toBe(true)

    // Should contain the file name
    const hasFileName = output.some((line) => line.includes("/test/.vscode/settings.json"))
    expect(hasFileName).toBe(true)

    // Should render content lines with + symbol (green ANSI codes)
    const contentLines = output.filter((line) => line.includes("+"))
    expect(contentLines.length).toBeGreaterThan(0)

    // Each content line should contain the green ANSI escape code
    for (const line of contentLines) {
      expect(line).toContain("\x1b[32m")
    }

    // Should NOT contain "(file will be modified)"
    const hasFallback = output.some((line) => line.includes("(file will be modified)"))
    expect(hasFallback).toBe(false)

    // The rendered content should include the actual JSON content
    const allOutput = output.join("\n")
    expect(allOutput).toContain("typescript.tsserver.experimental.enableProjectDiagnostics")
  })

  it("should render modification code actions with standard diff output", () => {
    const existingText = JSON.stringify({
      compilerOptions: {
        target: "ES2022"
      }
    }, null, 2)

    const tsconfigSourceFile = ts.parseJsonText("/test/tsconfig.json", existingText) as ts.JsonSourceFile

    const state: Assessment.State = {
      ...makeAssessmentState(),
      tsconfig: {
        path: "/test/tsconfig.json",
        sourceFile: tsconfigSourceFile,
        parsed: JSON.parse(existingText),
        text: existingText,
        hasPlugins: false,
        hasLspPlugin: false,
        currentSchemaPath: Option.none(),
        currentDiagnosticSeverities: Option.none()
      }
    }

    // Create a modification code action that changes "ES2022" to "ES2024"
    const result: ComputeChangesResult = {
      codeActions: [{
        description: "Update tsconfig.json compiler target",
        changes: [{
          fileName: "/test/tsconfig.json",
          isNewFile: false,
          textChanges: [{
            span: {
              start: existingText.indexOf('"ES2022"'),
              length: '"ES2022"'.length
            },
            newText: '"ES2024"'
          }]
        }]
      }],
      messages: []
    }

    const output = runAndCapture(result, state)

    // Should contain the description
    const hasDescription = output.some((line) => line.includes("Update tsconfig.json compiler target"))
    expect(hasDescription).toBe(true)

    // Should contain diff markers (red - for removal, green + for addition)
    const hasRemoval = output.some((line) => line.includes("\x1b[31m") && line.includes("-"))
    const hasAddition = output.some((line) => line.includes("\x1b[32m") && line.includes("+"))
    expect(hasRemoval).toBe(true)
    expect(hasAddition).toBe(true)

    // The diff should show the old and new values
    const allOutput = output.join("\n")
    expect(allOutput).toContain("ES2022")
    expect(allOutput).toContain("ES2024")

    // Should NOT contain "(file will be modified)" since source file was found
    const hasFallback = output.some((line) => line.includes("(file will be modified)"))
    expect(hasFallback).toBe(false)
  })

  it("should render .oxlintrc.json modifications from the assessed source", () => {
    const existingText = '{\n  "rules": {}\n}\n'
    const state: Assessment.State = {
      ...makeAssessmentState(),
      oxlintConfig: Option.some({
        path: "/test/.oxlintrc.json",
        sourceFile: ts.parseJsonText("/test/.oxlintrc.json", existingText) as ts.JsonSourceFile,
        parsed: JSON.parse(existingText),
        text: existingText,
        currentSchemaPath: Option.none()
      })
    }
    const result: ComputeChangesResult = {
      codeActions: [{
        description: "Add $schema to .oxlintrc.json",
        changes: [{
          fileName: "/test/.oxlintrc.json",
          isNewFile: false,
          textChanges: [{
            span: { start: 2, length: 0 },
            newText: '"$schema": "./node_modules/@effect/tsgo/oxlint-schema.json",\n  '
          }]
        }]
      }],
      messages: []
    }

    const output = runAndCapture(result, state)
    const allOutput = output.join("\n")
    expect(allOutput).toContain("oxlint-schema.json")
    expect(allOutput).not.toContain("(file will be modified)")
  })

  it("renders .zed/settings.json modifications from the assessed JSONC source", () => {
    const existingText = `{
  "lsp": {
    "typescript-language-server": {
      "binary": {
        // Keep this comment in the rendered source.
        "path": "/usr/local/bin/typescript-language-server"
      }
    }
  }
}
`
    const path = "/test/.zed/settings.json"
    const state = makeAssessmentState({
      zedSettings: { path, text: existingText }
    })
    const oldPath = '"/usr/local/bin/typescript-language-server"'
    const result: ComputeChangesResult = {
      codeActions: [{
        description: "Update lsp.typescript-language-server.binary.path setting",
        changes: [{
          fileName: path,
          isNewFile: false,
          textChanges: [{
            span: {
              start: existingText.indexOf(oldPath),
              length: oldPath.length
            },
            newText: '"./node_modules/.bin/tsc"'
          }]
        }]
      }],
      messages: []
    }

    const output = runAndCapture(result, state).join("\n")
    expect(output).toContain("/usr/local/bin/typescript-language-server")
    expect(output).toContain("./node_modules/.bin/tsc")
    expect(output).toContain("Keep this comment in the rendered source.")
    expect(output).not.toContain("(file will be modified)")
  })

  it("should render no changes message when codeActions is empty", () => {
    const result: ComputeChangesResult = {
      codeActions: [],
      messages: []
    }

    const state = makeAssessmentState()
    const output = runAndCapture(result, state)

    const allOutput = output.join("\n")
    expect(allOutput).toContain("No changes needed")
  })
})
