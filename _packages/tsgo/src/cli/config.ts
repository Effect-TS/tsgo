import * as Effect from "effect/Effect"
import * as Option from "effect/Option"
import * as Path from "effect/Path"
import * as Command from "effect/unstable/cli/Command"
import * as Assessment from "./setup/assessment.js"
import * as Changes from "./setup/changes.js"
import { getAllRules } from "./setup/rule-info.js"
import { createRulePrompt } from "./setup/rule-prompt.js"
import * as Target from "./setup/target.js"
import { selectTsConfigFile } from "./setup/tsconfig-prompt.js"

export const configCommand = Command.make("config").pipe(
  Command.withDescription("Configure diagnostic severities for an existing tsconfig using the interactive rule picker."),
  Command.withHandler(() =>
    Effect.gen(function*() {
      const path = yield* Path.Path
      const currentDir = path.resolve(process.cwd())
      const tsconfigInput = yield* selectTsConfigFile(currentDir)
      const assessmentInput = yield* Assessment.createAssessmentInput(currentDir, tsconfigInput)
      const assessmentState = Assessment.assess(assessmentInput)

      const allRules = getAllRules()
      const currentDiagnosticSeverities = Option.match(assessmentState.tsconfig.currentDiagnosticSeverities, {
        onNone: () => ({}),
        onSome: (diagnosticSeverities) => diagnosticSeverities
      })

      const diagnosticSeverities = yield* createRulePrompt(allRules, currentDiagnosticSeverities)
      const targetState = Target.withDiagnosticSeverities(Target.fromAssessment(assessmentState), diagnosticSeverities)
      const result = Changes.computeChanges(assessmentState, targetState)

      yield* Changes.reviewAndApplyChanges(result, assessmentState, {
        confirmMessage: "Apply diagnostic configuration changes?",
        cancelMessage: "Configuration cancelled. No changes were made."
      })
    })
  )
)
