import * as Console from "effect/Console"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Option from "effect/Option"
import * as Path from "effect/Path"
import * as Command from "effect/unstable/cli/Command"
import * as Prompt from "effect/unstable/cli/Prompt"
import { assess, createAssessmentInput } from "./assessment.js"
import { computeChanges } from "./changes.js"
import { renderCodeActions } from "./diff-renderer.js"
import { gatherTargetState } from "./target-prompt.js"
import { selectTsConfigFile } from "./tsconfig-prompt.js"

const DEFAULT_LSP_VERSION = "^0.0.0"

export const setupCommand = Command.make("setup").pipe(
  Command.withDescription("Setup @effect/tsgo for the given project using an interactive CLI."),
  Command.withHandler(() =>
    Effect.gen(function*() {
      const path = yield* Path.Path
      const fs = yield* FileSystem.FileSystem

      // ========================================================================
      // Phase 1: Select tsconfig file
      // ========================================================================
      const currentDir = path.resolve(process.cwd())
      const tsconfigInput = yield* selectTsConfigFile(currentDir)

      // ========================================================================
      // Phase 2: Read files and create assessment input
      // ========================================================================
      const assessmentInput = yield* createAssessmentInput(currentDir, tsconfigInput)

      // ========================================================================
      // Phase 3: Perform assessment
      // ========================================================================
      const assessmentState = assess(assessmentInput)

      // ========================================================================
      // Phase 4: Gather target state from user
      // ========================================================================
      const targetState = yield* gatherTargetState(assessmentState, {
        defaultLspVersion: DEFAULT_LSP_VERSION
      })

      // ========================================================================
      // Phase 5: Compute changes
      // ========================================================================
      const result = computeChanges(assessmentState, targetState)

      // ========================================================================
      // Phase 6: Review changes
      // ========================================================================
      yield* renderCodeActions(result, assessmentState)

      // Check if VSCode was selected but settings file doesn't exist
      const needsNewVSCodeSettings = targetState.editors.includes("vscode") &&
        Option.isSome(targetState.packageJson.lspVersion) &&
        Option.isSome(targetState.vscodeSettings) &&
        Option.isNone(assessmentState.vscodeSettings)

      if (result.codeActions.length === 0 && !needsNewVSCodeSettings) {
        return
      }

      // ========================================================================
      // Phase 6b: Confirm changes
      // ========================================================================
      const shouldProceed = yield* Prompt.confirm({
        message: "Apply all changes?",
        initial: true
      })

      if (!shouldProceed) {
        yield* Console.log("Setup cancelled. No changes were made.")
        return
      }

      // ========================================================================
      // Phase 7: Apply changes
      // ========================================================================
      yield* Console.log("")
      yield* Console.log("Applying changes...")

      for (const codeAction of result.codeActions) {
        for (const fileChange of codeAction.changes) {
          const fileName = fileChange.fileName
          const fileExists = yield* fs.exists(fileName)

          if (fileExists) {
            const existingContent = yield* fs.readFileString(fileName)

            // Sort changes in reverse order by position to avoid offset issues
            const sortedChanges = [...fileChange.textChanges].sort((a, b) => b.span.start - a.span.start)

            let newContent = existingContent
            for (const textChange of sortedChanges) {
              const start = textChange.span.start
              const end = start + textChange.span.length
              newContent = newContent.slice(0, start) + textChange.newText + newContent.slice(end)
            }

            yield* fs.writeFileString(fileName, newContent)
          }
        }
      }

      // Handle creating new .vscode/settings.json if needed
      if (needsNewVSCodeSettings) {
        const vscodeDir = path.join(currentDir, ".vscode")
        const vscodeSettingsPath = path.join(vscodeDir, "settings.json")

        yield* fs.makeDirectory(vscodeDir, { recursive: true }).pipe(Effect.ignore)

        const settings = Option.isSome(targetState.vscodeSettings)
          ? targetState.vscodeSettings.value.settings
          : {}
        const content = JSON.stringify(settings, null, 2) + "\n"
        yield* fs.writeFileString(vscodeSettingsPath, content)
      }

      yield* Console.log("Changes applied successfully!")
      yield* Console.log("")

      // Display any additional messages (editor setup instructions)
      for (const message of result.messages) {
        yield* Console.log(message)
      }
    })
  )
)
