import * as NodeRuntime from "@effect/platform-node/NodeRuntime"
import * as NodeServices from "@effect/platform-node/NodeServices"
import * as Console from "effect/Console"
import * as Data from "effect/Data"
import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Option from "effect/Option"
import * as Command from "effect/unstable/cli/Command"
import * as Flag from "effect/unstable/cli/Flag"
import {
  type Component,
  defaultTypescriptPackageNames,
  discoverBinaries,
  patch,
  requireComponents,
  resolveReplacement,
  selectComponents,
  unpatch
} from "../patcher/index.js"
import { configCommand } from "./config.js"
import {
  type DiagnosticsOutputFormat,
  propagateDiagnosticsExit,
  runDiagnosticsBinary
} from "./diagnostics.js"
import { ensureIntegrationSelected, integrationFlags, IntegrationSelectionError } from "./integrationFlags.js"
import { setupCommand } from "./setup/index.js"
import * as pkgJson from "../../package.json" with { type: "json" }

class ChmodBinaryError extends Data.TaggedError("ChmodBinaryError")<{ readonly targetPath: string }> {
  get message(): string {
    return `Failed to set executable permissions on ${this.targetPath}.`
  }
}

const selectedComponents = (typescript: boolean, oxlint: boolean): ReadonlySet<Component> => {
  const components = new Set<Component>()
  if (typescript) components.add("typescript")
  if (oxlint) {
    components.add("oxlint")
    components.add("oxlint-tsgolint")
  }
  return components
}

const discoverSelected = (
  components: ReadonlySet<Component>,
  preferredTypescriptPackage?: string
) => Effect.gen(function*() {
  const discovered = yield* discoverBinaries(process.cwd(), preferredTypescriptPackage)
  const selected = selectComponents(discovered, components)
  return yield* requireComponents(selected, components)
})

const renderSkipped = (skipped: ReadonlyArray<{ readonly message: string }>) =>
  Effect.forEach(skipped, ({ message }) => Console.error(message), { discard: true })

const typescriptPackageFlag = Flag.optional(
  Flag.string("typescript-package").pipe(
    Flag.withDescription("Native TypeScript package name to try before the default package names")
  )
)

const patchCommand = Command.make("patch", {
  ...integrationFlags,
  force: Flag.boolean("force").pipe(
    Flag.withDescription("Deprecated compatibility flag; replacements are selected by package version")
  ),
  skipMissing: Flag.boolean("skip-missing").pipe(
    Flag.withDefault(false),
    Flag.withDescription("Skip selected integrations without a matching packaged replacement")
  ),
  typescriptPackage: typescriptPackageFlag
}).pipe(
  Command.withDescription("Patch the selected Effect integrations"),
  Command.withHandler(({ oxlint, skipMissing, typescript, typescriptPackage }) => Effect.gen(function*() {
    yield* ensureIntegrationSelected(typescript, oxlint)
    if (!typescript && Option.isSome(typescriptPackage)) {
      return yield* new IntegrationSelectionError({
        reason: "--typescript-package requires the TypeScript integration."
      })
    }
    const components = selectedComponents(typescript, oxlint)
    const result = yield* patch({
      cwd: process.cwd(),
      components,
      preferredTypescriptPackage: Option.getOrUndefined(typescriptPackage),
      skipMissing
    })
    yield* renderSkipped(result.skipped)
    yield* Effect.forEach(
      result.changed,
      (target) => Console.log(`Patched ${target.component} at ${target.binaryPath}`),
      { discard: true }
    )
  }))
)

const unpatchCommand = Command.make("unpatch", {
  ...integrationFlags,
  typescriptPackage: typescriptPackageFlag
}).pipe(
  Command.withDescription("Unpatch and restore the selected integrations"),
  Command.withHandler(({ oxlint, typescript, typescriptPackage }) => Effect.gen(function*() {
    yield* ensureIntegrationSelected(typescript, oxlint)
    if (!typescript && Option.isSome(typescriptPackage)) {
      return yield* new IntegrationSelectionError({
        reason: "--typescript-package requires the TypeScript integration."
      })
    }
    const components = selectedComponents(typescript, oxlint)
    const result = yield* unpatch({
      cwd: process.cwd(),
      components,
      preferredTypescriptPackage: Option.getOrUndefined(typescriptPackage)
    })
    yield* renderSkipped(result.skipped)
    yield* Effect.forEach(
      result.changed,
      (target) => Console.log(`Restored original ${target.component} at ${target.binaryPath}`),
      { discard: true }
    )
  }))
)

const resolveTypeScriptExecutable = Effect.gen(function*() {
  const components = new Set<Component>(["typescript"])
  const targets = yield* discoverSelected(components, defaultTypescriptPackageNames[0])
  return yield* resolveReplacement(targets[0]!)
})

const getExePathCommand = Command.make("get-exe-path").pipe(
  Command.withDescription("Print the Effect Language Service executable path"),
  Command.withHandler(() => Effect.gen(function*() {
    const fs = yield* FileSystem.FileSystem
    const replacement = yield* resolveTypeScriptExecutable
    yield* fs.chmod(replacement.path, 0o755).pipe(
      Effect.mapError(() => new ChmodBinaryError({ targetPath: replacement.path }))
    )
    yield* Console.log(replacement.path)
  }))
)

const diagnosticsCommand = Command.make("diagnostics", {
  file: Flag.file("file").pipe(
    Flag.optional,
    Flag.withDescription("The full path of the file to check for diagnostics")
  ),
  project: Flag.file("project").pipe(
    Flag.optional,
    Flag.withDescription("The full path of the project tsconfig.json file to check for diagnostics")
  ),
  format: Flag.choice(
    "format",
    ["json", "pretty", "text", "github-actions"] as ReadonlyArray<DiagnosticsOutputFormat>
  ).pipe(
    Flag.withDefault("pretty" as const),
    Flag.withDescription(
      "Output format: json (machine-readable), pretty (colored with context), text (plain text), github-actions (workflow commands)"
    )
  ),
  strict: Flag.boolean("strict").pipe(
    Flag.withDefault(false),
    Flag.withDescription("Treat warnings as errors (affects exit code)")
  ),
  severity: Flag.string("severity").pipe(
    Flag.optional,
    Flag.withDescription("Filter by severity levels (comma-separated: error,warning,message)")
  ),
  progress: Flag.boolean("progress").pipe(
    Flag.withDefault(false),
    Flag.withDescription("Show progress as files are checked (outputs to stderr)")
  ),
  lspconfig: Flag.string("lspconfig").pipe(
    Flag.optional,
    Flag.withDescription("An optional inline JSON lsp config that replaces the current project lsp config")
  )
}).pipe(
  Command.withDescription("Gets the Effect language service diagnostics on the given files or project"),
  Command.withHandler(({ file, format, lspconfig, progress, project, severity, strict }) => Effect.gen(function*() {
    const fs = yield* FileSystem.FileSystem
    const replacement = yield* resolveTypeScriptExecutable
    yield* fs.chmod(replacement.path, 0o755).pipe(
      Effect.mapError(() => new ChmodBinaryError({ targetPath: replacement.path }))
    )
    const result = runDiagnosticsBinary(replacement.path, {
      cwd: process.cwd(),
      file: Option.getOrUndefined(file),
      project: Option.getOrUndefined(project),
      format,
      strict,
      severity: Option.getOrUndefined(severity),
      progress,
      lspconfig: Option.getOrUndefined(lspconfig)
    })
    propagateDiagnosticsExit(result)
  }))
)

const rootCommand = Command.make("tsgo").pipe(
  Command.withSubcommands([patchCommand, unpatchCommand, getExePathCommand, diagnosticsCommand, setupCommand, configCommand])
)

rootCommand.pipe(
  Command.run({ version: pkgJson.version }),
  Effect.provide(NodeServices.layer),
  NodeRuntime.runMain()
)
