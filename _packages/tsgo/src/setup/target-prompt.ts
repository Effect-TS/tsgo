import * as Effect from "effect/Effect"
import * as Option from "effect/Option"
import type * as Terminal from "effect/Terminal"
import * as Prompt from "effect/unstable/cli/Prompt"
import { applyPresetDiagnosticSeverities, type DiagnosticPresetName, isPresetEnabled } from "../presets.js"
import type { Assessment } from "./types.js"
import type { Editor, Target } from "./target.js"
import { getAllPresets, getAllRules } from "./rule-info.js"
import { createRulePrompt } from "./rule-prompt.js"

/**
 * Context input for gathering target state
 */
export interface GatherTargetContext {
  readonly defaultLspVersion: string
}

/**
 * Gather target state from user based on current assessment
 */
export const gatherTargetState = (
  assessment: Assessment.State,
  context: GatherTargetContext
): Effect.Effect<Target.State, Terminal.QuitError, Prompt.Environment> =>
  Effect.gen(function*() {
    // Determine current LSP installation state
    const currentLspState = Option.match(assessment.packageJson.lspVersion, {
      onNone: () => "no" as const,
      onSome: (lsp) => lsp.dependencyType
    })

    // Ask what user wants to do with the language service
    const lspDependencyType = yield* Prompt.select({
      message: "Language service installation:",
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
        },
        {
          title: "Uninstall",
          description: "Language service won't be installed or will be removed if already present",
          value: "no" as const
        }
      ]
    })

    // If user doesn't want to install the language service, return early with everything disabled
    if (lspDependencyType === "no") {
      return {
        packageJson: {
          lspVersion: Option.none(),
          prepareScript: false
        },
        tsconfig: {
          diagnosticSeverities: Option.none()
        },
        vscodeSettings: Option.none(),
        editors: []
      } satisfies Target.State
    }

    const currentDiagnosticSeverities = Option.match(assessment.tsconfig.currentDiagnosticSeverities, {
      onNone: () => ({}),
      onSome: (diagnosticSeverities) => diagnosticSeverities
    })

    const selectedDiagnosticModes = yield* Prompt.multiSelect({
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
    })

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

    const editors = yield* Prompt.multiSelect({
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
    })

    // Build target state
    const vscodeSettings: Option.Option<Target.VSCodeSettings> = editors.includes("vscode")
      ? Option.some({
        settings: {
          "typescript.native-preview.tsdk": "node_modules/@typescript/native-preview",
          "typescript.experimental.useTsgo": true
        }
      })
      : Option.none()

    return {
      packageJson: {
        lspVersion: Option.some({ dependencyType: lspDependencyType, version: context.defaultLspVersion }),
        prepareScript: true
      },
      tsconfig: {
        diagnosticSeverities
      },
      vscodeSettings,
      editors
    } satisfies Target.State
  })
