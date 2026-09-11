import * as Effect from "effect/Effect"
import * as Option from "effect/Option"
import type * as Terminal from "effect/Terminal"
import * as Prompt from "effect/unstable/cli/Prompt"
import { applyPresetDiagnosticSeverities, type DiagnosticPresetName, isPresetEnabled } from "../presets.js"
import * as Target from "./target.js"
import type { Assessment, Editor, Integration } from "./types.js"
import { getAllPresets, getAllRules } from "./rule-info.js"
import { createRulePrompt } from "./rule-prompt.js"

/**
 * Context input for gathering target state
 */
export type GatherTargetContext = Target.Context

/**
 * Gather target state from user based on current assessment
 */
export const gatherTargetOptions = (
  assessment: Assessment.State
): Effect.Effect<Target.Options, Terminal.QuitError, Prompt.Environment> =>
  Effect.gen(function*() {
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
            Option.isSome(assessment.packageJson.oxlintTsgolintVersion) ||
            Option.isSome(assessment.packageJson.vitePlusVersion)
        }
      ]
    })

    const useTypescript = integrations.includes("typescript")
    const useOxlint = integrations.includes("oxlint")

    if (integrations.length === 0) {
      return {
        integrations,
        dependencyType: "devDependencies",
        diagnosticSeverities: {},
        editors: []
      }
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

    // Editor Selection - Using multi-select
    // Pre-select editors with existing settings files.
    const hasVscodeSettings = Option.isSome(assessment.vscodeSettings)
    const hasZedSettings = Option.isSome(assessment.zedSettings)

    const editors = useTypescript ? yield* Prompt.multiSelect({
      message: "Which editors do you use?",
      choices: [
        {
          title: "VS Code / Cursor / VS Code-based editors",
          value: "vscode" as Editor,
          selected: hasVscodeSettings
        },
        {
          title: "Zed",
          value: "zed" as Editor,
          selected: hasZedSettings
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

    return {
      integrations,
      dependencyType: lspDependencyType,
      diagnosticSeverities: diagnosticSeveritiesRecord,
      editors
    } satisfies Target.Options
  })

export const gatherTargetState = (
  assessment: Assessment.State,
  context: GatherTargetContext
) => gatherTargetOptions(assessment).pipe(Effect.flatMap((options) => Target.create(assessment, context, options)))
