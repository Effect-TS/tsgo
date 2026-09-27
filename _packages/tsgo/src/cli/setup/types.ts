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

export type Editor = "vscode" | "zed" | "nvim" | "emacs"
export type Integration = "typescript" | "oxlint"

export interface PackageDependency {
  readonly dependencyType: "dependencies" | "devDependencies"
  readonly version: string
  /**
   * The npm package name this dependency refers to.
   */
  readonly packageName?: string
}

export namespace Assessment {
  export interface Input {
    readonly packageJson: FileInput
    readonly tsconfig: FileInput
    readonly oxlintConfig: Option.Option<FileInput>
    readonly vscodeSettings: Option.Option<FileInput>
    readonly zedSettings: Option.Option<FileInput>
  }

  export interface PackageJson {
    readonly path: string
    readonly sourceFile: ts.JsonSourceFile
    readonly parsed: Record<string, unknown>
    readonly text: string
    readonly lspVersion: Option.Option<PackageDependency>
    readonly typescriptVersion: Option.Option<PackageDependency>
    readonly oxlintVersion: Option.Option<PackageDependency>
    readonly oxlintTsgolintVersion: Option.Option<PackageDependency>
    readonly vitePlusVersion: Option.Option<PackageDependency>
    readonly prepareScript: Option.Option<{
      readonly script: string
      readonly hasPatch: boolean
      readonly integrations: ReadonlyArray<Integration>
    }>
  }

  export interface TsConfig {
    readonly path: string
    readonly sourceFile: ts.JsonSourceFile
    readonly parsed: Record<string, unknown>
    readonly text: string
    readonly hasPlugins: boolean
    readonly hasLspPlugin: boolean
    readonly currentSchemaPath: Option.Option<string>
    readonly currentDiagnosticSeverities: Option.Option<Record<string, RuleSeverity>>
  }

  export interface VSCodeSettings {
    readonly path: string
    readonly sourceFile: ts.JsonSourceFile
    readonly parsed: Record<string, unknown>
    readonly text: string
  }

  export type ZedSettings = VSCodeSettings

  export interface OxlintConfig {
    readonly path: string
    readonly sourceFile: ts.JsonSourceFile
    readonly parsed: Record<string, unknown>
    readonly text: string
    readonly currentSchemaPath: Option.Option<string>
  }

  export interface State {
    readonly packageJson: PackageJson
    readonly tsconfig: TsConfig
    readonly oxlintConfig: Option.Option<OxlintConfig>
    readonly vscodeSettings: Option.Option<VSCodeSettings>
    readonly zedSettings: Option.Option<ZedSettings>
  }
}

export namespace Target {
  export interface PackageJson {
    readonly lspVersion: Option.Option<PackageDependency>
    readonly typescriptVersion: Option.Option<PackageDependency>
    readonly oxlintVersion: Option.Option<PackageDependency>
    readonly oxlintTsgolintVersion: Option.Option<PackageDependency>
    readonly prepareScript: boolean
    readonly managePrepareScript: boolean
    readonly integrations: ReadonlyArray<Integration>
  }

  export interface TsConfig {
    readonly schemaPath: Option.Option<string>
    readonly diagnosticSeverities: Option.Option<Record<string, RuleSeverity>>
    readonly manageIntegration: boolean
  }

  export interface VSCodeSettings {
    readonly settings: Record<string, unknown>
  }

  export interface ZedSettings {
    readonly settings: Record<string, unknown>
  }

  export interface State {
    readonly packageJson: PackageJson
    readonly tsconfig: TsConfig
    readonly oxlintrcSchemaPath: Option.Option<string>
    readonly vscodeSettings: Option.Option<VSCodeSettings>
    readonly zedSettings: Option.Option<ZedSettings>
    readonly editors: ReadonlyArray<Editor>
  }
}
