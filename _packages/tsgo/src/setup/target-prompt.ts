import * as Effect from "effect/Effect"
import * as Option from "effect/Option"
import type * as Terminal from "effect/Terminal"
import * as Prompt from "effect/unstable/cli/Prompt"
import type { Assessment, Editor, Target } from "./types.js"

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
          plugin: false
        },
        vscodeSettings: Option.none(),
        editors: []
      } satisfies Target.State
    }

    // Prepare Script Configuration
    const currentPrepareState = Option.match(assessment.packageJson.prepareScript, {
      onNone: () => false,
      onSome: (script) => script.hasPatch
    })

    const prepareScript = yield* Prompt.select({
      message: "Enable language service diagnostics during TypeScript compilation?",
      choices: [
        {
          title: "Yes",
          description:
            "Enable patching - Recommended for LLMs, diagnostics will appear in tsc output and CI/CD, will patch the tsc binary and typescript library locally.",
          value: true,
          selected: currentPrepareState
        },
        {
          title: "No",
          description: "Skip patching - diagnostics only available in your editor",
          value: false,
          selected: !currentPrepareState
        }
      ]
    })

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
          "typescript.native-preview.tsdk": "node_modules/@typescript/native-preview"
        }
      })
      : Option.none()

    return {
      packageJson: {
        lspVersion: Option.some({ dependencyType: lspDependencyType, version: context.defaultLspVersion }),
        prepareScript
      },
      tsconfig: {
        plugin: true
      },
      vscodeSettings,
      editors
    } satisfies Target.State
  })
