import * as Option from "effect/Option"
import { describe, expect, it } from "vitest"
import { assess, type Assessment } from "../../src/setup/assessment.js"
import { computeChanges } from "../../src/setup/changes.js"
import type { Target } from "../../src/setup/types.js"

/**
 * Create a test assessment input from plain objects
 */
function createTestAssessmentInput(
  packageJson: Record<string, unknown>,
  tsconfig: Record<string, unknown>,
  vscodeSettings?: Record<string, unknown>
): Assessment.Input {
  return {
    packageJson: {
      fileName: "package.json",
      text: JSON.stringify(packageJson, null, 2)
    },
    tsconfig: {
      fileName: "tsconfig.json",
      text: JSON.stringify(tsconfig, null, 2)
    },
    vscodeSettings: vscodeSettings !== undefined
      ? Option.some({
        fileName: ".vscode/settings.json",
        text: JSON.stringify(vscodeSettings, null, 2)
      })
      : Option.none()
  }
}

/**
 * Apply text changes to original text and return the result
 */
function applyTextChanges(
  originalText: string,
  textChanges: ReadonlyArray<{ span: { start: number; length: number }; newText: string }>
): string {
  let result = originalText
  // Apply changes in reverse order to maintain correct positions
  const sortedChanges = [...textChanges].sort((a, b) => b.span.start - a.span.start)
  for (const change of sortedChanges) {
    const before = result.substring(0, change.span.start)
    const after = result.substring(change.span.start + change.span.length)
    result = before + change.newText + after
  }
  return result
}

/**
 * Helper to test setup changes and generate snapshots
 */
function expectSetupChanges(
  assessmentInput: Assessment.Input,
  targetState: Target.State
) {
  // Run assessment (synchronous in tsgo)
  const assessmentState = assess(assessmentInput)

  // Compute changes (synchronous in tsgo)
  const result = computeChanges(assessmentState, targetState)

  // 1. Snapshot of change summary (file + description)
  const changeSummary = result.codeActions.flatMap((action) =>
    action.changes.map((fileChange) => ({
      file: fileChange.fileName,
      description: action.description
    }))
  )
  expect(changeSummary).toMatchSnapshot("change summary")

  // 2. Snapshot of final package.json and validate it's valid JSON
  const packageJsonFileChange = result.codeActions
    .flatMap((action) => action.changes)
    .find((fc) => fc.fileName === "package.json")
  if (packageJsonFileChange) {
    if (packageJsonFileChange.isNewFile) {
      const content = packageJsonFileChange.textChanges[0]?.newText
      expect(content).toMatchSnapshot("package.json")
      expect(() => JSON.parse(content!)).not.toThrow()
    } else {
      const finalPackageJson = applyTextChanges(assessmentInput.packageJson.text, packageJsonFileChange.textChanges)
      expect(finalPackageJson).toMatchSnapshot("package.json")
      expect(() => JSON.parse(finalPackageJson)).not.toThrow()
    }
  }

  // 3. Snapshot of final tsconfig.json and validate it's valid JSON
  const tsconfigFileChange = result.codeActions
    .flatMap((action) => action.changes)
    .find((fc) => fc.fileName === "tsconfig.json")
  if (tsconfigFileChange) {
    if (tsconfigFileChange.isNewFile) {
      const content = tsconfigFileChange.textChanges[0]?.newText
      expect(content).toMatchSnapshot("tsconfig.json")
      expect(() => JSON.parse(content!)).not.toThrow()
    } else {
      const finalTsconfig = applyTextChanges(assessmentInput.tsconfig.text, tsconfigFileChange.textChanges)
      expect(finalTsconfig).toMatchSnapshot("tsconfig.json")
      expect(() => JSON.parse(finalTsconfig)).not.toThrow()
    }
  }

  // 4. Snapshot of messages
  if (result.messages.length > 0) {
    expect(result.messages).toMatchSnapshot("messages")
  }

  // 5. Snapshot of final .vscode/settings.json and validate it's valid JSON
  const vscodeSettingsFileChange = result.codeActions
    .flatMap((action) => action.changes)
    .find((fc) => fc.fileName.endsWith("settings.json"))
  if (vscodeSettingsFileChange) {
    if (vscodeSettingsFileChange.isNewFile) {
      const content = vscodeSettingsFileChange.textChanges[0]?.newText
      expect(content).toMatchSnapshot(".vscode/settings.json")
      expect(() => JSON.parse(content!)).not.toThrow()
    } else if (Option.isSome(assessmentInput.vscodeSettings)) {
      const finalVscodeSettings = applyTextChanges(
        assessmentInput.vscodeSettings.value.text,
        vscodeSettingsFileChange.textChanges
      )
      expect(finalVscodeSettings).toMatchSnapshot(".vscode/settings.json")
      expect(() => JSON.parse(finalVscodeSettings)).not.toThrow()
    }
  }
}

const VSCODE_SETTINGS: Target.VSCodeSettings = {
  settings: {
    "typescript.native-preview.tsdk": "node_modules/@typescript/native-preview"
  }
}

describe("Setup CLI", () => {
  it("should generate changes for adding LSP with defaults", () => {
    const assessmentInput = createTestAssessmentInput(
      {
        name: "test-project",
        version: "1.0.0",
        dependencies: {}
      },
      {
        compilerOptions: {
          strict: true,
          target: "ES2022"
        }
      }
    )

    const targetState: Target.State = {
      packageJson: {
        lspVersion: Option.some({ dependencyType: "devDependencies" as const, version: "^0.0.5" }),
        prepareScript: false
      },
      tsconfig: {
        plugin: true
      },
      vscodeSettings: Option.none(),
      editors: []
    }

    expectSetupChanges(assessmentInput, targetState)
  })

  it("should generate changes for removing LSP when already installed", () => {
    const assessmentInput = createTestAssessmentInput(
      {
        name: "test-project",
        version: "1.0.0",
        dependencies: {},
        devDependencies: {
          "@effect/tsgo": "^0.1.0",
          typescript: "^5.0.0"
        }
      },
      {
        compilerOptions: {
          strict: true,
          target: "ES2022",
          plugins: [
            {
              name: "@effect/language-service"
            }
          ]
        }
      }
    )

    const targetState: Target.State = {
      packageJson: {
        lspVersion: Option.none(),
        prepareScript: false
      },
      tsconfig: {
        plugin: false
      },
      vscodeSettings: Option.none(),
      editors: []
    }

    expectSetupChanges(assessmentInput, targetState)
  })

  it("should update LSP version when already installed with older version", () => {
    const assessmentInput = createTestAssessmentInput(
      {
        name: "test-project",
        version: "1.0.0",
        dependencies: {},
        devDependencies: {
          "@effect/tsgo": "^0.0.1",
          typescript: "^5.0.0"
        }
      },
      {
        compilerOptions: {
          strict: true,
          target: "ES2022",
          plugins: [
            {
              name: "@effect/language-service"
            }
          ]
        }
      }
    )

    const targetState: Target.State = {
      packageJson: {
        lspVersion: Option.some({ dependencyType: "devDependencies" as const, version: "^0.0.5" }),
        prepareScript: false
      },
      tsconfig: {
        plugin: true
      },
      vscodeSettings: Option.none(),
      editors: []
    }

    expectSetupChanges(assessmentInput, targetState)
  })

  it("should generate changes for adding LSP with prepare script", () => {
    const assessmentInput = createTestAssessmentInput(
      {
        name: "test-project",
        version: "1.0.0",
        dependencies: {}
      },
      {
        compilerOptions: {
          strict: true,
          target: "ES2022"
        }
      }
    )

    const targetState: Target.State = {
      packageJson: {
        lspVersion: Option.some({ dependencyType: "devDependencies" as const, version: "^0.0.5" }),
        prepareScript: true
      },
      tsconfig: {
        plugin: true
      },
      vscodeSettings: Option.none(),
      editors: []
    }

    expectSetupChanges(assessmentInput, targetState)
  })

  it("should generate changes for removing LSP and prepare script", () => {
    const assessmentInput = createTestAssessmentInput(
      {
        name: "test-project",
        version: "1.0.0",
        devDependencies: {
          "@effect/tsgo": "^0.1.0",
          typescript: "^5.0.0"
        },
        scripts: {
          prepare: "effect-tsgo patch"
        }
      },
      {
        compilerOptions: {
          strict: true,
          target: "ES2022",
          plugins: [
            {
              name: "@effect/language-service"
            }
          ]
        }
      }
    )

    const targetState: Target.State = {
      packageJson: {
        lspVersion: Option.none(),
        prepareScript: false
      },
      tsconfig: {
        plugin: false
      },
      vscodeSettings: Option.none(),
      editors: []
    }

    expectSetupChanges(assessmentInput, targetState)
  })

  it("should remove only LSP patch command from prepare script with multiple commands", () => {
    const assessmentInput = createTestAssessmentInput(
      {
        name: "test-project",
        version: "1.0.0",
        devDependencies: {
          "@effect/tsgo": "^0.1.0",
          typescript: "^5.0.0"
        },
        scripts: {
          prepare: "husky install && effect-tsgo patch",
          test: "vitest"
        }
      },
      {
        compilerOptions: {
          strict: true,
          target: "ES2022",
          plugins: [
            {
              name: "@effect/language-service"
            }
          ]
        }
      }
    )

    const targetState: Target.State = {
      packageJson: {
        lspVersion: Option.none(),
        prepareScript: false
      },
      tsconfig: {
        plugin: false
      },
      vscodeSettings: Option.none(),
      editors: []
    }

    expectSetupChanges(assessmentInput, targetState)
  })

  it("should not override existing plugins when adding LSP plugin", () => {
    const assessmentInput = createTestAssessmentInput(
      {
        name: "test-project",
        version: "1.0.0",
        dependencies: {}
      },
      {
        compilerOptions: {
          strict: true,
          target: "ES2022",
          plugins: [
            {
              name: "typescript-plugin-css-modules",
              options: {
                classnameTransform: "camelCase"
              }
            },
            {
              name: "another-typescript-plugin"
            }
          ]
        }
      }
    )

    const targetState: Target.State = {
      packageJson: {
        lspVersion: Option.some({ dependencyType: "devDependencies" as const, version: "^0.0.5" }),
        prepareScript: false
      },
      tsconfig: {
        plugin: true
      },
      vscodeSettings: Option.none(),
      editors: []
    }

    expectSetupChanges(assessmentInput, targetState)
  })

  it("should add LSP plugin alongside existing plugins", () => {
    const assessmentInput = createTestAssessmentInput(
      {
        name: "test-project",
        version: "1.0.0",
        dependencies: {}
      },
      {
        compilerOptions: {
          strict: true,
          target: "ES2022",
          plugins: [
            {
              name: "typescript-plugin-css-modules"
            }
          ]
        }
      }
    )

    const targetState: Target.State = {
      packageJson: {
        lspVersion: Option.some({ dependencyType: "devDependencies" as const, version: "^0.0.5" }),
        prepareScript: false
      },
      tsconfig: {
        plugin: true
      },
      vscodeSettings: Option.none(),
      editors: []
    }

    expectSetupChanges(assessmentInput, targetState)
  })

  it("should remove only LSP plugin while preserving other plugins", () => {
    const assessmentInput = createTestAssessmentInput(
      {
        name: "test-project",
        version: "1.0.0",
        dependencies: {},
        devDependencies: {
          "@effect/tsgo": "^0.1.0",
          typescript: "^5.0.0"
        }
      },
      {
        compilerOptions: {
          strict: true,
          target: "ES2022",
          plugins: [
            {
              name: "typescript-plugin-css-modules"
            },
            {
              name: "@effect/language-service"
            }
          ]
        }
      }
    )

    const targetState: Target.State = {
      packageJson: {
        lspVersion: Option.none(),
        prepareScript: false
      },
      tsconfig: {
        plugin: false
      },
      vscodeSettings: Option.none(),
      editors: []
    }

    expectSetupChanges(assessmentInput, targetState)
  })

  it("should add LSP with VS Code editor selected and create new settings file", () => {
    const assessmentInput = createTestAssessmentInput(
      {
        name: "test-project",
        version: "1.0.0",
        dependencies: {}
      },
      {
        compilerOptions: {
          strict: true,
          target: "ES2022"
        }
      }
      // No vscodeSettings — file does not exist
    )

    const targetState: Target.State = {
      packageJson: {
        lspVersion: Option.some({ dependencyType: "devDependencies" as const, version: "^0.0.5" }),
        prepareScript: false
      },
      tsconfig: {
        plugin: true
      },
      vscodeSettings: Option.some(VSCODE_SETTINGS),
      editors: ["vscode"]
    }

    expectSetupChanges(assessmentInput, targetState)
  })

  it("should preserve existing VS Code settings when adding LSP-specific settings", () => {
    const assessmentInput = createTestAssessmentInput(
      {
        name: "test-project",
        version: "1.0.0",
        dependencies: {}
      },
      {
        compilerOptions: {
          strict: true,
          target: "ES2022"
        }
      },
      {
        "editor.formatOnSave": true,
        "editor.tabSize": 2,
        "files.autoSave": "onFocusChange"
      }
    )

    const targetState: Target.State = {
      packageJson: {
        lspVersion: Option.some({ dependencyType: "devDependencies" as const, version: "^0.0.5" }),
        prepareScript: false
      },
      tsconfig: {
        plugin: true
      },
      vscodeSettings: Option.some(VSCODE_SETTINGS),
      editors: ["vscode"]
    }

    expectSetupChanges(assessmentInput, targetState)
  })

  it("should preserve all existing VS Code settings from a real repository config", () => {
    const assessmentInput = createTestAssessmentInput(
      {
        name: "test-project",
        version: "1.0.0",
        dependencies: {}
      },
      {
        compilerOptions: {
          strict: true,
          target: "ES2022"
        }
      },
      {
        "typescript.tsdk": "./packages/core/node_modules/typescript/lib",
        "typescript.preferences.importModuleSpecifier": "non-relative",
        "typescript.enablePromptUseWorkspaceTsdk": true,
        "editor.formatOnSave": true,
        "eslint.format.enable": true,
        "editor.acceptSuggestionOnCommitCharacter": true,
        "editor.acceptSuggestionOnEnter": "on",
        "editor.quickSuggestionsDelay": 10,
        "editor.suggestOnTriggerCharacters": true,
        "editor.tabCompletion": "off",
        "editor.suggest.localityBonus": true,
        "editor.suggestSelection": "recentlyUsed",
        "editor.wordBasedSuggestions": "matchingDocuments",
        "editor.parameterHints.enabled": true,
        "files.insertFinalNewline": true
      }
    )

    const targetState: Target.State = {
      packageJson: {
        lspVersion: Option.some({ dependencyType: "devDependencies" as const, version: "^0.0.5" }),
        prepareScript: false
      },
      tsconfig: {
        plugin: true
      },
      vscodeSettings: Option.some(VSCODE_SETTINGS),
      editors: ["vscode"]
    }

    expectSetupChanges(assessmentInput, targetState)
  })
})
