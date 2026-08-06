import * as Effect from "effect/Effect"
import * as Option from "effect/Option"
import * as Path from "effect/Path"
import * as Command from "effect/unstable/cli/Command"
import * as Assessment from "./assessment.js"
import * as Changes from "./changes.js"
import {
  hasNonInteractiveTargetFlags,
  NonInteractiveSetupError,
  resolveTargetOptions,
  setupFlags
} from "./options.js"
import * as Target from "./target.js"
import { gatherTargetOptions } from "./target-prompt.js"
import { readTsConfigFile, selectTsConfigFile } from "./tsconfig-prompt.js"
import * as upstreamJson from "../../../upstream.json" with { type: "json" }
import * as pkgJson from "../../../package.json" with { type: "json" }

export const setupCommand = Command.make("setup", setupFlags).pipe(
  Command.withDescription("Setup @effect/tsgo for the given project."),
  Command.withHandler((flags) =>
    Effect.gen(function*() {
      const path = yield* Path.Path

      const currentDir = path.resolve(process.cwd())
      if (!flags.nonInteractive && hasNonInteractiveTargetFlags(flags)) {
        return yield* Effect.fail(new NonInteractiveSetupError({
          reason: "Setup choice flags require --non-interactive."
        }))
      }
      if (flags.nonInteractive && Option.isNone(flags.project)) {
        return yield* Effect.fail(new NonInteractiveSetupError({
          reason: "Non-interactive setup requires --project."
        }))
      }
      const tsconfigInput = Option.isSome(flags.project)
        ? yield* readTsConfigFile(path.resolve(currentDir, flags.project.value))
        : yield* selectTsConfigFile(currentDir)

      const assessmentInput = yield* Assessment.createAssessmentInput(currentDir, tsconfigInput)
      const assessmentState = Assessment.assess(assessmentInput)
      const targetContext = {
        defaultLspVersion: pkgJson.version,
        defaultTypescriptVersion: upstreamJson.tags.typescript.latest,
        defaultOxlintVersion: upstreamJson.tags.oxlint.latest,
        defaultOxlintTsgolintVersion: upstreamJson.tags["oxlint-tsgolint"].latest,
        defaultSchemaPath: path.resolve(currentDir, "node_modules", pkgJson.name, "schema.json"),
        defaultOxlintrcSchemaPath: path.resolve(currentDir, "node_modules", pkgJson.name, "oxlint-schema.json")
      }
      const targetOptions = flags.nonInteractive
        ? yield* resolveTargetOptions(assessmentState, flags)
        : yield* gatherTargetOptions(assessmentState)
      const targetState = yield* Target.create(assessmentState, targetContext, targetOptions)
      const result = Changes.computeChanges(assessmentState, targetState)

      if (flags.apply) {
        yield* Changes.previewChanges(result, assessmentState)
        yield* Changes.applyChanges(result)
      } else if (flags.nonInteractive) {
        yield* Changes.previewChanges(result, assessmentState)
      } else {
        yield* Changes.reviewAndApplyChanges(result, assessmentState, {
          cancelMessage: "Setup cancelled. No changes were made."
        })
      }
    })
  )
)
