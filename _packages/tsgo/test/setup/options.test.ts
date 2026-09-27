import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Effect from "effect/Effect"
import * as Option from "effect/Option"
import * as Command from "effect/unstable/cli/Command"
import { describe, expect, it } from "vitest"
import { assess } from "../../src/cli/setup/assessment.js"
import {
  resolveTargetOptions,
  setupFlags,
  type SetupFlags
} from "../../src/cli/setup/options.js"
import type { Assessment } from "../../src/cli/setup/types.js"

const createAssessment = (): Assessment.State =>
  assess({
    packageJson: {
      fileName: "/project/package.json",
      text: JSON.stringify({
        devDependencies: {
          "@effect/tsgo": "1.0.0",
          "oxlint": "1.0.0"
        }
      })
    },
    tsconfig: {
      fileName: "/project/tsconfig.json",
      text: JSON.stringify({
        compilerOptions: {
          plugins: [{
            name: "@effect/language-service",
            diagnosticSeverity: { floatingEffect: "warning" }
          }]
        }
      })
    },
    oxlintConfig: Option.none(),
    vscodeSettings: Option.some({
      fileName: "/project/.vscode/settings.json",
      text: "{}"
    }),
    zedSettings: Option.some({
      fileName: "/project/.zed/settings.json",
      text: "{}"
    })
  })

const parseSetupFlags = async (args: ReadonlyArray<string>) => {
  let result: SetupFlags | undefined
  const command = Command.make("setup", setupFlags).pipe(
    Command.withHandler((flags) => Effect.sync(() => result = flags))
  )
  await Effect.runPromise(
    Command.runWith(command, { version: "test" })(args).pipe(Effect.provide(NodeServices.layer))
  )
  return result
}

describe("non-interactive setup options", () => {
  it("parses automation and target choice flags", async () => {
    const flags = await parseSetupFlags([
      "--non-interactive",
      "--project",
      "tsconfig.json",
      "--accept-defaults",
      "--apply",
      "--no-typescript",
      "--oxlint",
      "--dependency-type",
      "dependencies",
      "--preset",
      "effect-native",
      "--diagnostic",
      "floatingEffect=error",
      "--vscode",
      "--zed",
      "--nvim",
      "--no-emacs"
    ])

    expect(flags).toMatchObject({
      nonInteractive: true,
      acceptDefaults: true,
      apply: true,
      typescript: Option.some(false),
      oxlint: Option.some(true),
      dependencyType: Option.some("dependencies"),
      preset: ["effect-native"],
      diagnostic: ["floatingEffect=error"],
      vscode: Option.some(true),
      zed: Option.some(true),
      nvim: Option.some(true),
      emacs: Option.some(false)
    })
  })

  it("reproduces assessment-sensitive recommendations", async () => {
    const flags = await parseSetupFlags(["--non-interactive", "--accept-defaults"])
    const options = await Effect.runPromise(resolveTargetOptions(createAssessment(), flags!))

    expect(options.integrations).toEqual(["typescript", "oxlint"])
    expect(options.dependencyType).toBe("devDependencies")
    expect(options.editors).toEqual(["vscode", "zed"])
    expect(options.diagnosticSeverities.floatingEffect).toBe("warning")
  })

  it("lets --no-zed override the accepted Zed recommendation", async () => {
    const flags = await parseSetupFlags([
      "--non-interactive",
      "--accept-defaults",
      "--no-zed"
    ])
    const options = await Effect.runPromise(resolveTargetOptions(createAssessment(), flags!))

    expect(options.editors).toEqual(["vscode"])
  })

  it("requires unresolved choices when defaults are not accepted", async () => {
    const flags = await parseSetupFlags(["--non-interactive"])

    await expect(Effect.runPromise(resolveTargetOptions(createAssessment(), flags!)))
      .rejects.toThrow("requires both integration flags or --accept-defaults")
  })

  it("requires both integration decisions without defaults", async () => {
    const flags = await parseSetupFlags([
      "--non-interactive",
      "--typescript",
      "--dependency-type",
      "devDependencies"
    ])

    await expect(Effect.runPromise(resolveTargetOptions(createAssessment(), flags!)))
      .rejects.toThrow("requires both integration flags")
  })

  it("uses explicit choices instead of recommendations", async () => {
    const flags = await parseSetupFlags([
      "--non-interactive",
      "--typescript",
      "--no-oxlint",
      "--dependency-type",
      "dependencies",
      "--no-presets",
      "--diagnostic",
      "floatingEffect=error",
      "--no-vscode",
      "--no-zed",
      "--nvim"
    ])
    const options = await Effect.runPromise(resolveTargetOptions(createAssessment(), flags!))

    expect(options).toMatchObject({
      integrations: ["typescript"],
      dependencyType: "dependencies",
      editors: ["nvim"],
      diagnosticSeverities: { floatingEffect: "error" }
    })
  })

  it("rejects invalid presets and diagnostic overrides", async () => {
    const invalidPreset = await parseSetupFlags([
      "--non-interactive",
      "--accept-defaults",
      "--preset",
      "unknown"
    ])
    await expect(Effect.runPromise(resolveTargetOptions(createAssessment(), invalidPreset!)))
      .rejects.toThrow("Unknown diagnostic preset 'unknown'")

    const invalidDiagnostic = await parseSetupFlags([
      "--non-interactive",
      "--accept-defaults",
      "--diagnostic",
      "floatingEffect=loud"
    ])
    await expect(Effect.runPromise(resolveTargetOptions(createAssessment(), invalidDiagnostic!)))
      .rejects.toThrow("Invalid diagnostic 'floatingEffect=loud'")
  })

  it("does not select TypeScript-only defaults when TypeScript is disabled", async () => {
    const flags = await parseSetupFlags([
      "--non-interactive",
      "--accept-defaults",
      "--no-typescript",
      "--oxlint"
    ])
    const options = await Effect.runPromise(resolveTargetOptions(createAssessment(), flags!))

    expect(options.editors).toEqual([])
  })

  it.each(["--vscode", "--zed"])(
    "rejects the TypeScript-only %s override when TypeScript is disabled",
    async (editorFlag) => {
      const flags = await parseSetupFlags([
        "--non-interactive",
        "--no-typescript",
        "--oxlint",
        "--dependency-type",
        "devDependencies",
        editorFlag
      ])

      await expect(Effect.runPromise(resolveTargetOptions(createAssessment(), flags!)))
        .rejects.toThrow("editor choices require --typescript")
    }
  )
})
