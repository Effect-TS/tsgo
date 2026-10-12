import * as Console from "effect/Console"
import * as Effect from "effect/Effect"
import { Command } from "effect/cli"
import { Flag } from "effect/cli"
import {
  type Component,
  discoverBinaries,
  type ReplacementResolver,
  requireComponents,
  resolveReplacement,
  selectComponents
} from "../patcher/index.js"

export type OxlintEnvShell = "sh" | "powershell"

export interface OxlintEnvVariable {
  readonly name: string
  readonly value: string
}

const oxlintEnvVariables = [
  { component: "oxlint", name: "NAPI_RS_NATIVE_LIBRARY_PATH" },
  { component: "oxlint-tsgolint", name: "OXLINT_TSGOLINT_PATH" }
] as const

export const resolveOxlintEnv = (
  cwd: string,
  resolver: ReplacementResolver = resolveReplacement
) => Effect.scoped(Effect.gen(function*() {
  const components = new Set<Component>(oxlintEnvVariables.map(({ component }) => component))
  const targets = yield* requireComponents(selectComponents(yield* discoverBinaries(cwd), components), components)
  return yield* Effect.forEach(oxlintEnvVariables, ({ component, name }) => {
    const target = targets.find((target) => target.component === component)!
    return resolver(target).pipe(Effect.map(({ path }): OxlintEnvVariable => ({ name, value: path })))
  })
}))

export const renderOxlintEnv = (variables: ReadonlyArray<OxlintEnvVariable>, shell: OxlintEnvShell): string =>
  variables.map(({ name, value }) =>
    shell === "powershell"
      ? `$env:${name} = '${value.replace(/['\u2018\u2019\u201a\u201b]/g, "$&$&")}'`
      : `export ${name}='${value.replace(/'/g, "'\\''")}'`
  ).join("\n")

export const oxlintEnvCommand = Command.make("oxlint-env", {
  shell: Flag.Literals("shell", ["sh", "powershell"] as ReadonlyArray<OxlintEnvShell>).pipe(
    Flag.withDefault("sh" as const),
    Flag.withDescription("Assignment syntax: sh (POSIX shells) or powershell")
  )
}).pipe(
  Command.withDescription("Print environment variables that load the Effect Oxlint integration without patching"),
  Command.withHandler(({ shell }) => Effect.gen(function*() {
    const variables = yield* resolveOxlintEnv(process.cwd())
    yield* Console.log(renderOxlintEnv(variables, shell))
  }))
)
