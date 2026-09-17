import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as Option from "effect/Option"
import * as Flag from "effect/unstable/cli/Flag"
import {
  applyPresetDiagnosticSeverities,
  isPresetEnabled,
  type DiagnosticPresetName
} from "../presets.js"
import { getAllPresets, getAllRules, type RuleSeverity } from "./rule-info.js"
import type * as Target from "./target.js"
import type { Assessment, Editor, Integration } from "./types.js"

const dependencyTypes = ["devDependencies", "dependencies"] as const
const severityNames = ["off", "suggestion", "message", "warning", "error"] as const

export const setupFlags = {
  project: Flag.file("project").pipe(
    Flag.optional,
    Flag.withDescription("The project tsconfig file to configure")
  ),
  nonInteractive: Flag.boolean("non-interactive").pipe(
    Flag.withDefault(false),
    Flag.withDescription("Never open an interactive prompt")
  ),
  acceptDefaults: Flag.boolean("accept-defaults").pipe(
    Flag.withDefault(false),
    Flag.withDescription("Use recommended defaults for unspecified setup choices")
  ),
  apply: Flag.boolean("apply").pipe(
    Flag.withDefault(false),
    Flag.withDescription("Apply changes without asking for confirmation")
  ),
  typescript: Flag.boolean("typescript").pipe(
    Flag.optional,
    Flag.withDescription("Configure the TypeScript language service integration")
  ),
  oxlint: Flag.boolean("oxlint").pipe(
    Flag.optional,
    Flag.withDescription("Configure the Oxlint type-aware rules integration")
  ),
  dependencyType: Flag.choice("dependency-type", dependencyTypes).pipe(
    Flag.optional,
    Flag.withDescription("Install @effect/tsgo in this dependency section")
  ),
  preset: Flag.string("preset").pipe(
    Flag.atLeast(0),
    Flag.withDescription("Enable a diagnostic preset; may be specified more than once")
  ),
  noPresets: Flag.boolean("no-presets").pipe(
    Flag.withDefault(false),
    Flag.withDescription("Do not select any diagnostic presets")
  ),
  diagnostic: Flag.string("diagnostic").pipe(
    Flag.atLeast(0),
    Flag.withDescription("Set a diagnostic severity as <rule>=<severity>; may be specified more than once")
  ),
  vscode: Flag.boolean("vscode").pipe(
    Flag.optional,
    Flag.withDescription("Configure VS Code-based editors")
  ),
  zed: Flag.boolean("zed").pipe(
    Flag.optional,
    Flag.withDescription("Configure Zed")
  ),
  nvim: Flag.boolean("nvim").pipe(
    Flag.optional,
    Flag.withDescription("Show Neovim setup instructions")
  ),
  emacs: Flag.boolean("emacs").pipe(
    Flag.optional,
    Flag.withDescription("Show Emacs setup instructions")
  )
}

export type SetupFlags = {
  readonly [K in keyof typeof setupFlags]: typeof setupFlags[K] extends Flag.Flag<infer A> ? A : never
}

export class NonInteractiveSetupError extends Data.TaggedError("NonInteractiveSetupError")<{
  readonly reason: string
}> {
  get message(): string {
    return this.reason
  }
}

const fail = (reason: string) => Effect.fail(new NonInteractiveSetupError({ reason }))

export const hasNonInteractiveTargetFlags = (flags: SetupFlags): boolean =>
  flags.acceptDefaults ||
  Option.isSome(flags.typescript) ||
  Option.isSome(flags.oxlint) ||
  Option.isSome(flags.dependencyType) ||
  flags.preset.length > 0 ||
  flags.noPresets ||
  flags.diagnostic.length > 0 ||
  Option.isSome(flags.vscode) ||
  Option.isSome(flags.zed) ||
  Option.isSome(flags.nvim) ||
  Option.isSome(flags.emacs)

const currentDiagnosticSeverities = (assessment: Assessment.State) =>
  Option.getOrElse(assessment.tsconfig.currentDiagnosticSeverities, () => ({}))

const resolvePresets = (
  assessment: Assessment.State,
  flags: SetupFlags
): Effect.Effect<ReadonlyArray<DiagnosticPresetName>, NonInteractiveSetupError> => {
  if (flags.noPresets && flags.preset.length > 0) {
    return fail("--no-presets cannot be combined with --preset.")
  }

  const presets = getAllPresets()
  const presetNames = new Set(presets.map((preset) => preset.name))
  const selected = flags.noPresets
    ? []
    : flags.preset.length > 0
    ? flags.preset
    : flags.acceptDefaults
    ? presets
      .filter((preset) => isPresetEnabled(preset.name, currentDiagnosticSeverities(assessment)))
      .map((preset) => preset.name)
    : []
  const invalid = selected.find((name) => !presetNames.has(name))
  return invalid === undefined
    ? Effect.succeed(selected)
    : fail(`Unknown diagnostic preset '${invalid}'. Available presets: ${[...presetNames].join(", ")}.`)
}

const applyDiagnosticOverrides = (
  severities: Record<string, RuleSeverity>,
  overrides: ReadonlyArray<string>
): Effect.Effect<Record<string, RuleSeverity>, NonInteractiveSetupError> => {
  const rulesByLowerCase = new Map(getAllRules().map((rule) => [rule.name.toLowerCase(), rule.name]))
  const result = { ...severities }

  for (const override of overrides) {
    const separator = override.lastIndexOf("=")
    const inputName = separator === -1 ? "" : override.slice(0, separator)
    const severity = separator === -1 ? "" : override.slice(separator + 1)
    const ruleName = rulesByLowerCase.get(inputName.toLowerCase())
    if (ruleName === undefined || !severityNames.includes(severity as RuleSeverity)) {
      return fail(
        `Invalid diagnostic '${override}'. Expected <rule>=<severity>, where severity is one of ${severityNames.join(", ")}.`
      )
    }
    result[ruleName] = severity as RuleSeverity
  }

  return Effect.succeed(result)
}

export const resolveTargetOptions = (
  assessment: Assessment.State,
  flags: SetupFlags
): Effect.Effect<Target.Options, NonInteractiveSetupError> =>
  Effect.gen(function*() {
    if (!flags.acceptDefaults && (Option.isNone(flags.typescript) || Option.isNone(flags.oxlint))) {
      return yield* fail(
        "Non-interactive setup requires both integration flags or --accept-defaults. " +
          "Pass --typescript/--no-typescript and --oxlint/--no-oxlint."
      )
    }

    const defaultOxlint = Option.isSome(assessment.packageJson.oxlintVersion) ||
      Option.isSome(assessment.packageJson.oxlintTsgolintVersion) ||
      Option.isSome(assessment.packageJson.vitePlusVersion)
    const integrations: Array<Integration> = []
    if (Option.getOrElse(flags.typescript, () => flags.acceptDefaults)) {
      integrations.push("typescript")
    }
    if (Option.getOrElse(flags.oxlint, () => flags.acceptDefaults && defaultOxlint)) {
      integrations.push("oxlint")
    }
    const useTypescript = integrations.includes("typescript")
    const hasTypescriptOnlyOverrides = flags.preset.length > 0 ||
      flags.diagnostic.length > 0 ||
      Option.getOrElse(flags.vscode, () => false) ||
      Option.getOrElse(flags.zed, () => false) ||
      Option.getOrElse(flags.nvim, () => false) ||
      Option.getOrElse(flags.emacs, () => false)
    if (!useTypescript && hasTypescriptOnlyOverrides) {
      return yield* fail("Diagnostic presets, diagnostic overrides, and editor choices require --typescript.")
    }

    const dependencyType = Option.match(flags.dependencyType, {
      onNone: () => Option.match(assessment.packageJson.lspVersion, {
        onNone: () => "devDependencies" as const,
        onSome: (dependency) => dependency.dependencyType
      }),
      onSome: (value) => value
    })
    if (integrations.length > 0 && Option.isNone(flags.dependencyType) && !flags.acceptDefaults) {
      return yield* fail("Non-interactive setup requires --dependency-type unless --accept-defaults is used.")
    }

    const selectedPresets = useTypescript ? yield* resolvePresets(assessment, flags) : []
    const presetSeverities = applyPresetDiagnosticSeverities(
      currentDiagnosticSeverities(assessment),
      selectedPresets
    )
    const diagnosticSeverities = yield* applyDiagnosticOverrides(presetSeverities, flags.diagnostic)
    const editorSelections: ReadonlyArray<readonly [Option.Option<boolean>, Editor, boolean]> = [
      [flags.vscode, "vscode", Option.isSome(assessment.vscodeSettings)],
      [flags.zed, "zed", Option.isSome(assessment.zedSettings)],
      [flags.nvim, "nvim", false],
      [flags.emacs, "emacs", false]
    ]
    const editors = useTypescript
      ? editorSelections.flatMap(([selection, editor, recommended]) =>
        Option.getOrElse(selection, () => flags.acceptDefaults && recommended) ? [editor] : []
      )
      : []

    return { integrations, dependencyType, diagnosticSeverities, editors }
  })
