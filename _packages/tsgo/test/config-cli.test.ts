import * as Option from "effect/Option"
import { describe, expect, it } from "vitest"
import { assess } from "../src/cli/setup/assessment.js"
import { computeChanges } from "../src/cli/setup/changes.js"
import * as Target from "../src/cli/setup/target.js"
import type { Assessment } from "../src/cli/setup/types.js"

function createAssessmentInput(
  packageJson: Record<string, unknown>,
  tsconfig: Record<string, unknown>,
  vscodeSettings?: Record<string, unknown>,
  oxlintConfig?: Record<string, unknown>
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
    oxlintConfig: oxlintConfig
      ? Option.some({
        fileName: ".oxlintrc.json",
        text: JSON.stringify(oxlintConfig, null, 2)
      })
      : Option.none(),
    vscodeSettings: vscodeSettings
      ? Option.some({
        fileName: ".vscode/settings.json",
        text: JSON.stringify(vscodeSettings, null, 2)
      })
      : Option.none()
  }
}

describe("Config CLI", () => {
  it("preserves the assessed Oxlintrc schema path", () => {
    const assessmentState = assess(createAssessmentInput(
      { devDependencies: { "@effect/tsgo": "^0.1.0" } },
      {},
      undefined,
      { $schema: "./node_modules/oxlint/configuration_schema.json", rules: {} }
    ))

    expect(Target.fromAssessment(assessmentState).oxlintrcSchemaPath).toEqual(
      Option.some("./node_modules/oxlint/configuration_schema.json")
    )
  })

  it("only targets diagnostic severities", () => {
    const assessmentInput = createAssessmentInput(
      {
        name: "test-project",
        version: "1.0.0",
        devDependencies: {
          "@effect/tsgo": "^0.1.0"
        },
        scripts: {
          prepare: "effect-tsgo patch"
        }
      },
      {
        compilerOptions: {
          strict: true,
          plugins: [{
            name: "@effect/language-service",
            diagnosticSeverity: {
              floatingeffect: "warning"
            }
          }]
        }
      },
      {
        "editor.formatOnSave": true
      }
    )

    const assessmentState = assess(assessmentInput)

    const targetState = Target.withDiagnosticSeverities(Target.fromAssessment(assessmentState), {
      floatingEffect: "error",
      globalFetch: "warning"
    })

    expect(targetState.packageJson.lspVersion).toEqual(assessmentState.packageJson.lspVersion)
    expect(targetState.packageJson.prepareScript).toBe(true)
    expect(targetState.packageJson.integrations).toEqual(["typescript"])
    expect(targetState.editors).toEqual([])
    expect(targetState.vscodeSettings).toEqual(Option.map(assessmentState.vscodeSettings, (settings) => ({
      settings: settings.parsed
    })))

    const result = computeChanges(assessmentState, targetState)

    expect(result.codeActions.some((action) => action.changes.some((change) => change.fileName === "tsconfig.json")))
      .toBe(true)
    expect(result.codeActions.some((action) => action.changes.some((change) => change.fileName === "package.json")))
      .toBe(false)
    expect(
      result.codeActions.some((action) => action.changes.some((change) => change.fileName === ".vscode/settings.json"))
    ).toBe(false)
  })

  it("does not remove integration configuration when prepare is missing", () => {
    const assessmentState = assess(createAssessmentInput(
      {
        devDependencies: { "@effect/tsgo": "^0.1.0" }
      },
      {
        $schema: "./node_modules/@effect/tsgo/schema.json",
        compilerOptions: {
          plugins: [{ name: "@effect/language-service" }]
        }
      }
    ))
    const targetState = Target.withDiagnosticSeverities(Target.fromAssessment(assessmentState), {
      floatingEffect: "error"
    })
    const result = computeChanges(assessmentState, targetState)

    expect(result.codeActions.some((action) => action.description.includes("Remove $schema"))).toBe(false)
    expect(result.codeActions.some((action) => action.description.includes("Remove @effect/language-service"))).toBe(false)
  })

  it("adds the plugin when configuring an installed integration", () => {
    const assessmentState = assess(createAssessmentInput(
      { devDependencies: { "@effect/tsgo": "^0.1.0" } },
      { compilerOptions: {} }
    ))
    const targetState = Target.withDiagnosticSeverities(Target.fromAssessment(assessmentState), {
      floatingEffect: "error"
    })
    const result = computeChanges(assessmentState, targetState)

    expect(result.codeActions.some((action) => action.description.includes("@effect/language-service plugin"))).toBe(true)
  })
})
