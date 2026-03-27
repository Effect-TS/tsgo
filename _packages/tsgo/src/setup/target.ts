import * as Option from "effect/Option"
import type { Assessment } from "./types.js"
import type { Editor, Target } from "./types.js"
import type { RuleSeverity } from "./rule-info.js"

export type { Editor, Target }

export const fromAssessment = (inputState: Assessment.State): Target.State => ({
  packageJson: {
    lspVersion: inputState.packageJson.lspVersion,
    nativePreviewVersion: inputState.packageJson.nativePreviewVersion,
    prepareScript: Option.map(inputState.packageJson.prepareScript, (_) => _.hasPatch).pipe(
      Option.getOrElse(() => false)
    )
  },
  tsconfig: {
    diagnosticSeverities: inputState.tsconfig.currentDiagnosticSeverities
  },
  vscodeSettings: Option.map(inputState.vscodeSettings, (settings) => ({
    settings: settings.parsed
  })),
  editors: []
})

export const withDiagnosticSeverities = (
  state: Target.State,
  diagnosticSeverities: Record<string, RuleSeverity>
): Target.State => ({
  ...state,
  tsconfig: {
    ...state.tsconfig,
    diagnosticSeverities: Option.some(diagnosticSeverities)
  }
})
