import * as Option from "effect/Option"
import * as Effect from "effect/Effect"
import * as Path from "effect/Path"
import { defaultTypescriptPackageNames } from "./consts.js"
import type { RuleSeverity } from "./rule-info.js"
import type { Assessment } from "./types.js"
import type { Editor, Integration, Target } from "./types.js"

export type { Editor, Target }

export interface Options {
  readonly integrations: ReadonlyArray<Integration>
  readonly dependencyType: "dependencies" | "devDependencies"
  readonly diagnosticSeverities: Readonly<Record<string, RuleSeverity>>
  readonly editors: ReadonlyArray<Editor>
}

export interface Context {
  readonly defaultLspVersion: string
  readonly defaultTypescriptVersion: string
  readonly defaultOxlintVersion: string
  readonly defaultOxlintTsgolintVersion: string
  readonly defaultSchemaPath: string
  readonly defaultOxlintrcSchemaPath: string
}

export const create = (
  assessment: Assessment.State,
  context: Context,
  options: Options
): Effect.Effect<Target.State, never, Path.Path> =>
  Effect.gen(function*() {
    const path = yield* Path.Path
    const { dependencyType, diagnosticSeverities: diagnosticSeveritiesRecord, editors, integrations } = options
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
        oxlintrcSchemaPath: Option.none(),
        vscodeSettings: Option.none(),
        editors: []
      } satisfies Target.State
    }

    const diagnosticSeverities = Object.keys(diagnosticSeveritiesRecord).length > 0
      ? Option.some({ ...diagnosticSeveritiesRecord })
      : Option.none()
    const defaultTypescriptPackageName = defaultTypescriptPackageNames[0]
    const relativeSchemaPath = path
      .relative(path.dirname(assessment.tsconfig.path), context.defaultSchemaPath)
      .replaceAll("\\", "/")
    const oxlintrcSchemaPath = useOxlint
      ? Option.map(assessment.oxlintConfig, (config) => {
        const relativePath = path
          .relative(path.dirname(config.path), context.defaultOxlintrcSchemaPath)
          .replaceAll("\\", "/")
        return relativePath.startsWith(".") ? relativePath : `./${relativePath}`
      })
      : Option.none()
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
        lspVersion: Option.some({ dependencyType, version: context.defaultLspVersion }),
        typescriptVersion: useTypescript
          ? Option.orElse(assessment.packageJson.typescriptVersion, () => Option.some({
            dependencyType,
            version: context.defaultTypescriptVersion,
            packageName: defaultTypescriptPackageName
          }))
          : assessment.packageJson.typescriptVersion,
        oxlintVersion: useOxlint && (
            Option.isSome(assessment.packageJson.oxlintVersion) ||
            Option.isNone(assessment.packageJson.vitePlusVersion)
          )
          ? Option.some({
            dependencyType: Option.match(assessment.packageJson.oxlintVersion, {
              onNone: () => dependencyType,
              onSome: (dependency) => dependency.dependencyType
            }),
            version: context.defaultOxlintVersion
          })
          : assessment.packageJson.oxlintVersion,
        oxlintTsgolintVersion: useOxlint && (
            Option.isSome(assessment.packageJson.oxlintTsgolintVersion) ||
            Option.isNone(assessment.packageJson.vitePlusVersion)
          )
          ? Option.some({
            dependencyType: Option.match(assessment.packageJson.oxlintTsgolintVersion, {
              onNone: () => dependencyType,
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
      oxlintrcSchemaPath,
      vscodeSettings,
      editors
    } satisfies Target.State
  })

export const fromAssessment = (inputState: Assessment.State): Target.State => ({
  packageJson: {
    lspVersion: inputState.packageJson.lspVersion,
    typescriptVersion: inputState.packageJson.typescriptVersion,
    oxlintVersion: inputState.packageJson.oxlintVersion,
    oxlintTsgolintVersion: inputState.packageJson.oxlintTsgolintVersion,
    prepareScript: Option.map(inputState.packageJson.prepareScript, (_) => _.hasPatch).pipe(
      Option.getOrElse(() => false)
    ),
    managePrepareScript: false,
    integrations: Option.match(inputState.packageJson.prepareScript, {
      onNone: () => Option.isSome(inputState.packageJson.lspVersion) ? ["typescript"] : [],
      onSome: (_) => _.integrations
    })
  },
  tsconfig: {
    schemaPath: inputState.tsconfig.currentSchemaPath,
    diagnosticSeverities: inputState.tsconfig.currentDiagnosticSeverities,
    manageIntegration: false
  },
  oxlintrcSchemaPath: Option.flatMap(inputState.oxlintConfig, (config) => config.currentSchemaPath),
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
