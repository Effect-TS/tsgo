import * as Effect from "effect/Effect"
import * as Option from "effect/Option"
import * as Path from "effect/Path"
import type * as Terminal from "effect/Terminal"
import * as Prompt from "effect/unstable/cli/Prompt"
import { applyPresetDiagnosticSeverities, type DiagnosticPresetName, isPresetEnabled } from "../presets.js"
import { defaultTypescriptPackageNames } from "./consts.js"
import type { Assessment, Editor, Integration, Target } from "./types.js"
import { getAllPresets, getAllRules } from "./rule-info.js"
import { createRulePrompt } from "./rule-prompt.js"

/**
 * Context input for gathering target state
 */
export interface GatherTargetContext {
  readonly defaultLspVersion: string
  readonly defaultTypescriptVersion: string
  readonly defaultOxlintVersion: string
  readonly defaultOxlintTsgolintVersion: string
  readonly defaultSchemaPath: string
}

/**
 * Gather target state from user based on current assessment
 */
export const gatherTargetState = (
  assessment: Assessment.State,
  context: GatherTargetContext
): Effect.Effect<Target.State, Terminal.QuitError, Prompt.Environment | Path.Path> =>
  Effect.gen(function*() {
    const path = yield* Path.Path

    const integrations = yield* Prompt.multiSelect({
      message: "Which integrations would you like to configure?",
      choices: [
        {
          title: "TypeScript language service",
          value: "typescript" as Integration,
          selected: true
        },
        {
          title: "Oxlint type-aware rules",
          value: "oxlint" as Integration,
          selected: Option.isSome(assessment.packageJson.oxlintVersion) ||
            Option.isSome(assessment.packageJson.oxlintTsgolintVersion)
        }
      ]
    })

    const useTypescript = integrations.includes("typescript")
    const useOxlint = integrations.includes("oxlint")

    if (integrations.length === 0) {
      return {
        packageJson: {
          lspVersion: Option.none(),
          typescriptVersion: assessment.packageJson.typescriptVersion,
          oxlintVersion: assessment.packageJson.oxlintVersion,
          oxlintTsgolintVersion: assessment.packageJson.oxlintTsgolintVersion,
          prepareScript: false,
          managePrepareScript: true,
          integrations
        },
        tsconfig: {
          schemaPath: Option.none(),
          diagnosticSeverities: Option.none(),
          manageIntegration: true
        },
        vscodeSettings: Option.none(),
        editors: []
      } satisfies Target.State
    }

    // Determine current LSP installation state
    const currentLspState = Option.match(assessment.packageJson.lspVersion, {
      onNone: () => "no" as const,
      onSome: (lsp) => lsp.dependencyType
    })

    // Ask where to install the CLI used by either integration.
    const lspDependencyType = yield* Prompt.select({
      message: "@effect/tsgo installation:",
      choices: [
        {
          title: "Install in devDependencies",
          description: "This is the recommended default option",
          value: "devDependencies" as const,
          selected: currentLspState === "no" || currentLspState === "devDependencies"
        },
        {
          title: "Install in dependencies",
          description: "We usually don't recommend this, but if you need it for any reason",
          value: "dependencies" as const,
          selected: currentLspState === "dependencies"
        }
      ]
    })

    const currentDiagnosticSeverities = Option.match(assessment.tsconfig.currentDiagnosticSeverities, {
      onNone: () => ({}),
      onSome: (diagnosticSeverities) => diagnosticSeverities
    })

    const selectedDiagnosticModes = useTypescript ? yield* Prompt.multiSelect({
      message: "Which diagnostic presets would you like to use?",
      choices: [
        {
          title: "Custom",
          description: "Review and adjust individual diagnostic severities after presets are applied",
          value: "custom" as const
        },
        ...getAllPresets().map((preset) => ({
          title: preset.name,
          description: preset.description,
          value: preset.name as DiagnosticPresetName,
          selected: isPresetEnabled(preset.name as DiagnosticPresetName, currentDiagnosticSeverities)
        }))
      ]
    }) : []

    const shouldCustomizeDiagnostics = selectedDiagnosticModes.includes("custom")
    const selectedPresetNames = selectedDiagnosticModes.filter((value): value is DiagnosticPresetName =>
      value !== "custom"
    )
    const initialSeverities = applyPresetDiagnosticSeverities(currentDiagnosticSeverities, selectedPresetNames)

    const diagnosticSeveritiesRecord = shouldCustomizeDiagnostics
      ? yield* createRulePrompt(
        getAllRules(),
        initialSeverities
      )
      : initialSeverities

    const diagnosticSeverities = Object.keys(diagnosticSeveritiesRecord).length > 0
      ? Option.some(diagnosticSeveritiesRecord)
      : Option.none()

    // Editor Selection - Using multi-select
    // Pre-select VSCode if .vscode/settings.json exists
    const hasVscodeSettings = Option.isSome(assessment.vscodeSettings)

    const editors = useTypescript ? yield* Prompt.multiSelect({
      message: "Which editors do you use?",
      choices: [
        {
          title: "VS Code / Cursor / VS Code-based editors",
          value: "vscode" as Editor,
          selected: hasVscodeSettings
        },
        {
          title: "Neovim",
          value: "nvim" as Editor
        },
        {
          title: "Emacs",
          value: "emacs" as Editor
        }
      ]
    }) : []

    // Build target state
    const defaultTypescriptPackageName = defaultTypescriptPackageNames[0]
    const relativeSchemaPath = path
      .relative(path.dirname(assessment.tsconfig.path), context.defaultSchemaPath)
      .replaceAll("\\", "/")
    const vscodeSettings: Option.Option<Target.VSCodeSettings> = editors.includes("vscode")
      ? Option.some({
        settings: {
          "js/ts.experimental.useTsgo": true,
          "js/ts.tsdk.path": "./node_modules/typescript/bin",
          "js/ts.tsdk.promptToUseWorkspaceVersion": true,
          "js/ts.tsdk.additionalLocations": ["./node_modules/typescript/bin"]
        }
      })
      : Option.none()

    return {
      packageJson: {
        lspVersion: Option.some({ dependencyType: lspDependencyType, version: context.defaultLspVersion }),
        typescriptVersion: useTypescript
          ? Option.orElse(assessment.packageJson.typescriptVersion, () => Option.some({
            dependencyType: lspDependencyType,
            version: context.defaultTypescriptVersion,
            packageName: defaultTypescriptPackageName
          }))
          : assessment.packageJson.typescriptVersion,
        oxlintVersion: useOxlint
          ? Option.some({
            dependencyType: Option.match(assessment.packageJson.oxlintVersion, {
              onNone: () => lspDependencyType,
              onSome: (dependency) => dependency.dependencyType
            }),
            version: context.defaultOxlintVersion
          })
          : assessment.packageJson.oxlintVersion,
        oxlintTsgolintVersion: useOxlint
          ? Option.some({
            dependencyType: Option.match(assessment.packageJson.oxlintTsgolintVersion, {
              onNone: () => lspDependencyType,
              onSome: (dependency) => dependency.dependencyType
            }),
            version: context.defaultOxlintTsgolintVersion
          })
          : assessment.packageJson.oxlintTsgolintVersion,
        prepareScript: true,
        managePrepareScript: true,
        integrations
      },
      tsconfig: {
        schemaPath: useTypescript
          ? Option.some(relativeSchemaPath.startsWith(".") ? relativeSchemaPath : `./${relativeSchemaPath}`)
          : Option.none(),
        diagnosticSeverities,
        manageIntegration: true
      },
      vscodeSettings,
      editors
    } satisfies Target.State
  })
