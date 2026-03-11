import * as Effect from "effect/Effect"
import * as FileSystem from "effect/FileSystem"
import * as Option from "effect/Option"
import * as Path from "effect/Path"
import type * as PlatformError from "effect/PlatformError"
import * as ts from "typescript"
import { FileReadError, PackageJsonNotFoundError } from "./errors.js"
import type { Assessment, FileInput } from "./types.js"
import { LSP_PACKAGE_NAME, LSP_PLUGIN_NAME, PATCH_COMMAND } from "./consts.js"

/**
 * Read files from file system and create assessment input
 */
export const createAssessmentInput = (
  currentDir: string,
  tsconfigInput: FileInput
): Effect.Effect<
  Assessment.Input,
  PackageJsonNotFoundError | FileReadError | PlatformError.PlatformError,
  FileSystem.FileSystem | Path.Path
> =>
  Effect.gen(function*() {
    const fs = yield* FileSystem.FileSystem
    const path = yield* Path.Path

    // Read package.json (required)
    const packageJsonPath = path.join(currentDir, "package.json")
    const packageJsonExists = yield* fs.exists(packageJsonPath)

    if (!packageJsonExists) {
      return yield* new PackageJsonNotFoundError({ path: packageJsonPath })
    }

    const packageJsonText = yield* fs.readFileString(packageJsonPath).pipe(
      Effect.mapError((cause) => new FileReadError({ path: packageJsonPath, cause }))
    )
    const packageJsonInput: FileInput = {
      fileName: packageJsonPath,
      text: packageJsonText
    }

    // Read .vscode/settings.json (optional)
    const vscodeSettingsPath = path.join(currentDir, ".vscode", "settings.json")
    const vscodeSettingsExists = yield* fs.exists(vscodeSettingsPath)

    let vscodeSettingsInput = Option.none<FileInput>()
    if (vscodeSettingsExists) {
      const vscodeSettingsText = yield* fs.readFileString(vscodeSettingsPath).pipe(
        Effect.mapError((cause) => new FileReadError({ path: vscodeSettingsPath, cause }))
      )
      vscodeSettingsInput = Option.some({
        fileName: vscodeSettingsPath,
        text: vscodeSettingsText
      })
    }

    return {
      packageJson: packageJsonInput,
      tsconfig: tsconfigInput,
      vscodeSettings: vscodeSettingsInput
    }
  })

/**
 * Assess package.json from input
 */
const assessPackageJson = (
  input: FileInput
): Assessment.PackageJson => {
  const sourceFile = ts.parseJsonText(input.fileName, input.text)
  const errors: Array<ts.Diagnostic> = []
  const parsed = ts.convertToObject(sourceFile, errors) as {
    devDependencies?: Record<string, string>
    dependencies?: Record<string, string>
    scripts?: Record<string, string>
  }

  // Check for @effect/tsgo in both dependencies and devDependencies
  let lspVersion: Option.Option<
    { readonly dependencyType: "dependencies" | "devDependencies"; readonly version: string }
  > = Option.none()

  if (LSP_PACKAGE_NAME in (parsed.devDependencies ?? {})) {
    lspVersion = Option.some({
      dependencyType: "devDependencies" as const,
      version: parsed.devDependencies![LSP_PACKAGE_NAME]
    })
  } else if (LSP_PACKAGE_NAME in (parsed.dependencies ?? {})) {
    lspVersion = Option.some({
      dependencyType: "dependencies" as const,
      version: parsed.dependencies![LSP_PACKAGE_NAME]
    })
  }

  // Check for prepare script
  const prepareScript = "prepare" in (parsed.scripts ?? {})
    ? Option.some({
      script: parsed.scripts!.prepare,
      hasPatch: parsed.scripts!.prepare.toLowerCase().includes(PATCH_COMMAND)
    })
    : Option.none()

  return {
    path: input.fileName,
    sourceFile,
    parsed,
    text: input.text,
    lspVersion,
    prepareScript
  }
}

/**
 * Assess tsconfig from input
 */
const assessTsConfig = (
  input: FileInput
): Assessment.TsConfig => {
  const sourceFile = ts.parseJsonText(input.fileName, input.text)
  const errors: Array<ts.Diagnostic> = []
  const parsed = ts.convertToObject(sourceFile, errors) as {
    compilerOptions?: {
      plugins?: Array<{
        name?: string
        [key: string]: unknown
      }>
    }
  }

  const hasPlugins = parsed.compilerOptions?.plugins !== undefined
  const plugins = parsed.compilerOptions?.plugins ?? []
  const hasLspPlugin = plugins.some((plugin) => plugin.name === LSP_PLUGIN_NAME)

  return {
    path: input.fileName,
    sourceFile,
    parsed,
    text: input.text,
    hasPlugins,
    hasLspPlugin
  }
}

/**
 * Assess VSCode settings from input
 */
const assessVSCodeSettings = (
  input: FileInput
): Assessment.VSCodeSettings => {
  const sourceFile = ts.parseJsonText(input.fileName, input.text)
  const errors: Array<ts.Diagnostic> = []
  const parsed = ts.convertToObject(sourceFile, errors) as Record<string, unknown>

  return {
    path: input.fileName,
    sourceFile,
    parsed,
    text: input.text
  }
}

/**
 * Perform assessment from input data
 */
export const assess = (
  input: Assessment.Input
): Assessment.State => {
  const packageJson = assessPackageJson(input.packageJson)
  const tsconfig = assessTsConfig(input.tsconfig)

  const vscodeSettings = Option.isSome(input.vscodeSettings)
    ? Option.some(assessVSCodeSettings(input.vscodeSettings.value))
    : Option.none<Assessment.VSCodeSettings>()

  return {
    packageJson,
    tsconfig,
    vscodeSettings
  }
}
