import type * as Option from "effect/Option"
import type * as ts from "typescript"
import type { RuleSeverity } from "./rule-info.js"

export interface SetupFileTextChanges extends ts.FileTextChanges {
  readonly isNewFile: boolean
}

export interface SetupCodeAction {
  readonly description: string
  readonly changes: ReadonlyArray<SetupFileTextChanges>
}

export interface FileInput {
  readonly fileName: string
  readonly text: string
}

export type Editor = "vscode" | "nvim" | "emacs"

export interface PackageDependency {
  readonly dependencyType: "dependencies" | "devDependencies"
  readonly version: string
}

export namespace Assessment {
  export interface Input {
    readonly packageJson: FileInput
    readonly tsconfig: FileInput
    readonly vscodeSettings: Option.Option<FileInput>
  }

  export interface PackageJson {
    readonly path: string
    readonly sourceFile: ts.JsonSourceFile
    readonly parsed: Record<string, unknown>
    readonly text: string
    readonly lspVersion: Option.Option<PackageDependency>
    readonly nativePreviewVersion: Option.Option<PackageDependency>
    readonly prepareScript: Option.Option<{
      readonly script: string
      readonly hasPatch: boolean
    }>
  }

  export interface TsConfig {
    readonly path: string
    readonly sourceFile: ts.JsonSourceFile
    readonly parsed: Record<string, unknown>
    readonly text: string
    readonly hasPlugins: boolean
    readonly hasLspPlugin: boolean
    readonly currentDiagnosticSeverities: Option.Option<Record<string, RuleSeverity>>
  }

  export interface VSCodeSettings {
    readonly path: string
    readonly sourceFile: ts.JsonSourceFile
    readonly parsed: Record<string, unknown>
    readonly text: string
  }

  export interface State {
    readonly packageJson: PackageJson
    readonly tsconfig: TsConfig
    readonly vscodeSettings: Option.Option<VSCodeSettings>
  }
}

export namespace Target {
  export interface PackageJson {
    readonly lspVersion: Option.Option<PackageDependency>
    readonly nativePreviewVersion: Option.Option<PackageDependency>
    readonly prepareScript: boolean
  }

  export interface TsConfig {
    readonly diagnosticSeverities: Option.Option<Record<string, RuleSeverity>>
  }

  export interface VSCodeSettings {
    readonly settings: Record<string, unknown>
  }

  export interface State {
    readonly packageJson: PackageJson
    readonly tsconfig: TsConfig
    readonly vscodeSettings: Option.Option<VSCodeSettings>
    readonly editors: ReadonlyArray<Editor>
  }
}
