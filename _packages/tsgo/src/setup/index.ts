import * as Effect from "effect/Effect"
import * as Path from "effect/Path"
import * as Command from "effect/unstable/cli/Command"
import * as Assessment from "./assessment.js"
import * as Changes from "./changes.js"
import { gatherTargetState } from "./target-prompt.js"
import { selectTsConfigFile } from "./tsconfig-prompt.js"
import { DEFAULT_LSP_VERSION } from "./consts.js"

export const setupCommand = Command.make("setup").pipe(
  Command.withDescription("Setup @effect/tsgo for the given project using an interactive CLI."),
  Command.withHandler(() =>
    Effect.gen(function*() {
      const path = yield* Path.Path

      const currentDir = path.resolve(process.cwd())
      const tsconfigInput = yield* selectTsConfigFile(currentDir)

      const assessmentInput = yield* Assessment.createAssessmentInput(currentDir, tsconfigInput)
      const assessmentState = Assessment.assess(assessmentInput)
      const targetState = yield* gatherTargetState(assessmentState, {
        defaultLspVersion: DEFAULT_LSP_VERSION
      })
      const result = Changes.computeChanges(assessmentState, targetState)

      yield* Changes.reviewAndApplyChanges(result, assessmentState, {
        cancelMessage: "Setup cancelled. No changes were made."
      })
    })
  )
)
